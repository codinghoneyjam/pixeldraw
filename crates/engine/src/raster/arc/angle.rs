//! The arc family — sampled ellipse perimeter, stroke, angle clip.
//!
//! `ImageDraw.arc` strokes a *sampled* ellipse perimeter, so an arc is not an
//! analytic curve: it is a polyline of ~0.5° samples, each segment going
//! through the same `line8` / wide-quad path as everything else. That is why
//! this module is thin — [`arc_ellipse_points`] plus the primitives from
//! `super::polygon`.
//!
//! **Near-parity is deliberate.** The golden fixtures measure the JS tree's
//! current behaviour against PIL with tolerances, not exact equality:
//!
//! | case | rule |
//! |---|---|
//! | `arc` | drops **0** PIL pixels, may overshoot by **≤ 16** |
//! | `chord` fill | drops **0**, overshoots by **≤ 4** |
//! | `chord` outline | drops **≤ 16**, overshoots by **≤ 40** |
//!
//! A port that matched PIL *better* would fail the gate, so the bounds in
//! [`near_parity`] are the contract, not a concession.
//!
//! Angles follow PIL: 0° is 3 o'clock and they increase clockwise, which on a
//! y-down canvas means `sin` grows downward.

use crate::errors::EngineResult;
use crate::raster::mask::{Mask, assert_mask_size};
use crate::raster::polygon::{line8, polygon_generic, wide_line_quad_edges};

const DEG: f64 = std::f64::consts::PI / 180.0;

/// A `[cx, cy, rx, ry]` box, the way the fixtures spell one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Box2 {
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
}

impl Box2 {
    /// PIL's bounding-box convention: two corners → centre + radii.
    pub fn from_corners(a: [f64; 2], b: [f64; 2]) -> Self {
        Self {
            cx: (a[0] + b[0]) / 2.0,
            cy: (a[1] + b[1]) / 2.0,
            rx: (b[0] - a[0]) / 2.0,
            ry: (b[1] - a[1]) / 2.0,
        }
    }
}

/// Sample the ellipse perimeter between two PIL angles.
///
/// The end angle is normalised forward, so `start = 300, end = 60` walks the
/// long way round through 0°. `step_deg = 0.5` is what the fixtures used.
pub fn arc_ellipse_points(b: Box2, start_deg: f64, end_deg: f64, step_deg: f64) -> Vec<[f64; 2]> {
    let mut end = end_deg;
    while end < start_deg {
        end += 360.0;
    }
    let span = end - start_deg;
    let steps = (span / step_deg).ceil().max(1.0) as i64;
    let mut pts = Vec::with_capacity((steps + 1) as usize);
    for i in 0..=steps {
        let a = (start_deg + span * i as f64 / steps as f64) * DEG;
        pts.push([b.cx + b.rx * a.cos(), b.cy + b.ry * a.sin()]);
    }
    pts
}

/// Shared by `chord.rs`, which snaps its samples the same way.
/// JS `Math.round`: half **toward +∞** (`Math.round(-0.5) === -0`), which is
/// *not* `f64::round`'s half-away-from-zero. Only differs on exact .5
/// negatives, but the arc samples land on them often enough to matter.
#[inline]
pub(super) fn round_half_up(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}

/// Stroke an open point path segment by segment.
///
/// Each segment is rounded to integer grid coordinates the way PIL resolves
/// floats before plotting; shared endpoints round identically, so the chained
/// path stays continuous.
pub(super) fn stroke_open_path(data: &mut [u8], w: usize, h: usize, pts: &[[f64; 2]], width: i32) {
    for i in 0..pts.len().saturating_sub(1) {
        let x0 = round_half_up(pts[i][0]);
        let y0 = round_half_up(pts[i][1]);
        let x1 = round_half_up(pts[i + 1][0]);
        let y1 = round_half_up(pts[i + 1][1]);
        if x0 == x1 && y0 == y1 {
            // Degenerate after rounding (dense sampling on a flat region):
            // paint a width-sized dot, matching PIL's zero-length wide line.
            dot_rect(data, w, h, x0, y0, width);
            continue;
        }
        if width <= 1 {
            line8(data, w, h, x0, y0, x1, y1, 1);
        } else {
            polygon_generic(data, w, h, &wide_line_quad_edges(x0, y0, x1, y1, width));
        }
    }
}

