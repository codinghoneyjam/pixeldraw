//! View / viewport math — the zoom·pan·clamp·fit SSOT (docs/render.md §2).
//!
//! Ported 1:1 from two DOM-free JS modules:
//!
//! * `src/features/viewport/view.js` (86 lines) — the pure functions. Their
//!   worked examples are pinned by `tools/view_check.mjs` (against
//!   `reference/view_ref.mjs`) and by `tests/view_basic.test.mjs`, so every
//!   rounding decision below is a contract, not a choice.
//! * `src/features/viewport/view_store.js` (71 lines) — the mutable holder,
//!   which becomes [`ViewStore`] below, minus its `subscribe` set.
//!
//! ## Coordinate model
//!
//! A `View` is `{ zoom, offset_x, offset_y }`: `zoom` is CSS px per canvas px,
//! and `(offset_x, offset_y)` is where canvas (0,0) lands in viewport-relative
//! CSS px. `f64` is the domain of the zoom and both offsets. Canvas pixel
//! indices that must become integers are converted in exactly one place each
//! ([`pixel_at`], [`visible_chunk_range`]) and the rule is stated there.
//!
//! ## What is deliberately *not* here
//!
//! Notification. `view_store.js` kept a private `subscribe` set whose two
//! subscribers were `renderer.requestRender()` (`ui/app.js`) and the
//! status-bar repaint. Neither is one of the seven `events::EVENTS` signals,
//! so this port does not invent an eighth catalog entry. [`ViewChange::events`]
//! returns the catalog names a change should emit (currently none) and the app
//! crate wires the redraw itself; see its doc comment.
//!
//! ## Error codes
//!
//! `view.js` raised `OUT_OF_RANGE` at three sites — a non-finite
//! `zoom`/`offsetX`/`offsetY`, `zoom <= 0`, and `newZoom <= 0` — and those
//! three stay `OUT_OF_RANGE`. Changing them would be a silent behavioural
//! change; the reference tooling only pins the error *type*, not its code.
//! `VIEWPORT_INVALID` is raised by [`Viewport::new`], the display-viewport
//! guard whose JS analogue (`features/document/document.js` L62, "viewport
//! size must be positive") raised exactly that. `VIEWPORT_OUT_OF_RANGE` never
//! fires from this module: it belongs to `document.js`'s `validateViewport`
//! crop rect, which is document-model concern rather than view math.

pub use crate::constants::ZOOM_LEVELS;

use crate::errors::{CODE_VIEWPORT_INVALID, DrawToolError};

/// `clampView`'s default margin: the canvas is never scrolled fully out of
/// view, so 32 CSS px of it always stays on screen.
pub const CLAMP_MARGIN: f64 = 32.0;

/// `fitView`'s default padding.
pub const FIT_PADDING: f64 = 16.0;

/// The sub-pixel fudge `visibleChunkRange` subtracts from the far edge.
const CHUNK_EDGE_EPS: f64 = 1e-9;

/// First and last rung of the frozen ladder, so the fallbacks in
/// [`step_zoom`] / [`largest_rung_at_most`] never index a magic number.
const ZOOM_MIN_INDEX: usize = 0;
const ZOOM_MAX_INDEX: usize = ZOOM_LEVELS.len() - 1;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A point in viewport-relative CSS px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPoint {
    pub x: f64,
    pub y: f64,
}

/// A point in canvas px — *unrounded*, i.e. `zoom` still applied in reverse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasPoint {
    pub x: f64,
    pub y: f64,
}

/// An integer canvas pixel index.
///
/// `i64`, not `u32`: a pointer left of or above the canvas legitimately reads
/// negative (`tools/view_check.mjs` pins `pixelAt(v, 99, 49) == {-1, -1}`), and
/// the off-canvas painting tests reach `-5000`, so `u32` would wrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixel {
    pub x: i64,
    pub y: i64,
}

/// `View = { zoom, offsetX, offsetY }`.
///
/// The fields are private so the invariants — finite values, `zoom > 0` — hold
/// by construction. `view.js` enforced them with a runtime `assertNum`; making
/// the constructor the only door turns that into a type-level guarantee.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    zoom: f64,
    offset_x: f64,
    offset_y: f64,
}

