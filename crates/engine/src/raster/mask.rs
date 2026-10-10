//! ellipse / rounded-rect / outline-ring masks, ported from
//! `src/core/raster/raster_masks.js`.
//!
//! These are the shapes `applyShape` composites through a [`PixelWriter`], and
//! they are graded against `raster_golden.json` groups `ellipse`/`rrect`/
//! `outline` — so the arithmetic below must stay in the same order, including
//! the empty-row/-column correction that keeps a degenerate ellipse connected.

use crate::errors::{DrawToolError, EngineResult};

/// Row-major 0/1 bitmap, `w * h` bytes.
///
/// `x`/`y` are the mask's origin in canvas space and default to 0; masks that
/// are a minimal bbox (wide lines) set them so callers can translate back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mask {
    pub x: i32,
    pub y: i32,
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

impl Mask {
    pub fn new(w: usize, h: usize) -> Self {
        Self::new_at(0, 0, w, h)
    }

    /// A mask at a canvas origin.
    pub fn new_at(x: i32, y: i32, w: usize, h: usize) -> Self {
        Self {
            x,
            y,
            w,
            h,
            data: vec![0u8; w * h],
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> u8 {
        self.data[y * self.w + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, v: u8) {
        self.data[y * self.w + x] = v;
    }

    /// `true` when the cell is painted.
    #[inline]
    pub fn is_set(&self, x: usize, y: usize) -> bool {
        self.get(x, y) == 1
    }

    /// The fixture's `#`/`.` text form, for diff output.
    pub fn rows(&self) -> Vec<String> {
        self.data
            .chunks(self.w)
            .map(|row| {
                row.iter()
                    .map(|&v| if v == 1 { '#' } else { '.' })
                    .collect()
            })
            .collect()
    }
}

/// Canonical mask-size guard: every entry point asserts this.
///
/// `DrawToolError(OUT_OF_RANGE)` rather than a panic, because the size can come
/// from user data (imported geometry) and the caller must be able to branch.
pub fn assert_mask_size(w: usize, h: usize) -> EngineResult<()> {
    if w < 1 || h < 1 {
        return Err(DrawToolError::out_of_range(format!(
            "mask size must be >= 1, got {w}x{h}"
        )));
    }
    Ok(())
}

/// `[x, y]` or `{x, y}` point in, `[x, y]` out.
#[inline]
pub fn to_xy(p: &[i32]) -> (i32, i32) {
    (p[0], p[1])
}

// <META - ROLE : ellipse mask with the empty row/col correction | L68-96>
/// Filled ellipse mask.
///
/// The two correction passes afterwards are load-bearing: a thin ellipse can
/// leave a row or column with no set cell, and the JS tree forced the middle
/// cell(s) in. `outlineRing` subtracts the inner ellipse, so skipping the
/// correction changes the ring, not just the fill.
pub fn ellipse_mask(w: usize, h: usize) -> EngineResult<Mask> {
    assert_mask_size(w, h)?;
    let mut m = Mask::new(w, h);
    let rhs = (w * w * h * h) as i64;
    for y in 0..h {
        for x in 0..w {
            // i64 throughout: `2x+1-w` is negative for the left half, which a
            // usize subtraction would panic on in debug builds.
            let dx = (2 * x + 1) as i64 - w as i64;
            let dy = (2 * y + 1) as i64 - h as i64;
            let lhs = dx * dx * (h * h) as i64 + dy * dy * (w * w) as i64;
            if lhs <= rhs {
                m.set(x, y, 1);
            }
        }
    }
    // Empty rows: paint the middle column(s).
    let xs = dedup_sorted([((w - 1) / 2) as i64, (w / 2) as i64]);
    let ys = dedup_sorted([((h - 1) / 2) as i64, (h / 2) as i64]);
    for y in 0..h {
        if !(0..w).any(|x| m.is_set(x, y)) {
            for &x in &xs {
                m.set(x as usize, y, 1);
            }
        }
    }
    // Empty columns: paint the middle row(s).
    for x in 0..w {
        if !(0..h).any(|y| m.is_set(x, y)) {
            for &y in &ys {
                m.set(x, y as usize, 1);
            }
        }
    }
    Ok(m)
}

/// `[a, b]` → sorted, de-duplicated, JS `[...new Set([...])]` semantics.
fn dedup_sorted(vals: [i64; 2]) -> Vec<i64> {
    let mut v = vals.to_vec();
    v.sort_unstable();
    v.dedup();
    v
}

// <META - ROLE : rounded-rect mask with the X=2x+1 corner rule | L98-118>
/// Filled rounded-rectangle mask; `r` is clamped into `[0, min(w,h)/2]`.
pub fn rrect_mask(w: usize, h: usize, r: i64) -> EngineResult<Mask> {
    assert_mask_size(w, h)?;
    let r = r.clamp(0, ((w / 2).min(h / 2)) as i64);
    let mut m = Mask::new(w, h);
    if r == 0 {
        m.data.fill(1);
        return Ok(m);
    }
    for y in 0..h {
        for x in 0..w {
            let big_x = (2 * x + 1) as i64;
            let big_y = (2 * y + 1) as i64;
            let cx = if big_x < 2 * r {
                Some(2 * r)
            } else if big_x > 2 * (w as i64 - r) {
                Some(2 * (w as i64 - r))
            } else {
                None
            };
            let cy = if big_y < 2 * r {
                Some(2 * r)
            } else if big_y > 2 * (h as i64 - r) {
                Some(2 * (h as i64 - r))
            } else {
                None
            };
            let inside = match (cx, cy) {
                (Some(cx), Some(cy)) => (big_x - cx).pow(2) + (big_y - cy).pow(2) <= 4 * r * r,
                _ => true,
            };
            m.set(x, y, inside as u8);
        }
    }
    Ok(m)
}

// <META - ROLE : inner-aligned outline ring; 2n >= min side means solid | L120-142>
/// Outline ring of thickness `n` around an ellipse or rounded rect.
///
/// When the inset would be empty (`iw < 1 || ih < 1`) the whole outer mask is
/// returned, which is how a 1px-tall ring ends up solid.
pub fn outline_ring(kind: &str, w: usize, h: usize, r: i64, n: usize) -> EngineResult<Mask> {
    assert_mask_size(w, h)?;
    let outer = if kind == "ellipse" {
        ellipse_mask(w, h)?
    } else {
        rrect_mask(w, h, r)?
    };
    let mut m = Mask::new(w, h);
    let iw = w as i64 - 2 * n as i64;
    let ih = h as i64 - 2 * n as i64;
    if iw < 1 || ih < 1 {
        m.data.copy_from_slice(&outer.data);
        return Ok(m);
    }
    let iw = iw as usize;
    let ih = ih as usize;
    let clamped_r = r.clamp(0, ((w / 2).min(h / 2)) as i64);
    let rr = (clamped_r - n as i64).max(0);
    let inner = if kind == "ellipse" {
        ellipse_mask(iw, ih)?
    } else {
        rrect_mask(iw, ih, rr)?
    };
    for y in 0..h {
        for x in 0..w {
            let inner_bit = if x >= n && x < n + iw && y >= n && y < n + ih {
                inner.get(x - n, y - n)
            } else {
                0
            };
            m.set(x, y, (outer.is_set(x, y) && inner_bit == 0) as u8);
        }
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    // <META - ROLE : golden ellipse group, 15 cases | L165-185>
    #[test]
    fn golden_ellipse_cases() {
        for case in crate::fixtures::group("ellipse") {
            let w = crate::fixtures::usize_field(case, "w");
            let h = crate::fixtures::usize_field(case, "h");
            let got = ellipse_mask(w, h).expect("valid size");
            let want = crate::fixtures::rows_to_mask(case);
            assert_eq!(
                got.rows(),
                crate::fixtures::mask_to_rows(&want),
                "ellipse {w}x{h}"
            );
        }
    }

    #[test]
    fn golden_rrect_cases() {
        for case in crate::fixtures::group("rrect") {
            let w = crate::fixtures::usize_field(case, "w");
            let h = crate::fixtures::usize_field(case, "h");
            let r = crate::fixtures::i64_field(case, "r");
            let got = rrect_mask(w, h, r).expect("valid size");
            let want = crate::fixtures::rows_to_mask(case);
            assert_eq!(
                got.rows(),
                crate::fixtures::mask_to_rows(&want),
                "rrect {w}x{h} r{r}"
            );
        }
    }

    #[test]
    fn golden_outline_cases() {
        for case in crate::fixtures::group("outline") {
            let kind = crate::fixtures::str_field(case, "kind");
            let w = crate::fixtures::usize_field(case, "w");
            let h = crate::fixtures::usize_field(case, "h");
            let r = case["r"].as_i64().unwrap_or(0);
            let n = crate::fixtures::usize_field(case, "n");
            let got = outline_ring(kind, w, h, r, n).expect("valid size");
            let want = crate::fixtures::rows_to_mask(case);
            assert_eq!(
                got.rows(),
                crate::fixtures::mask_to_rows(&want),
                "outline {kind} {w}x{h} n{n}"
            );
        }
    }

    #[test]
    fn rrect_radius_is_clamped() {
        // r=99 on an 8x8 clamps to 4, so the corners stay rounded — 52 of 64.
        let m = rrect_mask(8, 8, 99).expect("valid");
        assert_eq!(m.data.iter().filter(|&&v| v == 1).count(), 52);
        let m = rrect_mask(8, 8, 0).expect("valid");
        assert!(m.data.iter().all(|&v| v == 1));
    }

    #[test]
    fn thin_ring_is_solid() {
        // 2n >= h, so the inset disappears and the ring becomes the fill.
        let m = outline_ring("ellipse", 8, 2, 0, 1).expect("valid");
        let fill = ellipse_mask(8, 2).expect("valid");
        assert_eq!(m.data, fill.data);
    }

    #[test]
    fn size_guard_rejects_zero() {
        assert!(ellipse_mask(0, 5).is_err());
        assert!(rrect_mask(5, 0, 0).is_err());
    }
}
