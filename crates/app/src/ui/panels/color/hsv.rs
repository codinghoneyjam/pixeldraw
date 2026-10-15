//! HSV colour maths for the wheel, kept apart from the widget that draws it.
//!
//! Split out of the parent `color.rs`: `rgb_to_hsv` / `hsv_to_rgb` /
//! `angle_to_hue` are pinned byte-for-byte to the JS originals and are the only
//! part of the colour panel that could be right or wrong on its own, so they
//! get their own file and their own tests.

/// JS `rgbToHsv` verbatim: `d > 0` with exact float equality
/// (`mx === rn`), not an epsilon. Epsilon comparisons shift hue by up to a
/// degree near the sector boundaries.
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let rn = r as f32 / 255.0;
    let gn = g as f32 / 255.0;
    let bn = b as f32 / 255.0;
    let mx = rn.max(gn).max(bn);
    let mn = rn.min(gn).min(bn);
    let d = mx - mn;
    let mut h = 0.0;
    if d > 0.0 {
        if mx == rn {
            h = 60.0 * (((gn - bn) / d) % 6.0);
        } else if mx == gn {
            h = 60.0 * ((bn - rn) / d + 2.0);
        } else {
            h = 60.0 * ((rn - gn) / d + 4.0);
        }
    }
    if h < 0.0 {
        h += 360.0;
    }
    (h, if mx == 0.0 { 0.0 } else { d / mx }, mx)
}

/// JS `hsvToRgb` verbatim: six-sector table, `floor(hue/60)` clamped to 5,
/// `Math.round` per channel.
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let hue = ((h % 360.0) + 360.0) % 360.0;
    let c = v * s;
    let x = c * (1.0 - (((hue / 60.0) % 2.0) - 1.0).abs());
    let m = v - c;
    let sec = (hue / 60.0).floor().clamp(0.0, 5.0) as usize;
    let (r1, g1, b1) = match sec {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// JS `angleToHue`: pointer offset → hue degrees.
pub fn angle_to_hue(dx: f32, dy: f32) -> f32 {
    (dy.atan2(dx).to_degrees() + 450.0) % 360.0
}

/// JS `onRing`: true when the offset lands on the ring band.
pub(crate) fn on_ring(dx: f32, dy: f32) -> bool {
    dx.hypot(dy) >= crate::ui::panels::color::RING_IN - 6.0
}

/// JS `withSlotAlpha`: wheel picks are opaque by construction, so the active
/// slot's alpha is re-attached instead of resetting translucency on drag.
pub(crate) fn with_slot_alpha(active_hex: &str, picked_hex: String) -> String {
    let h = active_hex.trim_start_matches('#');
    if h.len() == 8 {
        format!("{picked_hex}{}", &h[6..8])
    } else {
        picked_hex
    }
}

/// Live wheel result: the picked colour plus whether the gesture ended
/// (JS `pick` fires continuously, `commit` fires exactly once — recent
/// colours are recorded only from the commit).
#[cfg(test)]
mod hsv_tests {
    use super::*;

    #[test]
    fn sectors_match_js() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), (255, 0, 0));
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), (0, 255, 0));
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), (0, 0, 255));
        assert_eq!(hsv_to_rgb(360.0, 1.0, 1.0), (255, 0, 0));
    }

    #[test]
    fn angle_maps_like_js() {
        assert!((angle_to_hue(1.0, 0.0) - 90.0).abs() < 1e-4);
        assert!((angle_to_hue(0.0, -1.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn slot_alpha_is_reapplied() {
        assert_eq!(
            with_slot_alpha("#ff000080", "#00ff00".to_string()),
            "#00ff0080"
        );
        assert_eq!(with_slot_alpha("#ff0000", "#00ff00".to_string()), "#00ff00");
    }

    #[test]
    fn ring_band_matches_js() {
        assert!(on_ring(60.0, 0.0));
        assert!(!on_ring(10.0, 0.0));
    }
}