/// `createView(o = {})`'s argument.
///
/// Every field is optional because the JS used `??`: an absent `zoom` defaults
/// to 1 and an absent offset to 0. `None` is that "absent" state, which is why
/// these are not plain `f64` with sentinel values.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ViewOptions {
    pub zoom: Option<f64>,
    pub offset_x: Option<f64>,
    pub offset_y: Option<f64>,
}

/// The inclusive chunk-index rectangle a viewport can see.
///
/// `None` (the JS `null`) means the canvas is entirely scrolled off screen;
/// `paintDirty`'s fast path keys straight off that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleChunkRange {
    pub cx0: u32,
    pub cy0: u32,
    pub cx1: u32,
    pub cy1: u32,
}

/// The display viewport in CSS px — `view_store.js`'s `viewport()` result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
}

// ---------------------------------------------------------------------------
// View construction
// ---------------------------------------------------------------------------

impl View {
    /// The three components, validated. Equivalent to `createView` with all
    /// fields present.
    pub fn new(zoom: f64, offset_x: f64, offset_y: f64) -> Result<Self, DrawToolError> {
        create_view(ViewOptions {
            zoom: Some(zoom),
            offset_x: Some(offset_x),
            offset_y: Some(offset_y),
        })
    }

    /// `createView()` — zoom 1, offsets 0. The un-zoomed, top-left-anchored view.
    pub fn identity() -> Self {
        Self {
            zoom: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn offset_x(&self) -> f64 {
        self.offset_x
    }

    pub fn offset_y(&self) -> f64 {
        self.offset_y
    }
}

impl Viewport {
    /// `host.clientWidth || 800` / `host.clientHeight || 600` — the size the JS
    /// store fell back to when the host reported nothing usable.
    pub const DEFAULT: Viewport = Viewport {
        width: 800.0,
        height: 600.0,
    };

    /// Rejects a viewport that is not a positive, finite size.
    ///
    /// Raises `VIEWPORT_INVALID`, the code `features/document/document.js`
    /// used for its own "viewport size must be positive" guard. The JS *store*
    /// did not raise: it silently substituted its 800×600 fallback inside a
    /// `try {} catch {}` around `host.clientWidth`. This port keeps both halves
    /// — [`Viewport::new`] reports the problem, [`ViewStore::set_viewport`]
    /// swallows it exactly like the JS did.
    pub fn new(width: f64, height: f64) -> Result<Self, DrawToolError> {
        if !width.is_finite() || !height.is_finite() {
            return Err(DrawToolError::new(
                CODE_VIEWPORT_INVALID,
                "viewport width and height must be finite numbers",
            ));
        }
        if width <= 0.0 || height <= 0.0 {
            return Err(DrawToolError::new(
                CODE_VIEWPORT_INVALID,
                format!("viewport size must be positive, got {width}x{height}"),
            ));
        }
        Ok(Self { width, height })
    }
}

// ---------------------------------------------------------------------------
// Pure view math (view.js)
// ---------------------------------------------------------------------------

/// `createView(o = {})`, defaults and all.
///
/// An absent (or `None`) field takes the JS `??` default; a *present* field is
/// validated, so `createView({ zoom: NaN })` is the error the JS raised while
/// `createView({})` is the harmless `zoom: 1` view.
pub fn create_view(o: ViewOptions) -> Result<View, DrawToolError> {
    let zoom = o.zoom.unwrap_or(1.0);
    let offset_x = o.offset_x.unwrap_or(0.0);
    let offset_y = o.offset_y.unwrap_or(0.0);
    require_finite(zoom, "zoom")?;
    require_finite(offset_x, "offsetX")?;
    require_finite(offset_y, "offsetY")?;
    if zoom <= 0.0 {
        return Err(DrawToolError::out_of_range("view.zoom must be > 0"));
    }
    Ok(View {
        zoom,
        offset_x,
        offset_y,
    })
}

/// `assertNum` — the JS `typeof v !== "number" || !Number.isFinite(v)` guard.
fn require_finite(v: f64, name: &str) -> Result<(), DrawToolError> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(DrawToolError::out_of_range(format!(
            "view.{name} must be a finite number"
        )))
    }
}

