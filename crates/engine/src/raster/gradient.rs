//! Radial multi-stop gradient raster (the shockwave-ring reproduction).
//!
//! Ported from `src/core/raster/gradient.js`. The output is per-pixel packed
//! RGBA, not a 1-bit mask — a gradient carries colour, so callers write the
//! non-zero pixels through a [`PixelWriter`](crate::chunkstore::PixelWriter).
//!
//! **Truncation, not rounding.** The legacy shockwave baker used `int()`
//! truncation for alpha/luminance, and the T-5 gate is alpha parity against it.
//! Round-half-up disagrees with truncation on roughly half of all fractional
//! pixels, so [`grad_trunc`] is the spec rule here and must keep matching the
//! oracle's `floor()` in `tools/gen_fixtures.py`.

use crate::errors::{DrawToolError, EngineResult};
use crate::pixel::{Rgba, pack_rgba};
use crate::raster::mask::assert_mask_size;

/// `Math.floor` — the baker's truncation rule.
#[inline]
fn grad_trunc(v: f64) -> u8 {
    v.floor().clamp(0.0, 255.0) as u8
}

#[inline]
fn clamp01(t: f64) -> f64 {
    t.clamp(0.0, 1.0)
}

/// One gradient stop: `[t, r, g, b, a]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stop {
    pub t: f64,
    pub rgba: Rgba,
}

impl Stop {
    pub const fn new(t: f64, r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            t,
            rgba: [r, g, b, a],
        }
    }
}

/// Interpolate one RGBA tuple across sorted stops at `t`.
///
/// Stops are expected sorted by `t`; the caller sorts (see [`RadialGradient`]).
fn sample_stops(sorted: &[Stop], t: f64) -> Rgba {
    let first = sorted[0];
    let last = sorted[sorted.len() - 1];
    if t <= first.t {
        return first.rgba;
    }
    if t >= last.t {
        return last.rgba;
    }
    for i in 0..sorted.len() - 1 {
        let a = sorted[i];
        let b = sorted[i + 1];
        if t < a.t || t > b.t {
            continue;
        }
        let f = if b.t == a.t {
            0.0
        } else {
            (t - a.t) / (b.t - a.t)
        };
        // The JS returned this array as the block's tail expression; say so.
        return [
            grad_trunc(a.rgba[0] as f64 + (b.rgba[0] as f64 - a.rgba[0] as f64) * f),
            grad_trunc(a.rgba[1] as f64 + (b.rgba[1] as f64 - a.rgba[1] as f64) * f),
            grad_trunc(a.rgba[2] as f64 + (b.rgba[2] as f64 - a.rgba[2] as f64) * f),
            grad_trunc(a.rgba[3] as f64 + (b.rgba[3] as f64 - a.rgba[3] as f64) * f),
        ];
    }
    // Unreachable for a sorted list with t strictly inside, but the JS fell
    // through to this and so must we.
    last.rgba
}

/// A packed-RGBA raster: `{ w, h, data }` with `data` a `u32` per pixel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradientMask {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u32>,
}

impl GradientMask {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            data: vec![0u32; w * h],
        }
    }

    /// The RGBA bytes of one pixel, `None` when the pixel was left empty.
    pub fn rgba(&self, x: usize, y: usize) -> Option<Rgba> {
        let p = self.data[y * self.w + x];
        if p == 0 {
            return None;
        }
        Some([
            (p & 0xff) as u8,
            ((p >> 8) & 0xff) as u8,
            ((p >> 16) & 0xff) as u8,
            ((p >> 24) & 0xff) as u8,
        ])
    }
}

/// The gradient parameters.
#[derive(Debug, Clone)]
pub struct RadialGradient {
    pub cx: f64,
    pub cy: f64,
    pub r0: f64,
    pub r1: f64,
    pub stops: Vec<Stop>,
    pub w: usize,
    pub h: usize,
}