/// A `width × width` dot centred on an integer grid point.
pub(super) fn dot_rect(data: &mut [u8], w: usize, h: usize, x: i32, y: i32, width: i32) {
    let o = width / 2;
    for dy in -o..width - o {
        for dx in -o..width - o {
            let px = x + dx;
            let py = y + dy;
            if px >= 0 && py >= 0 && px < w as i32 && py < h as i32 {
                data[py as usize * w + px as usize] = 1;
            }
        }
    }
}

/// Clear stroke pixels outside the PIL angle sector (butt-cap behaviour).
///
/// The chord closing line is *not* part of the arc sector, so it must be
/// stroked after this clip — see `chord::chord_outline_mask`.
pub(super) fn clip_angle_range(
    data: &mut [u8],
    w: usize,
    h: usize,
    b: Box2,
    start_deg: f64,
    end_deg: f64,
) {
    let mut end = end_deg;
    while end < start_deg {
        end += 360.0;
    }
    for y in 0..h {
        for x in 0..w {
            if data[y * w + x] != 1 {
                continue;
            }
            let mut a = ((y as f64 - b.cy).atan2(x as f64 - b.cx)) / DEG;
            if a < 0.0 {
                a += 360.0;
            }
            if a < start_deg {
                a += 360.0;
            }
            if a < start_deg || a > end {
                data[y * w + x] = 0;
            }
        }
    }
}

/// Arc stroke mask matching PIL `draw.arc`.
pub fn arc_mask(
    b: Box2,
    start_deg: f64,
    end_deg: f64,
    width: i32,
    w: usize,
    h: usize,
) -> EngineResult<Mask> {
    assert_mask_size(w, h)?;
    let mut m = Mask::new(w, h);
    let pts = arc_ellipse_points(b, start_deg, end_deg, 0.5);
    stroke_open_path(&mut m.data, w, h, &pts, width);
    clip_angle_range(&mut m.data, w, h, b, start_deg, end_deg);
    Ok(m)
}

/// The measured tolerance for each near-parity case, pinned from the fixture
/// comments in `tools/parity_check.mjs`.
pub mod near_parity {
    /// `arc`: must cover every PIL pixel, may add ≤ 16.
    pub const ARC_MAX_EXTRA: usize = 16;
    /// `chord` fill: must cover every PIL pixel, may add ≤ 4.
    pub const CHORD_FILL_MAX_EXTRA: usize = 4;
    /// `chord` outline: may drop ≤ 16 and add ≤ 40.
    pub const CHORD_OUTLINE_MAX_MISSING: usize = 16;
    pub const CHORD_OUTLINE_MAX_EXTRA: usize = 40;

    /// `(missing, extra)` counts between a golden mask and a produced one.
    pub fn diff(want: &[Vec<bool>], got: &[Vec<bool>]) -> (usize, usize) {
        let mut missing = 0;
        let mut extra = 0;
        for y in 0..want.len().max(got.len()) {
            let w = want.get(y);
            let g = got.get(y);
            for x in 0..w
                .map(|r| r.len())
                .unwrap_or(0)
                .max(g.map(|r| r.len()).unwrap_or(0))
            {
                let a = w.and_then(|r| r.get(x)).copied().unwrap_or(false);
                let b = g.and_then(|r| r.get(x)).copied().unwrap_or(false);
                if a && !b {
                    missing += 1;
                }
                if b && !a {
                    extra += 1;
                }
            }
        }
        (missing, extra)
    }
}