/// `screenToCanvas` — `(sx - offsetX) / zoom`, float in and float out.
///
/// No rounding: callers that need a pixel index go through [`pixel_at`], which
/// is where the floor happens.
pub fn screen_to_canvas(v: View, sx: f64, sy: f64) -> CanvasPoint {
    CanvasPoint {
        x: (sx - v.offset_x) / v.zoom,
        y: (sy - v.offset_y) / v.zoom,
    }
}

/// `canvasToScreen` — `offset + x * zoom`.
///
/// Multiplication first, exactly as the JS wrote it: that is the operation
/// order that makes this a bit-exact inverse of [`screen_to_canvas`] for the
/// values `zoomAt` produces.
pub fn canvas_to_screen(v: View, x: f64, y: f64) -> ScreenPoint {
    ScreenPoint {
        x: v.offset_x + x * v.zoom,
        y: v.offset_y + y * v.zoom,
    }
}

/// `pixelAt` — `screenToCanvas` plus `Math.floor`.
///
/// Never clamps: a pointer left of the canvas reads `-1`, not `0`, which is
/// what the off-canvas drawing tests rely on.
pub fn pixel_at(v: View, sx: f64, sy: f64) -> Pixel {
    let p = screen_to_canvas(v, sx, sy);
    Pixel {
        x: floor_i64(p.x),
        y: floor_i64(p.y),
    }
}

/// `Math.floor` to an integer, in one place.
///
/// The `floor()` must come first: a bare `as i64` truncates toward zero, which
/// would turn `-0.5` into `0` instead of `-1` and quietly mirror the canvas
/// across its left edge. After the floor the value is integral, so the cast is
/// exact. Rust's float-to-int cast saturates rather than trapping, so an
/// absurd input yields `i64::MAX` where JS would yield `NaN` — unreachable,
/// since `View` rejects non-finite construction and sane inputs stay in range.
#[inline]
fn floor_i64(v: f64) -> i64 {
    v.floor() as i64
}

/// `zoomAt` — re-zoom while holding the anchor `(ax, ay)` fixed.
///
/// `k = newZoom / zoom` is computed once and reused for both axes; factoring it
/// differently is not just slower, it changes the last bits and the
/// "anchor stays fixed" invariant in `tools/view_check.mjs` starts failing at
/// the 1e-9 tolerance instead of passing bit-exactly.
pub fn zoom_at(v: View, ax: f64, ay: f64, new_zoom: f64) -> Result<View, DrawToolError> {
    require_finite(new_zoom, "zoom")?;
    if new_zoom <= 0.0 {
        return Err(DrawToolError::out_of_range("newZoom must be > 0"));
    }
    let k = new_zoom / v.zoom;
    Ok(View {
        zoom: new_zoom,
        offset_x: ax - (ax - v.offset_x) * k,
        offset_y: ay - (ay - v.offset_y) * k,
    })
}

/// `stepZoom` — one rung of the frozen ladder.
///
/// Both fallbacks are deliberate, and both are "sticky at the wrong end":
///
/// * `stepZoom(64, 1) === 32`, because `.find(l => l > 64)` matches nothing and
///   the JS fell back to `ZOOM_LEVELS.at(-1)`.
/// * `stepZoom(0.1, -1) === 0.25`, by the same argument at the bottom.
///
/// A view already above or below the ladder therefore moves *toward* the
/// ladder on the next step rather than staying put. `dir == 0` is an identity
/// that does not snap to a rung at all.
pub fn step_zoom(zoom: f64, dir: i32) -> f64 {
    if dir > 0 {
        ZOOM_LEVELS
            .iter()
            .copied()
            .find(|&level| level > zoom)
            .unwrap_or(ZOOM_LEVELS[ZOOM_MAX_INDEX])
    } else if dir < 0 {
        ZOOM_LEVELS
            .iter()
            .rev()
            .copied()
            .find(|&level| level < zoom)
            .unwrap_or(ZOOM_LEVELS[ZOOM_MIN_INDEX])
    } else {
        zoom
    }
}

