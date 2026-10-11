//! `ViewStore` — the mutable holder plus what a mutating call produced.

use crate::view::clamp::{clamp_view, pan_by};
use crate::view::fit::{actual_size_view, fit_view};
use crate::view::geom::{Pixel, ScreenPoint, canvas_to_screen, pixel_at};
use crate::view::types::{View, Viewport, canvas_size, viewport_new};
use crate::view::zoom::{step_zoom, zoom_at};

/// What a mutating [`ViewStore`] call produced.
///
/// The JS store had no return value: it committed and fanned out to subscribers,
/// so a caller had to re-read the store to learn what happened. This is the
/// committed view plus a `changed` flag, which lets the app skip the redraw when
/// nothing moved — the JS repainted unconditionally.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewChange {
    pub view: View,
    pub changed: bool,
}

impl ViewChange {
    /// The `events::EVENTS` names a view change should emit — empty, on purpose.
    ///
    /// `view_store.js` never dispatched one of the seven catalog signals; its
    /// subscribers were the renderer's frame scheduler and the status bar's zoom
    /// readout, wired directly in `ui/app.js`. So the catalog is deliberately not
    /// extended: `changed == true` *is* the signal.
    pub fn events(&self) -> &'static [&'static str] {
        &[]
    }
}

/// A [`View`], the viewport it was last clamped against, and the canvas dims it
/// was clamped to.
///
/// Three differences from the JS, none observable in the store's own behaviour:
/// the DOM read became [`ViewStore::set_viewport`]; `session.doc` became
/// [`ViewStore::set_canvas`]; and there is no `subscribe` (see
/// [`ViewChange::events`]).
pub struct ViewStore {
    view: View,
    viewport: Viewport,
    canvas: Option<(u32, u32)>,
}

impl Default for ViewStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewStore {
    pub fn new() -> Self {
        Self::with_viewport(Viewport::DEFAULT)
    }

    pub fn with_viewport(viewport: Viewport) -> Self {
        Self {
            view: View::identity(),
            viewport,
            canvas: None,
        }
    }

    #[inline]
    pub fn view(&self) -> View {
        self.view
    }

    #[inline]
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub fn canvas(&self) -> Option<(u32, u32)> {
        self.canvas
    }

