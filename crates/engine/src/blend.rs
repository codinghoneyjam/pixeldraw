//! Shared blend math (docs/contract.md §1-1, §2).
//!
//! Ported 1:1 from `src/core/blend.js`. This is the **single** implementation of
//! how two RGBA values combine. Four paths consume it in the JS tree — display
//! compositing, PNG flatten, eyedropper sampling and merge-down — and they must
//! never disagree, because the same pixel would otherwise show one colour on
//! screen, another in the file and a third under the eyedropper.
//!
//! **Float parity.** Every formula runs in `f64` and rounds exactly once with
//! [`rnd`]. Rust `f64` arithmetic is IEEE 754 like JavaScript's, so keeping the
//! operation order identical keeps the bytes identical. Reordering a line here
//! changes pixels.

use crate::pixel::Rgba;

/// `Math.floor(v + 0.5)` — the one rounding rule in the engine.
#[inline]
pub fn rnd(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// The six layer blend modes, in the order the UI lists them.
pub const BLEND_MODES: [&str; 6] = [
    "normal", "multiply", "screen", "overlay", "darken", "lighten",
];

/// Membership test, mirroring `isBlendMode`.
#[inline]
pub fn is_blend_mode(mode: &str) -> bool {
    BLEND_MODES.contains(&mode)
}

/// Blend one straight (unpremultiplied) channel pair, 0..255 integer math.
///
/// The formulas are literally the ones table-stated in contract §1-1. The
/// overlay seam at `dc == 128` is the documented formula's own behaviour: 128 is
/// not < 128, so it takes the screen branch and lands 1 LSB off the ideal
/// midpoint. That is the formula's property, not a bug.
#[inline]
fn blend_channel(mode: &str, sc: u8, dc: u8) -> u8 {
    let (sc, dc) = (sc as f64, dc as f64);
    let v = match mode {
        "multiply" => rnd(sc * dc / 255.0),
        "screen" => rnd(sc + dc - sc * dc / 255.0),
        "overlay" => {
            if dc < 128.0 {
                rnd(2.0 * sc * dc / 255.0)
            } else {
                rnd(255.0 - 2.0 * (255.0 - sc) * (255.0 - dc) / 255.0)
            }
        }
        "darken" => sc.min(dc),
        "lighten" => sc.max(dc),
        _ => sc,
    };
    v as u8
}

/// Mode-aware source-over, the only place alpha is combined.
///
/// Alpha handling is deliberately identical to a plain source-over: only the RGB
/// carried forward changes. `da === 0` skips the blend entirely (rule 3) — the
/// backdrop colour is undefined, so multiplying against black would turn a
/// transparent region black instead of leaving the source untouched.
///
/// `mode` is `""`/`"normal"` for a plain layer.
pub fn over_blend(dst: Rgba, src: Rgba, opacity: f64, mode: &str) -> Rgba {
    let sa = (src[3] as f64 / 255.0) * opacity;
    if sa == 0.0 {
        return dst;
    }
    let da = dst[3] as f64 / 255.0;
    let oa = sa + da * (1.0 - sa);
    let inv = 1.0 - sa;
    // A mode of "normal" (or an unknown string) must produce exactly what a
    // plain source-over produces, byte for byte, so the fast paths upstream
    // stay honest.
    let blend = mode != "normal" && !mode.is_empty() && da > 0.0;
    let c = |i: usize| -> u8 {
        let v = if blend {
            rnd((blend_channel(mode, src[i], dst[i]) as f64 * sa + dst[i] as f64 * da * inv) / oa)
        } else {
            rnd((src[i] as f64 * sa + dst[i] as f64 * da * inv) / oa)
        };
        v as u8
    };
    let a = rnd(oa * 255.0) as u8;
    if a == 0 {
        return [0, 0, 0, 0];
    }
    [c(0), c(1), c(2), a]
}

/// `normal`-mode alias for callers with no layer concept.
#[inline]
pub fn over(dst: Rgba, src: Rgba, opacity: f64) -> Rgba {
    over_blend(dst, src, opacity, "normal")
}

/// True when the mode needs the blend math rather than plain source-over.
#[inline]
pub fn mode_needs_blend(mode: &str) -> bool {
    mode != "normal" && !mode.is_empty() && is_blend_mode(mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    // <META - ROLE : the composite group of raster_golden.json, all 8 cases | L118-140>
    #[test]
    fn golden_composite_cases() {
        let cases: [(Rgba, Rgba, f64, Rgba); 8] = [
            ([0, 0, 0, 0], [255, 0, 0, 255], 1.0, [255, 0, 0, 255]),
            ([10, 20, 30, 255], [255, 0, 0, 255], 1.0, [255, 0, 0, 255]),
            (
                [0, 0, 0, 255],
                [255, 255, 255, 255],
                0.5,
                [128, 128, 128, 255],
            ),
            (
                [0, 0, 0, 0],
                [255, 255, 255, 255],
                0.5,
                [255, 255, 255, 128],
            ),
            ([255, 0, 0, 255], [0, 0, 255, 255], 0.25, [191, 0, 64, 255]),
            ([0, 0, 0, 0], [9, 9, 9, 0], 1.0, [0, 0, 0, 0]),
            (
                [200, 100, 50, 128],
                [10, 200, 90, 255],
                0.3,
                [112, 146, 68, 166],
            ),
            ([1, 2, 3, 255], [255, 255, 255, 255], 0.0, [1, 2, 3, 255]),
        ];
        for (dst, src, op, want) in cases {
            assert_eq!(over(dst, src, op), want, "over({dst:?}, {src:?}, {op})");
        }
    }

    #[test]
    fn source_alpha_zero_returns_destination() {
        assert_eq!(over([9, 9, 9, 255], [1, 2, 3, 0], 1.0), [9, 9, 9, 255]);
        assert_eq!(over([9, 9, 9, 0], [1, 2, 3, 0], 1.0), [9, 9, 9, 0]);
    }

    #[test]
    fn zero_result_alpha_normalises() {
        // Any result whose alpha rounds to 0 must come back fully transparent.
        let got = over([0, 0, 0, 0], [10, 20, 30, 1], 0.0);
        assert_eq!(got[3], 0);
        if got[3] == 0 {
            assert_eq!(got, [0, 0, 0, 0]);
        }
    }

    #[test]
    fn rounding_rule() {
        assert_eq!(rnd(1.5), 2.0);
        assert_eq!(rnd(0.5), 1.0);
        assert_eq!(rnd(2.4), 2.0);
        assert_eq!(rnd(2.6), 3.0);
    }

    #[test]
    fn blend_modes_frozen() {
        assert_eq!(
            BLEND_MODES,
            [
                "normal", "multiply", "screen", "overlay", "darken", "lighten"
            ]
        );
        assert!(is_blend_mode("multiply"));
        assert!(!is_blend_mode("nope"));
    }

    #[test]
    fn normal_mode_is_byte_identical_to_plain_over() {
        for mode in BLEND_MODES {
            let via_blend = over_blend([10, 20, 30, 200], [200, 100, 50, 128], 0.5, mode);
            if mode == "normal" {
                assert_eq!(via_blend, over([10, 20, 30, 200], [200, 100, 50, 128], 0.5));
            }
        }
    }

    #[test]
    fn backdrop_alpha_zero_skips_the_blend() {
        // multiply against an undefined backdrop must not turn black.
        let got = over_blend([0, 0, 0, 0], [255, 0, 0, 255], 1.0, "multiply");
        assert_eq!(got, [255, 0, 0, 255]);
    }
}