/// `panBy` — offsets shift, and nothing else moves.
///
/// Deliberately unclamped, matching the JS test
/// `panBy(createView({zoom: 2}), 5, -7) === {zoom: 2, offsetX: 5, offsetY: -7}`.
/// Clamping is [`ViewStore::set`]'s job, once, on commit; clamping here as
/// well would make a drag non-cancellable mid-gesture.
pub fn pan_by(v: View, dx: f64, dy: f64) -> View {
    View {
        zoom: v.zoom,
        offset_x: v.offset_x + dx,
        offset_y: v.offset_y + dy,
    }
}

/// `clampView` at the default [`CLAMP_MARGIN`].
pub fn clamp_view(v: View, canvas_w: f64, canvas_h: f64, vw: f64, vh: f64) -> View {
    clamp_view_with_margin(v, canvas_w, canvas_h, vw, vh, CLAMP_MARGIN)
}

/// `clampView` with an explicit margin.
pub fn clamp_view_with_margin(
    v: View,
    canvas_w: f64,
    canvas_h: f64,
    vw: f64,
    vh: f64,
    margin: f64,
) -> View {
    View {
        zoom: v.zoom,
        offset_x: clamp_offset(v.offset_x, margin - canvas_w * v.zoom, vw - margin),
        offset_y: clamp_offset(v.offset_y, margin - canvas_h * v.zoom, vh - margin),
    }
}

/// `Math.min(Math.max(val, lo), hi)` — in that order.
///
/// The order is load-bearing. When the range inverts (`hi < lo`, which happens
/// once `canvas * zoom` drops below `2 * margin - vw`) `Math.max` clamps to
/// `lo` first and `Math.min` then drags the answer back down to `hi`; swapping
/// the two calls would pin it to `lo` instead. `tests/view_basic.test.mjs`'s
/// oversized-offset case exercises the plain path, and `clamp_view_*` below
/// pins the inverted one.
fn clamp_offset(val: f64, lo: f64, hi: f64) -> f64 {
    val.max(lo).min(hi)
}

/// `fitView` at the default [`FIT_PADDING`].
pub fn fit_view(canvas_w: f64, canvas_h: f64, vw: f64, vh: f64) -> View {
    fit_view_with_padding(canvas_w, canvas_h, vw, vh, FIT_PADDING)
}

/// `fitView` with an explicit padding.
///
/// `want` is the continuous scale that would fit both axes; the ladder then
/// takes over and never exceeds it, so zooming out is always available.
pub fn fit_view_with_padding(canvas_w: f64, canvas_h: f64, vw: f64, vh: f64, padding: f64) -> View {
    let want = ((vw - 2.0 * padding) / canvas_w).min((vh - 2.0 * padding) / canvas_h);
    let zoom = largest_rung_at_most(want);
    View {
        zoom,
        offset_x: round_half_up((vw - canvas_w * zoom) / 2.0),
        offset_y: round_half_up((vh - canvas_h * zoom) / 2.0),
    }
}

/// `[...ZOOM_LEVELS].reverse().find(l => l <= want) ?? ZOOM_LEVELS[0]`.
///
/// The fallback is deliberate: a canvas bigger than even 0.25× of the viewport
/// stops at 0.25× rather than shrinking further, because the ladder is a
/// frozen whitelist and not a continuous scale.
fn largest_rung_at_most(want: f64) -> f64 {
    ZOOM_LEVELS
        .iter()
        .rev()
        .copied()
        .find(|&level| level <= want)
        .unwrap_or(ZOOM_LEVELS[ZOOM_MIN_INDEX])
}

