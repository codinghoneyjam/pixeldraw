//! Screen ↔ canvas mapping and the two rounding rules the JS wrote inline.

use crate::view::types::View;

/// Re-exported so `crate::view::geom::…` keeps resolving; the canonical home is
/// [`crate::view::types`].
pub use crate::view::types::{CanvasPoint, Pixel, ScreenPoint};

/// `screenToCanvas` — `(sx - offsetX) / zoom`, float in and float out.
///
/// No rounding: [`pixel_at`] is where the floor happens, and it is the only
/// caller that needs an integer.
pub fn screen_to_canvas(v: View, sx: f64, sy: f64) -> CanvasPoint {
    CanvasPoint {
        x: (sx - v.offset_x()) / v.zoom(),
        y: (sy - v.offset_y()) / v.zoom(),
    }
}

/// `canvasToScreen` — `offset + x * zoom`.
///
/// Multiplication first, exactly as the JS wrote it: that ordering is what
/// makes this a bit-exact inverse of [`screen_to_canvas`] for the values
/// `zoom_at` produces. Reordering it costs nothing and drifts.
pub fn canvas_to_screen(v: View, x: f64, y: f64) -> ScreenPoint {
    ScreenPoint {
        x: v.offset_x() + x * v.zoom(),
        y: v.offset_y() + y * v.zoom(),
    }
}

/// `pixelAt` — `screenToCanvas` plus `Math.floor`.
///
/// Never clamps: a pointer left of the canvas reads `-1`, not `0`, which is what
/// the off-canvas drawing tests rely on.
pub fn pixel_at(v: View, sx: f64, sy: f64) -> Pixel {
    let p = screen_to_canvas(v, sx, sy);
    Pixel {
        x: floor_i64(p.x),
        y: floor_i64(p.y),
    }
}

/// `Math.floor` to an integer, in one place.
///
/// A bare `as i64` truncates toward zero, which would turn `-0.5` into `0`
/// instead of `-1` and quietly mirror the canvas across its left edge.
#[inline]
pub fn floor_i64(v: f64) -> i64 {
    v.floor() as i64
}

/// `Math.round`, which is **not** `f64::round`.
///
/// JS rounds halves toward +∞ (`Math.round(-2.5) === -2`); Rust rounds them away
/// from zero. They disagree only on the `.5` boundary of a negative value — when
/// the canvas is larger than its viewport — which is the only time [`fit`](crate::view::fit_view)
/// and [`actual_size_view`] centre something negative. `Math.floor(v + 0.5)` is
/// the bit-identical port.
#[inline]
pub fn round_half_up(v: f64) -> f64 {
    (v + 0.5).floor()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::types::ViewOptions;
    use crate::view::types::create_view;

    fn v(zoom: f64, ox: f64, oy: f64) -> View {
        View::new(zoom, ox, oy).expect("valid")
    }

    #[test]
    fn mapping_is_the_plain_formula() {
        let view = v(4.0, 100.0, 50.0);
        let p = screen_to_canvas(view, 200.0, 150.0);
        assert_eq!((p.x, p.y), (25.0, 25.0));
        let s = canvas_to_screen(view, 25.0, 25.0);
        assert_eq!((s.x, s.y), (200.0, 150.0));
    }

    #[test]
    fn create_view_defaults_like_the_nullish_operator() {
        let d = create_view(ViewOptions::default()).expect("valid");
        assert_eq!((d.zoom(), d.offset_x(), d.offset_y()), (1.0, 0.0, 0.0));
    }

    #[test]
    fn pixel_at_never_clamps() {
        let view = v(1.0, 0.0, 0.0);
        assert_eq!(pixel_at(view, -0.5, -0.5).x, -1);
        assert_eq!(pixel_at(view, -0.5, -0.5).y, -1);
    }

    #[test]
    fn rounding_helpers_are_the_two_js_rules() {
        // floor, not truncation.
        assert_eq!(floor_i64(-0.5), -1);
        // Math.round, not f64::round.
        assert_eq!(round_half_up(-2.5), -2.0);
        assert_eq!(round_half_up(2.5), 3.0);
    }
}