    /// Attach the document's canvas, replacing any previous one.
    pub fn set_canvas(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<(), crate::errors::DrawToolError> {
        canvas_size(width, height)?;
        self.canvas = Some((width, height));
        Ok(())
    }

    /// Detach — back to the JS's "no document", where `set` stops clamping and
    /// `fit`/`actual` become no-ops.
    pub fn clear_canvas(&mut self) {
        self.canvas = None;
    }

    /// `set(v)` — clamp, then commit.
    ///
    /// The clamp is a no-op with no canvas attached, exactly as the JS's
    /// `if (!doc) return { ...v }`: until the app attaches one on
    /// `document-replaced`, the store holds whatever it was handed.
    pub fn set(&mut self, v: View) -> ViewChange {
        let clamped = match self.canvas {
            Some((w, h)) => clamp_view(
                v,
                f64::from(w),
                f64::from(h),
                self.viewport.width,
                self.viewport.height,
            ),
            None => v,
        };
        self.commit(clamped)
    }

    /// The anchor every store zoom keeps fixed.
    ///
    /// The *viewport* centre, not the canvas centre, and it reads the fallback
    /// viewport before a resize is reported — so the anchor is 400×300 initially
    /// regardless of the real window size.
    pub fn center(&self) -> ScreenPoint {
        ScreenPoint {
            x: self.viewport.width / 2.0,
            y: self.viewport.height / 2.0,
        }
    }

    /// `zoomTo(z)` — zoom about the viewport centre, clamped.
    ///
    /// Rejects `!finite || z <= 0` by *ignoring the call*, as the JS did, even
    /// though [`zoom_at`] raises for the same values. The pure function is
    /// strict and the store is forgiving, reproduced deliberately because
    /// `ui/actions/action_map.js` funnels user input through here.
    pub fn zoom_to(&mut self, z: f64) -> ViewChange {
        if !z.is_finite() || z <= 0.0 {
            return self.unchanged();
        }
        let c = self.center();
        match zoom_at(self.view, c.x, c.y, z) {
            // The Err arm is unreachable: `z` was just validated. A match rather
            // than `unwrap_or` so there is no panic path outside tests.
            Ok(next) => self.set(next),
            Err(_) => self.unchanged(),
        }
    }

    /// One rung up the ladder about the viewport centre. Sticky at 32×.
    pub fn zoom_in(&mut self) -> ViewChange {
        self.zoom_to(step_zoom(self.view.zoom(), 1))
    }

    /// One rung down, sticky at 0.25×.
    pub fn zoom_out(&mut self) -> ViewChange {
        self.zoom_to(step_zoom(self.view.zoom(), -1))
    }

    /// `panBy(dx, dy)`, clamped through `set`.
    ///
    /// The free [`pan_by`] does not clamp; this does, because a hand drag must
    /// not be able to lose the canvas off screen.
    pub fn pan_by(&mut self, dx: f64, dy: f64) -> ViewChange {
        self.set(pan_by(self.view, dx, dy))
    }

    /// Largest rung that fits the attached canvas, centred, clamped.
    pub fn fit(&mut self) -> ViewChange {
        let Some((w, h)) = self.canvas else {
            return self.unchanged();
        };
        let vp = self.viewport;
        self.set(fit_view(f64::from(w), f64::from(h), vp.width, vp.height))
    }

    /// Zoom 1, centred, clamped. No-op with no canvas attached.
    pub fn actual_size(&mut self) -> ViewChange {
        let Some((w, h)) = self.canvas else {
            return self.unchanged();
        };
        let vp = self.viewport;
        self.set(actual_size_view(
            f64::from(w),
            f64::from(h),
            vp.width,
            vp.height,
        ))
    }

    /// Report a new viewport size and re-clamp.
    ///
    /// This is `window.addEventListener("resize", …)` from `ui/app.js`, where the
    /// store re-read the DOM and committed an unchanged view purely to re-run the
    /// clamp. A size the host reported badly (zero, negative, non-finite) falls
    /// back to [`Viewport::DEFAULT`] instead of raising — the JS's `try {} catch {}`
    /// around `host.clientWidth`, kept word for word, so growing the window can
    /// pull the canvas back in from off screen.
    pub fn set_viewport(&mut self, width: f64, height: f64) -> ViewChange {
        self.viewport = viewport_new(width, height).unwrap_or(Viewport::DEFAULT);
        self.set(self.view)
    }

    /// The chunk rectangle currently visible, or `None`.
    pub fn visible_chunk_range(&self) -> Option<crate::view::VisibleChunkRange> {
        let (w, h) = self.canvas?;
        crate::view::visible_chunk_range(
            self.view,
            f64::from(w),
            f64::from(h),
            self.viewport.width,
            self.viewport.height,
        )
    }

    /// Where a screen point lands in canvas pixels.
    pub fn pixel_at(&self, sx: f64, sy: f64) -> Pixel {
        pixel_at(self.view, sx, sy)
    }

    /// Where a canvas point lands on screen.
    pub fn canvas_to_screen(&self, x: f64, y: f64) -> ScreenPoint {
        canvas_to_screen(self.view, x, y)
    }

    fn unchanged(&self) -> ViewChange {
        ViewChange {
            view: self.view,
            changed: false,
        }
    }

    fn commit(&mut self, next: View) -> ViewChange {
        let changed = next != self.view;
        self.view = next;
        ViewChange {
            view: next,
            changed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::types::View;

    fn store() -> ViewStore {
        ViewStore::new()
    }

    #[test]
    fn defaults_to_800x600_and_zoom_1() {
        let s = store();
        assert_eq!((s.viewport().width, s.viewport().height), (800.0, 600.0));
        assert_eq!(s.view().zoom(), 1.0);
        assert!(s.canvas().is_none());
    }

    #[test]
    fn set_clamps_against_the_attached_canvas() {
        let mut s = store();
        s.set_canvas(64, 64).expect("valid size");
        let far = View::new(1.0, 5000.0, 5000.0).expect("valid");
        let change = s.set(far);
        assert!(change.changed);
        assert_eq!(change.view.offset_x(), 800.0 - 32.0);
    }

    #[test]
    fn without_a_canvas_set_does_not_clamp() {
        let mut s = store();
        let far = View::new(1.0, 5000.0, 5000.0).expect("valid");
        let change = s.set(far);
        assert!(change.changed);
        assert_eq!(change.view.offset_x(), 5000.0);
    }

    #[test]
    fn zoom_to_ignores_bad_zoom_like_the_js() {
        let mut s = store();
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(!s.zoom_to(bad).changed, "{bad} must be ignored");
        }
    }

    #[test]
    fn zoom_in_out_walks_the_ladder_from_every_rung() {
        let mut s = store();
        s.set_canvas(512, 512).expect("valid size");
        let c = s.zoom_to(0.25);
        assert_eq!(c.view.zoom(), 0.25);
        for expected in [0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0] {
            let c = s.zoom_in();
            assert_eq!(c.view.zoom(), expected, "from {expected}");
        }
        assert_eq!(s.zoom_in().view.zoom(), 32.0);
    }

    #[test]
    fn fit_and_actual_are_noops_without_a_canvas() {
        let mut s = store();
        assert!(!s.fit().changed);
        assert!(!s.actual_size().changed);
    }

    #[test]
    fn fit_and_actual_recentre_with_a_canvas() {
        let mut s = store();
        s.set_canvas(512, 512).expect("valid size");
        let f = s.fit();
        assert!(f.changed, "from the identity view this is a real move");
        assert_eq!(f.view.zoom(), 1.0);
        // fit() centred the canvas, and actual_size() lands on the same view
        // (zoom 1, centred), so it correctly reports no movement.
        let a = s.actual_size();
        assert!(!a.changed, "fit had already reached the actual-size view");
        assert_eq!(a.view.zoom(), 1.0);
    }

    #[test]
    fn pan_by_clamps_but_the_free_function_does_not() {
        let mut s = store();
        s.set_canvas(64, 64).expect("valid size");
        let c = s.pan_by(-5000.0, 0.0);
        assert!(c.view.offset_x() >= 32.0 - 64.0);
    }

    #[test]
    fn set_viewport_reclamps_like_the_resize_handler() {
        let mut s = store();
        s.set_canvas(512, 512).expect("valid size");
        let z = s.zoom_to(4.0);
        assert!(z.changed);
        // Growing the viewport leaves the clamped view where it was; shrinking
        // it re-clamps and therefore reports a change. Either way the store must
        // not panic and must stay finite.
        let small = s.set_viewport(200.0, 200.0);
        assert!(small.view.offset_x().is_finite());
        let big = s.set_viewport(4000.0, 4000.0);
        assert!(big.view.offset_x().is_finite());
    }

    #[test]
    fn set_viewport_falls_back_for_bad_sizes() {
        let mut s = store();
        for bad in [(0.0, 0.0), (-1.0, 100.0), (f64::NAN, 1.0)] {
            s.set_viewport(bad.0, bad.1);
        }
        assert_eq!((s.viewport().width, s.viewport().height), (800.0, 600.0));
    }

    #[test]
    fn events_names_nothing_outside_the_frozen_catalog() {
        let c = ViewChange {
            view: View::identity(),
            changed: true,
        };
        for name in c.events() {
            assert!(
                crate::events::EVENTS.contains(name),
                "{name} must be a catalog signal"
            );
        }
    }

    #[test]
    fn set_canvas_rejects_bad_sizes() {
        let mut s = store();
        assert!(s.set_canvas(31, 32).is_err());
        assert!(s.set_canvas(33, 32).is_err());
        assert!(s.set_canvas(64, 64).is_ok());
    }
}