/// `actualSizeView` — zoom 1, centred.
///
/// Note there is no ladder involvement and no clamping here: `view_store`'s
/// `actual()` runs the result through `set`, which clamps it.
pub fn actual_size_view(canvas_w: f64, canvas_h: f64, vw: f64, vh: f64) -> View {
    View {
        zoom: 1.0,
        offset_x: round_half_up((vw - canvas_w) / 2.0),
        offset_y: round_half_up((vh - canvas_h) / 2.0),
    }
}

/// `Math.round`, which is **not** `f64::round`.
///
/// JS rounds halves toward +∞ (`Math.round(-2.5) === -2`); Rust's `f64::round`
/// rounds halves away from zero (`(-2.5f64).round() === -3`). They disagree
/// exactly on the `.5` boundary of a negative value — i.e. when the canvas is
/// wider or taller than its viewport — which is the only time [`fit_view`] and
/// [`actual_size_view`] centre something negative. `Math.floor(v + 0.5)` is the
/// bit-identical port.
#[inline]
fn round_half_up(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// `visibleChunkRange` — the chunks a viewport can see, or `None`.
///
/// The far-edge `- 1e-9` is the single most important detail in this function.
/// It keeps a canvas edge that lands exactly on a chunk boundary from
/// over-reporting by one chunk: with `canvas = 128`, viewport `64`, zoom 1, the
/// far edge sits on canvas x=64, and `floor(64/32) == 2` would claim chunk 2,
/// which covers canvas 64..95 and is therefore off screen. Subtracting the
/// epsilon floors to 1, the last chunk that is actually visible. Without it
/// the renderer composites a chunk nobody can see, and `paintDirty`'s budget
/// burns for nothing.
///
/// `cx0`/`cy0` clamp at zero instead: chunks left of or above the canvas do not
/// exist. The `None` return is the JS's "scrolled fully off screen".
pub fn visible_chunk_range(
    v: View,
    canvas_w: f64,
    canvas_h: f64,
    vw: f64,
    vh: f64,
) -> Option<VisibleChunkRange> {
    let a = screen_to_canvas(v, 0.0, 0.0);
    let b = screen_to_canvas(v, vw, vh);
    let chunk = crate::constants::CHUNK_PX as f64;

    let cx0 = (a.x / chunk).floor().max(0.0);
    let cy0 = (a.y / chunk).floor().max(0.0);
    // Far edges are capped by the canvas, so the near edges can never exceed
    // them; the comparison below is what turns an inverted range into `None`.
    let cx1 = (canvas_w / chunk - 1.0).min(((b.x - CHUNK_EDGE_EPS) / chunk).floor());
    let cy1 = (canvas_h / chunk - 1.0).min(((b.y - CHUNK_EDGE_EPS) / chunk).floor());

    if cx1 < cx0 || cy1 < cy0 {
        return None;
    }
    Some(VisibleChunkRange {
        cx0: floor_u32(cx0),
        cy0: floor_u32(cy0),
        cx1: floor_u32(cx1),
        cy1: floor_u32(cy1),
    })
}

/// The float→`u32` conversion for chunk indices.
///
/// `floor()` first, for the same reason as [`floor_i64`], and `.max(0.0)`
/// before the cast so the value that reaches `as u32` is always representable:
/// the saturating cast would otherwise turn a stray negative into 0 silently.
/// The callers above have already established `0 <= cx0 <= cx1`, so both guards
/// document the contract rather than defend against a reachable case.
#[inline]
fn floor_u32(v: f64) -> u32 {
    v.floor().max(0.0) as u32
}

// ---------------------------------------------------------------------------
// The holder (view_store.js)
// ---------------------------------------------------------------------------

/// What a mutating [`ViewStore`] call produced.
///
/// The JS store had no return value: it committed and fanned out to its
/// subscribers. A Rust API that mutates behind `&mut self` and returns nothing
/// forces the caller to re-read the store to learn what happened, so this is
/// the committed view plus a `changed` flag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewChange {
    /// The committed view, already clamped when a canvas is attached.
    pub view: View,
    /// False when the call was a no-op (a rejected zoom, a missing document) or
    /// genuinely left the view untouched.
    ///
    /// Unlike the JS, which called `emit()` unconditionally in `set`. The
    /// status bar and renderer both repainted on every `set` there; the flag
    /// lets the app crate skip the work when nothing moved.
    pub changed: bool,
}

