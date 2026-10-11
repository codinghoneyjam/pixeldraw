//! Fit-to-viewport and actual-size, both centring through the same rounding.

use crate::view::geom::round_half_up;
use crate::view::types::{View, from_parts};
use crate::view::zoom::largest_rung_at_most;

pub const FIT_PADDING: f64 = 16.0;

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
    from_parts(
        zoom,
        round_half_up((vw - canvas_w * zoom) / 2.0),
        round_half_up((vh - canvas_h * zoom) / 2.0),
    )
}

/// `actualSizeView` — zoom 1, centred.
///
/// No ladder involvement and no clamping: the store runs the result through
/// `set`, which clamps. `Math.round`'s half-up rule is what puts a canvas larger
/// than its viewport at a *negative* offset.
pub fn actual_size_view(canvas_w: f64, canvas_h: f64, vw: f64, vh: f64) -> View {
    from_parts(
        1.0,
        round_half_up((vw - canvas_w) / 2.0),
        round_half_up((vh - canvas_h) / 2.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_falls_back_to_the_smallest_rung() {
        // 2048 wide in a 200px viewport: even 0.25x does not fit.
        let v = fit_view(2048.0, 2048.0, 200.0, 200.0);
        assert_eq!(v.zoom(), 0.25);
    }

    #[test]
    fn fit_takes_the_largest_rung_that_fits() {
        let v = fit_view(512.0, 512.0, 800.0, 600.0);
        assert_eq!(v.zoom(), 1.0);
    }

    #[test]
    fn centring_rounds_halves_up() {
        // 5px canvas in a 2px viewport: (2-5)/2 = -1.5 -> Math.round -> -1.
        let v = actual_size_view(5.0, 5.0, 2.0, 2.0);
        assert_eq!(v.zoom(), 1.0);
        assert_eq!(v.offset_x(), -1.0);
        assert_eq!(v.offset_y(), -1.0);
    }
}
