//! `lineMask` — a PIL-exact single-segment stroke.
//!
//! Ported from `src/core/raster/segment.js`. Two regimes, because PIL has two:
//!
//! * `width == 1` → the `line8` Bresenham-ish path, with the far endpoint forced
//!   back in so a single-segment stroke covers both ends.
//! * `width >= 2` → `ImagingDrawWideLine` widens the segment into a **rotated
//!   quad** and fills it with the polygon scanline engine, sharing
//!   [`polygon_generic`] with the closed-path outlines.
//!
//! The tempting alternative — stamp an n×n square along a Bresenham walk and
//! trim the caps by projection — is exact on **1** of 54 measured PIL cases.
//! The rotated quad is exact on 54/54 (see the 54-case `wide_line` group).

use crate::errors::EngineResult;
use crate::raster::Mask;
use crate::raster::polygon::{Edge, line8, polygon_generic, wide_line_quad_edges};

/// A point, either `[x, y]` or `{x, y}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pt {
    pub x: i32,
    pub y: i32,
}

impl Pt {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

impl From<[i32; 2]> for Pt {
    fn from([x, y]: [i32; 2]) -> Self {
        Self { x, y }
    }
}

impl From<(i32, i32)> for Pt {
    fn from((x, y): (i32, i32)) -> Self {
        Self { x, y }
    }
}

/// `assertBrushSize` — integer 1..=64, the pen range.
fn assert_brush_size(n: i32) -> EngineResult<()> {
    use crate::constants::{PEN_MAX, PEN_MIN};
    if !((PEN_MIN as i32)..=(PEN_MAX as i32)).contains(&n) {
        return Err(crate::errors::DrawToolError::out_of_range(format!(
            "brush size {n} out of range"
        )));
    }
    Ok(())
}

/// Stroke one segment at an integer width, matching PIL `d.line`.
///
/// The mask is a **minimal bbox** with `x`/`y` set, so callers add them back.
pub fn line_mask(p0: impl Into<Pt>, p1: impl Into<Pt>, n: i32) -> EngineResult<Mask> {
    assert_brush_size(n)?;
    let (x0, y0) = {
        let p = p0.into();
        (p.x, p.y)
    };
    let (x1, y1) = {
        let p = p1.into();
        (p.x, p.y)
    };

    // Pad by the stroke half-width so the quad's corners stay inside the buffer
    // and get the same clipping a full-canvas raster would apply.
    let pad = n;
    let bx = x0.min(x1) - pad;
    let by = y0.min(y1) - pad;
    let w = (x0.max(x1) + pad - bx + 1) as usize;
    let h = (y0.max(y1) + pad - by + 1) as usize;
    let mut m = Mask::new(w, h);
    m.x = bx;
    m.y = by;
    let lx0 = x0 - bx;
    let ly0 = y0 - by;
    let lx1 = x1 - bx;
    let ly1 = y1 - by;

    if n <= 1 {
        // line8 excludes the far endpoint of each segment; include it so a
        // single-segment stroke covers both endpoints the way PIL does.
        line8(&mut m.data, w, h, lx0, ly0, lx1, ly1, 1);
        m.set(lx1 as usize, ly1 as usize, 1);
    } else {
        let edges: Vec<Edge> = wide_line_quad_edges(lx0, ly0, lx1, ly1, n);
        polygon_generic(&mut m.data, w, h, &edges);
    }
    Ok(m)
}

/// The `wide_line` group's own parameter shape.
pub fn line_mask_from_fixture(p0: (i32, i32), p1: (i32, i32), n: i32) -> EngineResult<Mask> {
    line_mask(p0, p1, n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{group, i64_field, mask_to_rows, rows_to_mask, usize_field};

    // <META - ROLE : golden wide_line group, all 54 cases | L115-130>
    #[test]
    fn golden_wide_line_cases() {
        let mut checked = 0;
        for case in group("wide_line") {
            let p0 = case["p0"].as_array().expect("p0");
            let p1 = case["p1"].as_array().expect("p1");
            let width = i64_field(case, "width") as i32;
            // This group spells the canvas W/H in upper case.
            let w = usize_field(case, "W");
            let h = usize_field(case, "H");
            let m = line_mask(
                Pt::new(
                    p0[0].as_i64().unwrap() as i32,
                    p0[1].as_i64().unwrap() as i32,
                ),
                Pt::new(
                    p1[0].as_i64().unwrap() as i32,
                    p1[1].as_i64().unwrap() as i32,
                ),
                width,
            )
            .expect("valid size");

            // `lineMask` returns a MINIMAL bbox; the fixture is a full W×H canvas.
            // Blit the mask into one and compare rows.
            let mut canvas = Mask::new(w, h);
            for row in 0..m.h {
                let dy = m.y + row as i32;
                if !(0..h as i32).contains(&dy) {
                    continue;
                }
                for col in 0..m.w {
                    let dx = m.x + col as i32;
                    if !(0..w as i32).contains(&dx) {
                        continue;
                    }
                    canvas.set(dx as usize, dy as usize, m.data[row * m.w + col]);
                }
            }
            let want = rows_to_mask(case);
            let got_rows: Vec<String> = canvas
                .data
                .chunks(canvas.w)
                .map(|row| {
                    row.iter()
                        .map(|&v| if v == 1 { '#' } else { '.' })
                        .collect()
                })
                .collect();
            assert_eq!(got_rows, mask_to_rows(&want), "wide_line {}", case["name"]);
            checked += 1;
        }
        assert_eq!(checked, 54, "the wide_line group has 54 cases");
    }

    #[test]
    fn pixel_line_includes_both_endpoints() {
        let m = line_mask(Pt::new(0, 0), Pt::new(4, 4), 1).expect("valid");
        assert!(!m.data.is_empty());
        assert!(m.data.contains(&1));
        let set = |x: i32, y: i32| m.is_set((x - m.x) as usize, (y - m.y) as usize);
        assert!(set(0, 0), "start point covered");
        assert!(set(4, 4), "far endpoint forced back in");
    }

    #[test]
    fn width_guard_matches_the_pen_range() {
        assert!(line_mask(Pt::new(0, 0), Pt::new(5, 5), 0).is_err());
        assert!(line_mask(Pt::new(0, 0), Pt::new(5, 5), 65).is_err());
        assert!(line_mask(Pt::new(0, 0), Pt::new(5, 5), 64).is_ok());
    }

    #[test]
    fn bbox_origin_is_the_padded_minimum() {
        let m = line_mask(Pt::new(10, 20), Pt::new(30, 8), 4).expect("valid");
        assert_eq!(m.x, 10 - 4);
        assert_eq!(m.y, 8 - 4);
    }
}