impl ViewChange {
    /// The `events::EVENTS` names a view change should emit.
    ///
    /// Empty, on purpose. `view_store.js` never dispatched one of the seven
    /// catalog signals; its subscribers were the renderer's frame scheduler and
    /// the status bar's zoom readout, both wired directly in `ui/app.js`. So
    /// the catalog is deliberately not extended to eight — see the module
    /// header. The app crate treats `changed == true` as "request a frame and
    /// repaint the zoom label" instead.
    ///
    /// The slice is a method rather than a field so that when a real signal is
    /// eventually added it can be produced without churning every call site.
    pub fn events(&self) -> &'static [&'static str] {
        &[]
    }
}

/// The mutable holder `view_store.js` was: a [`View`], the viewport it was last
/// clamped against, and the canvas dims it was clamped to.
///
/// Three differences from the JS, all forced by the port and none of them
/// observable in the store's own behaviour:
///
/// * The DOM read is gone. `host.clientWidth` became [`ViewStore::set_viewport`],
///   which the app calls on surface resize.
/// * `session.doc` became [`ViewStore::set_canvas`]; "no document" is still the
///   no-clamp, no-fit, no-actual state it was in the JS.
/// * There is no `subscribe`. See [`ViewChange::events`].
pub struct ViewStore {
    view: View,
    viewport: Viewport,
    canvas: Option<(u32, u32)>,
}

impl ViewStore {
    /// `let view = { zoom: 1, offsetX: 0, offsetY: 0 }` — the store's initial
    /// state, at the JS fallback viewport, with no document attached.
    pub fn new() -> Self {
        Self::with_viewport(Viewport::DEFAULT)
    }

