//! The view types and their constructors — `view.js`'s `createView`, plus the
//! three coordinate point structs the rest of the module passes around.
//!
//! Deliberately a leaf: nothing here calls the other `view` submodules, which is
//! what lets `zoom`·`geom`·`clamp`·`fit` depend on it without a cycle.

use crate::constants::require_valid_canvas_size;
use crate::errors::{CODE_VIEWPORT_INVALID, DrawToolError};

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
/// negative (`tools/view_check.mjs` pins `pixelAt(v, 99, 49) == {-1, -1}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixel {
    pub x: i64,
    pub y: i64,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
}

impl Viewport {
    /// `host.clientWidth || 800` / `host.clientHeight || 600`.
    pub const DEFAULT: Viewport = Viewport {
        width: 800.0,
        height: 600.0,
    };
}

/// `View = { zoom, offsetX, offsetY }`. Fields are private so the invariants
/// (finite, `zoom > 0`) hold by construction; `create_view` is the only door.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    zoom: f64,
    offset_x: f64,
    offset_y: f64,
}

/// `createView(o = {})`'s argument — every field optional because the JS used
/// `??`, so `None` is "absent" rather than a sentinel value.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ViewOptions {
    pub zoom: Option<f64>,
    pub offset_x: Option<f64>,
    pub offset_y: Option<f64>,
}

/// The inclusive chunk-index rectangle a viewport can see. `None` means the
/// canvas is scrolled fully off screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleChunkRange {
    pub cx0: u32,
    pub cy0: u32,
    pub cx1: u32,
    pub cy1: u32,
}

/// `createView(o = {})`, defaults and all.
///
/// An absent field takes the `??` default; a *present* field is validated, so
/// `createView({ zoom: NaN })` is the error the JS raised while `createView({})`
/// is the harmless `zoom: 1` view.
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
pub(crate) fn require_finite(v: f64, name: &str) -> Result<(), DrawToolError> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(DrawToolError::out_of_range(format!(
            "view.{name} must be a finite number"
        )))
    }
}

impl View {
    pub fn new(zoom: f64, offset_x: f64, offset_y: f64) -> Result<Self, DrawToolError> {
        create_view(ViewOptions {
            zoom: Some(zoom),
            offset_x: Some(offset_x),
            offset_y: Some(offset_y),
        })
    }

    pub fn identity() -> Self {
        Self {
            zoom: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    #[inline]
    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    #[inline]
    pub fn offset_x(&self) -> f64 {
        self.offset_x
    }

    #[inline]
    pub fn offset_y(&self) -> f64 {
        self.offset_y
    }
}

/// Package the three components, bypassing validation.
///
/// Private-field construction for the sibling modules that re-derive a view from
/// a valid one: the inputs are already finite and positive, so re-running
/// [`create_view`] would only add an error path that cannot be reached.
pub(crate) fn from_parts(zoom: f64, offset_x: f64, offset_y: f64) -> View {
    View {
        zoom,
        offset_x,
        offset_y,
    }
}

impl From<View> for (f64, f64, f64) {
    fn from(v: View) -> Self {
        (v.zoom, v.offset_x, v.offset_y)
    }
}

/// Rejects a viewport that is not positive and finite.
///
/// Raises `VIEWPORT_INVALID`, the code `document.js` used for its own "viewport
/// size must be positive" guard. The JS *store* did not raise — it silently
/// substituted its 800×600 fallback — so both halves are kept: this reports, and
/// [`crate::view::ViewStore::set_viewport`] swallows.
pub fn viewport_new(width: f64, height: f64) -> Result<Viewport, DrawToolError> {
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
    Ok(Viewport { width, height })
}

impl Viewport {
    pub fn new(width: f64, height: f64) -> Result<Self, DrawToolError> {
        viewport_new(width, height)
    }
}

impl VisibleChunkRange {
    /// True when the rectangle covers at least one chunk.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cx1 < self.cx0 || self.cy1 < self.cy0
    }
}

/// Guard used by [`ViewStore::set_canvas`] so the canvas-size rule has exactly
/// one definition (`constants::require_valid_canvas_size`).
pub(crate) fn canvas_size(width: u32, height: u32) -> Result<(), DrawToolError> {
    require_valid_canvas_size(width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_zoom_one_at_the_origin() {
        let v = View::identity();
        assert_eq!((v.zoom(), v.offset_x(), v.offset_y()), (1.0, 0.0, 0.0));
    }

    #[test]
    fn create_view_matches_new() {
        let a = View::new(2.0, 10.0, -5.0).expect("valid");
        let b = create_view(ViewOptions {
            zoom: Some(2.0),
            offset_x: Some(10.0),
            offset_y: Some(-5.0),
        })
        .expect("valid");
        assert_eq!(a, b);
    }
}