/// Radial gradient over a `w × h` canvas.
///
/// Multi-stop profiles may be non-monotonic (transparent → peak → transparent
/// ring), which is exactly the shockwave case. Alpha-0 pixels stay 0 so callers
/// can skip them.
pub fn radial_gradient_mask(g: &RadialGradient) -> EngineResult<GradientMask> {
    assert_mask_size(g.w, g.h)?;
    if g.stops.is_empty() {
        return Err(DrawToolError::invalid_state(
            "radialGradientMask needs at least one stop",
        ));
    }
    let mut sorted: Vec<Stop> = g.stops.clone();
    // Stable by t; the JS `Array.prototype.sort` comparator was `a[0] - b[0]`
    // with no tie-break, and V8's sort is stable, so `sort_by` matches it.
    sorted.sort_by(|a, b| a.t.total_cmp(&b.t));
    let mut m = GradientMask::new(g.w, g.h);
    let span = g.r1 - g.r0;
    for y in 0..g.h {
        for x in 0..g.w {
            let d = ((x as f64 - g.cx).powi(2) + (y as f64 - g.cy).powi(2)).sqrt();
            let t = if span <= 0.0 {
                if d <= g.r0 { 0.0 } else { 1.0 }
            } else {
                clamp01((d - g.r0) / span)
            };
            let c = sample_stops(&sorted, t);
            if c[3] > 0 {
                m.data[y * g.w + x] = pack_rgba(c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64);
            }
        }
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{group, usize_field};

    // <META - ROLE : golden gradient group, 2 cases | L130-150>
    #[test]
    fn golden_gradient_cases() {
        for case in group("gradient") {
            let g = RadialGradient {
                cx: crate::fixtures::f64_field(case, "cx"),
                cy: crate::fixtures::f64_field(case, "cy"),
                r0: crate::fixtures::f64_field(case, "r0"),
                r1: crate::fixtures::f64_field(case, "r1"),
                stops: case["stops"]
                    .as_array()
                    .expect("stops")
                    .iter()
                    .map(|s| {
                        let a = s.as_array().expect("stop array");
                        Stop {
                            t: a[0].as_f64().unwrap(),
                            rgba: [
                                a[1].as_u64().unwrap() as u8,
                                a[2].as_u64().unwrap() as u8,
                                a[3].as_u64().unwrap() as u8,
                                a[4].as_u64().unwrap() as u8,
                            ],
                        }
                    })
                    .collect(),
                w: usize_field(case, "w"),
                h: usize_field(case, "h"),
            };
            let got = radial_gradient_mask(&g).expect("valid size");
            let want = case["rgba"].as_array().expect("rgba rows");
            let mut mismatches = 0;
            for (y, row) in want.iter().enumerate() {
                let row = row.as_array().expect("rgba row");
                for (x, px) in row.iter().enumerate() {
                    let p = px.as_array().expect("rgba pixel");
                    let want_px: Rgba = [
                        p[0].as_u64().unwrap() as u8,
                        p[1].as_u64().unwrap() as u8,
                        p[2].as_u64().unwrap() as u8,
                        p[3].as_u64().unwrap() as u8,
                    ];
                    let got_px = got.rgba(x, y).unwrap_or([0, 0, 0, 0]);
                    if got_px != want_px {
                        mismatches += 1;
                    }
                }
            }
            assert_eq!(mismatches, 0, "gradient {} pixel mismatches", case["name"]);
        }
    }

    #[test]
    fn truncation_beats_rounding_here() {
        // The baker used int() truncation; half-up would differ on this value.
        assert_eq!(grad_trunc(127.9), 127);
        assert_eq!(grad_trunc(0.9), 0);
        assert_eq!(grad_trunc(255.0), 255);
    }

    #[test]
    fn clamped_span_edge_cases() {
        // span <= 0 means a hard step at r0.
        let g = RadialGradient {
            cx: 1.0,
            cy: 1.0,
            r0: 0.0,
            r1: 0.0,
            stops: vec![
                Stop::new(0.0, 255, 0, 0, 255),
                Stop::new(1.0, 0, 0, 255, 255),
            ],
            w: 3,
            h: 3,
        };
        let m = radial_gradient_mask(&g).expect("valid");
        assert_eq!(m.rgba(1, 1), Some([255, 0, 0, 255]));
        assert_eq!(m.rgba(0, 0), Some([0, 0, 255, 255]));
    }

    #[test]
    fn needs_at_least_one_stop() {
        let g = RadialGradient {
            cx: 0.0,
            cy: 0.0,
            r0: 0.0,
            r1: 1.0,
            stops: vec![],
            w: 4,
            h: 4,
        };
        match radial_gradient_mask(&g) {
            Err(e) => assert_eq!(e.code, crate::errors::CODE_INVALID_STATE),
            Ok(_) => panic!("empty stops must be rejected"),
        }
    }

    #[test]
    fn non_monotonic_profile_is_allowed() {
        // transparent -> peak -> transparent, the shockwave ring shape. r0=3,
        // r1=5 puts the corners (distance 5.66 from the centre) outside.
        let g = RadialGradient {
            cx: 4.0,
            cy: 4.0,
            r0: 3.0,
            r1: 5.0,
            stops: vec![
                Stop::new(0.0, 0, 0, 0, 0),
                Stop::new(0.5, 255, 255, 255, 255),
                Stop::new(1.0, 0, 0, 0, 0),
            ],
            w: 9,
            h: 9,
        };
        let m = radial_gradient_mask(&g).expect("valid");
        assert!(m.rgba(0, 0).is_none(), "outside the ring must stay empty");
        assert!(m.rgba(4, 4).is_none(), "the hole at the centre stays empty");
        assert_eq!(m.rgba(4, 0), Some([255, 255, 255, 255]), "the peak ring");
    }
}