    /// [`ViewStore::new`] over an explicitly sized viewport.
    pub fn with_viewport(viewport: Viewport) -> Self {
        Self {
            view: View::identity(),
            viewport,
            canvas: None,
        }
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// The attached canvas size, if any.
    pub fn canvas(&self) -> Option<(u32, u32)> {
        self.canvas
    }

    /// Attach the document's canvas, replacing any previous one.
    ///
    /// The app calls this when `document-replaced` fires; the JS read the canvas
    /// out of `session.doc` on every operation instead.
    pub fn set_canvas(&mut self, width: u32, height: u32) -> Result<(), DrawToolError> {
        crate::constants::require_valid_canvas_size(width, height)?;
        self.canvas = Some((width, height));
        Ok(())
    }

    /// Detach the canvas — back to the JS's "no document" behaviour, where
    /// `set` stops clamping and `fit`/`actual` become no-ops.
    pub fn clear_canvas(&mut self) {
        self.canvas = None;
    }

    /// `set(v)` — clamp, then commit.
    ///
    /// The clamp is the JS store's `clamp(v)`, which no-ops when there is no
    /// document (`if (!doc) return { ...v }`). That is not an oversight: a
    /// document is attached by the app on `document-replaced`, and until then
    /// the store holds whatever it was handed.
    pub fn set(&mut self, v: View) -> ViewChange {
        let clamped = match self.canvas {
            Some((w, h)) => clamp_view(
                v,
                w as f64,
                h as f64,
                self.viewport.width,
                self.viewport.height,
            ),
            None => v,
        };
        self.commit(clamped)
    }

    /// `center()` — the anchor every store zoom keeps fixed.
    ///
    /// Note this is the *viewport* centre, not the canvas centre; the JS read
    /// the same fallback viewport here, so before a resize is reported the
    /// anchor is 400×300 regardless of the real window size.
    pub fn center(&self) -> ScreenPoint {
        ScreenPoint {
            x: self.viewport.width / 2.0,
            y: self.viewport.height / 2.0,
        }
    }

    /// `zoomTo(z)` — zoom about the viewport centre, clamped.
    ///
    /// Rejects `!Number.isFinite(z) || z <= 0` by *ignoring the call*, exactly
    /// as the JS did. `zoomAt` itself raises for the same values, so the
    /// difference is visible to callers: the pure function is strict, the store
    /// is forgiving. Reproduced deliberately rather than "fixed", because
    /// `ui/actions/action_map.js` funnels user input through here.
    pub fn zoom_to(&mut self, z: f64) -> ViewChange {
        if !z.is_finite() || z <= 0.0 {
            return ViewChange {
                view: self.view,
                changed: false,
            };
        }
        let c = self.center();
        match zoom_at(self.view, c.x, c.y, z) {
            Ok(next) => self.set(next),
            // Unreachable: `z` was just validated. Kept as a match rather than
            // an `unwrap_or` so there is no panic path outside tests.
            Err(_) => ViewChange {
                view: self.view,
                changed: false,
            },
        }
    }

    /// `zoomIn()` — one rung up the ladder about the viewport centre.
    ///
    /// Sticky at 32×: `step_zoom(32, 1) === 32`, so `zoomTo` runs again with
    /// the zoom it already had and the offsets come back unchanged.
    pub fn zoom_in(&mut self) -> ViewChange {
        self.zoom_to(step_zoom(self.view.zoom(), 1))
    }

    /// `zoomOut()` — one rung down, sticky at 0.25×.
    pub fn zoom_out(&mut self) -> ViewChange {
        self.zoom_to(step_zoom(self.view.zoom(), -1))
    }

    /// `panBy(dx, dy)` on the store, clamped through `set`.
    ///
    /// The free [`pan_by`] does not clamp; this does, because a hand drag must
    /// not be able to lose the canvas off screen.
    pub fn pan_by(&mut self, dx: f64, dy: f64) -> ViewChange {
        self.set(pan_by(self.view, dx, dy))
    }

    /// `fit()` — largest rung that fits the attached canvas, centred, clamped.
    ///
    /// A no-op with no canvas attached, matching the JS's `if (!doc) return`.
    pub fn fit(&mut self) -> ViewChange {
        let (w, h) = match self.canvas {
            Some(dim) => dim,
            None => {
                return ViewChange {
                    view: self.view,
                    changed: false,
                };
            }
        };
        let vp = self.viewport;
        self.set(fit_view(w as f64, h as f64, vp.width, vp.height))
    }

    /// `actual()` — zoom 1, centred, clamped.
    ///
    /// A no-op with no canvas attached, matching the JS's `if (!doc) return`.
    pub fn actual_size(&mut self) -> ViewChange {
        let (w, h) = match self.canvas {
            Some(dim) => dim,
            None => {
                return ViewChange {
                    view: self.view,
                    changed: false,
                };
            }
        };
        let vp = self.viewport;
        self.set(actual_size_view(w as f64, h as f64, vp.width, vp.height))
    }

    /// Report a new viewport size and re-clamp.
    ///
    /// This is `window.addEventListener("resize", () => { renderer.resize();
    /// viewStore.set(viewStore.get()); })` from `ui/app.js` — the store re-read
    /// the DOM and committed an unchanged view purely to re-run the clamp.
    ///
    /// A size the host reported badly (zero, negative, non-finite) falls back
    /// to [`Viewport::DEFAULT`] instead of raising: that is the JS's
    /// `try {} catch {}` around `host.clientWidth`, kept word for word. Growing
    /// the window can therefore pull the canvas back in from off screen.
    pub fn set_viewport(&mut self, width: f64, height: f64) -> ViewChange {
        let vp = match Viewport::new(width, height) {
            Ok(vp) => vp,
            Err(_) => Viewport::DEFAULT,
        };
        self.viewport = vp;
        // Re-run the clamp against the new size; same trick the JS resize
        // handler used (set the view it was already holding).
        self.set(self.view)
    }

    /// Commit a view and report whether it actually moved.
    fn commit(&mut self, next: View) -> ViewChange {
        let changed = next != self.view;
        self.view = next;
        ViewChange {
            view: next,
            changed,
        }
    }
}

impl Default for ViewStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
