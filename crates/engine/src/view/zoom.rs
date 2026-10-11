//! Zoom: the frozen ladder walk and anchored re-zoom.

use crate::constants::ZOOM_LEVELS;
use crate::errors::DrawToolError;
use crate::view::types::{View, from_parts, require_finite};

pub const ZOOM_MIN_INDEX: usize = 0;
pub const ZOOM_MAX_INDEX: usize = ZOOM_LEVELS.len() - 1;

/// `zoomAt` — re-zoom while holding the anchor `(ax, ay)` fixed.
///
/// `k = newZoom / zoom` is computed once and reused for both axes. Factoring it
/// differently changes the last bits, and the "anchor stays fixed" invariant in
/// `tools/view_check.mjs` then fails at the 1e-9 tolerance instead of exactly.
pub fn zoom_at(v: View, ax: f64, ay: f64, new_zoom: f64) -> Result<View, DrawToolError> {
    require_finite(new_zoom, "zoom")?;
    if new_zoom <= 0.0 {
        return Err(DrawToolError::out_of_range("newZoom must be > 0"));
    }
    let k = new_zoom / v.zoom();
    Ok(from_parts(
        new_zoom,
        ax - (ax - v.offset_x()) * k,
        ay - (ay - v.offset_y()) * k,
    ))
}

/// `stepZoom` — one rung of the frozen ladder.
///
/// Both fallbacks are "sticky at the wrong end": `step_zoom(64, 1) === 32` and
/// `step_zoom(0.1, -1) === 0.25`, because `.find` matches nothing and the JS
/// fell back to the last/first rung. A view already outside the ladder therefore
/// moves toward it rather than staying put. `dir == 0` is a pure identity.
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

/// `[...ZOOM_LEVELS].reverse().find(l => l <= want) ?? ZOOM_LEVELS[0]`.
///
/// The fallback is deliberate: the ladder is a frozen whitelist, not a
/// continuous scale, so a canvas bigger than even 0.25× of the viewport stops
/// there instead of shrinking further.
pub fn largest_rung_at_most(want: f64) -> f64 {
    ZOOM_LEVELS
        .iter()
        .rev()
        .copied()
        .find(|&level| level <= want)
        .unwrap_or(ZOOM_LEVELS[ZOOM_MIN_INDEX])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::types::View;

    #[test]
    fn ladder_order_is_load_bearing_to_the_index_walk() {
        for pair in ZOOM_LEVELS.windows(2) {
            assert!(pair[0] < pair[1], "ladder must be strictly increasing");
        }
    }

    #[test]
    fn step_zoom_walks_all_twelve_rungs_both_ways() {
        let mut z = ZOOM_LEVELS[0];
        for expected in ZOOM_LEVELS.iter().skip(1) {
            z = step_zoom(z, 1);
            assert_eq!(z, *expected);
        }
        let mut z = ZOOM_LEVELS[ZOOM_MAX_INDEX];
        for expected in ZOOM_LEVELS.iter().rev().skip(1) {
            z = step_zoom(z, -1);
            assert_eq!(z, *expected);
        }
    }

    #[test]
    fn step_zoom_sticks_at_the_wrong_end() {
        assert_eq!(step_zoom(64.0, 1), 32.0);
        assert_eq!(step_zoom(0.1, -1), 0.25);
        assert_eq!(step_zoom(3.0, 0), 3.0);
    }

    #[test]
    fn zoom_at_keeps_the_anchor_fixed() {
        let v = View::new(1.0, 10.0, 20.0).expect("valid");
        let next = zoom_at(v, 40.0, 50.0, 4.0).expect("valid");
        let before = crate::view::geom::canvas_to_screen(v, 30.0, 30.0);
        let after = crate::view::geom::canvas_to_screen(next, 30.0, 30.0);
        assert!((before.x - after.x).abs() < 1e-9);
        assert!((before.y - after.y).abs() < 1e-9);
    }

    #[test]
    fn zoom_at_rejects_nothing_but_a_bad_zoom() {
        let v = View::identity();
        assert!(zoom_at(v, 0.0, 0.0, 0.0).is_err());
        assert!(zoom_at(v, 0.0, 0.0, f64::NAN).is_err());
        assert!(zoom_at(v, 0.0, 0.0, -1.0).is_err());
        assert!(zoom_at(v, 0.0, 0.0, 2.0).is_ok());
    }
}
