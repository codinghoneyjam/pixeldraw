//! Pan and the viewport clamp.

use crate::view::types::{View, from_parts};

pub const CLAMP_MARGIN: f64 = 32.0;

/// `panBy` — offsets shift and nothing else moves. Deliberately unclamped:
/// clamping here as well as in [`ViewStore::set`](crate::view::ViewStore::set)
/// would make a drag non-cancellable mid-gesture.
pub fn pan_by(v: View, dx: f64, dy: f64) -> View {
    from_parts(v.zoom(), v.offset_x() + dx, v.offset_y() + dy)
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
    from_parts(
        v.zoom(),
        clamp_offset(v.offset_x(), margin - canvas_w * v.zoom(), vw - margin),
        clamp_offset(v.offset_y(), margin - canvas_h * v.zoom(), vh - margin),
    )
}

/// `Math.min(Math.max(val, lo), hi)` — in that order.
///
/// The order is load-bearing. When the range inverts (`hi < lo`, which happens
/// once `canvas * zoom` drops below `2 * margin - vw`) `max` clamps to `lo` first
/// and `min` drags the answer back to `hi`; swapping the calls pins it to `lo`.
pub fn clamp_offset(val: f64, lo: f64, hi: f64) -> f64 {
    val.max(lo).min(hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::types::View;

    #[test]
    fn pan_by_does_not_clamp() {
        let v = View::new(2.0, 0.0, 0.0).expect("valid");
        let p = pan_by(v, 5.0, -7.0);
        assert_eq!((p.offset_x(), p.offset_y()), (5.0, -7.0));
        assert_eq!(p.zoom(), 2.0);
    }

    #[test]
    fn clamp_range_inversion_pins_to_hi_not_lo() {
        // A 32px canvas at 0.25x in a 50px viewport: lo = 24 - 8 = 16,
        // hi = 50 - 32 = 18... and with margin the range still inverts.
        let v = View::new(0.25, 0.0, 0.0).expect("valid");
        let c = clamp_view(v, 32.0, 32.0, 50.0, 50.0);
        // lo = 32 - 8 = 24, hi = 50 - 32 = 18 — inverted, so min wins.
        assert_eq!(c.offset_x(), 18.0);
        assert_eq!(c.offset_y(), 18.0);
    }

    #[test]
    fn custom_margin_is_honoured() {
        let v = View::new(1.0, -100.0, 0.0).expect("valid");
        let c = clamp_view_with_margin(v, 100.0, 100.0, 400.0, 400.0, 10.0);
        assert_eq!(c.offset_x(), 10.0 - 100.0);
    }
}
