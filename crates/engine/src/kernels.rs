//! Drawing kernels — the tool-level operations that write many pixels.
//!
//! Ported from `src/features/fill/fill.js` and `src/features/pen/brush.js`.
//! The JS tree scattered the family across three files; here each kernel is a
//! function that takes a [`PixelSink`] and nothing else, which is exactly the
//! testability property `tools_basic.test.mjs` exploited with its fake writer.
//!
//! The other two kernels the plan names live where their masks already are:
//!
//! * `strokeSegment` / `strokePoint` → [`crate::raster::brush`].
//! * `applyShape` → [`crate::raster::shape_raster`].
//!
//! Both are verified by the golden `stroke` group and the `shape_*` suites.

use crate::raster::PixelSink;
use crate::raster::brush::{stroke_point, stroke_segment};

/// A `w × h` grid that stamps receive from a stroke.
///
/// The same shape `tests/core_basic.test.mjs` used for the `stroke` golden: a
/// flat array with the out-of-range clip the fake writer did.
struct MaskSink<'a> {
    w: usize,
    h: usize,
    cells: &'a mut [bool],
}

impl PixelSink for MaskSink<'_> {
    fn set(&mut self, x: i32, y: i32, _packed: u32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        self.cells[y as usize * self.w + x as usize] = true;
    }

    fn get(&self, _x: i32, _y: i32) -> u32 {
        0
    }
}

/// Rasterize a pen stroke into a `w × h` row-major mask.
///
/// A single sample is a dot; several samples are joined by Bresenham segments
/// and stamped with the round footprint at every step. The out-of-canvas clip
/// lives in the [`MaskSink`], so a stroke that starts off-canvas paints exactly
/// the part that lands inside — which is what the golden `stroke` case with
/// `points: [[-3,2],[6,2]]` pins.
///
/// An out-of-range pen size yields an empty mask rather than an error: the same
/// "silently nothing" the JS `stampBrush` guard produced via `brushFootprint`.
pub fn stroke_mask(points: &[(i32, i32)], n: usize, w: usize, h: usize) -> Vec<bool> {
    let mut cells = vec![false; w * h];
    {
        let mut sink = MaskSink {
            w,
            h,
            cells: &mut cells,
        };
        if points.len() == 1 {
            stroke_point(&mut sink, points[0].0, points[0].1, n, 0);
        } else {
            for pair in points.windows(2) {
                stroke_segment(&mut sink, pair[0].0, pair[0].1, pair[1].0, pair[1].1, n, 0);
            }
        }
    }
    cells
}

/// Scanline flood fill, contiguous region of `seed` → `fill`.
///
/// The algorithm is the classic span-based one: pop a seed, walk left and right
/// to the span edges, fill it, then push the still-`seed` runs on the two
/// neighbouring rows. That is why the golden `fill` group's expect grids are
/// symmetric — a 4-neighbour DFS would visit the same cells in a different
/// order but produce identical output here.
///
/// `seed == fill` short-circuits before anything is touched, so "already this
/// colour" is a no-op rather than an infinite loop.
///
/// **What changed and why it is still the same walk.** The body used to spell
/// both halves out as `sink.get`/`sink.set` loops. It now asks the sink for the
/// two span operations instead — [`PixelSink::fill_span`] and
/// [`PixelSink::seed_runs`] — whose *default* bodies are exactly those loops. A
/// grid sink therefore still gets a pixel-at-a-time walk, while a
/// [`crate::chunkstore::PixelWriter`] answers both from 32 pixels at a time and
/// writes the span with one `set_run`. Same seeds popped, same order, same
/// pixels: the traversal, not the arithmetic, is what this kernel owns.
///
/// # Panics
///
/// Neither. Out-of-range coordinates are skipped, as in the JS original.
pub fn flood_fill_scanline<S: PixelSink>(
    sink: &mut S,
    w: usize,
    h: usize,
    sx: usize,
    sy: usize,
    seed: u32,
    fill: u32,
) {
    if seed == fill {
        return;
    }
    let (wi, hi) = (w as i32, h as i32);
    let mut stack: Vec<(isize, isize)> = vec![(sx as isize, sy as isize)];
    // Reused across the whole walk so a fill does not allocate per span.
    let mut runs: Vec<(i32, i32)> = Vec::new();
    while let Some((cx, cy)) = stack.pop() {
        let (cx, cy) = (cx as i32, cy as i32);
        if cx < 0 || cy < 0 || cx >= wi || cy >= hi {
            continue;
        }
        let Some((l, r)) = sink.fill_span(cx, cy, wi, hi, seed, fill) else {
            continue;
        };
        for ny in [cy - 1, cy + 1] {
            if ny < 0 || ny >= hi {
                continue;
            }
            runs.clear();
            sink.seed_runs(ny, l, r + 1, seed, &mut runs);
            stack.extend(runs.iter().map(|&(s, _)| (s as isize, ny as isize)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{group, usize_field};

    /// A grid-backed sink, the same shape the JS test's fake writer used.
    #[derive(Default)]
    struct Grid {
        cells: Vec<u32>,
        w: usize,
        h: usize,
    }

    impl Grid {
        fn new(w: usize, h: usize) -> Self {
            Self {
                cells: vec![0; w * h],
                w,
                h,
            }
        }

        /// The fixture's nested arrays → a flat store.
        fn from_json(grid: &serde_json::Value) -> Self {
            let rows = grid.as_array().expect("grid");
            let h = rows.len();
            let w = rows[0].as_array().expect("row").len();
            let mut g = Self::new(w, h);
            for (y, row) in rows.iter().enumerate() {
                for (x, v) in row.as_array().unwrap().iter().enumerate() {
                    g.cells[y * w + x] = v.as_u64().expect("cell") as u32;
                }
            }
            g
        }

        fn to_json(&self) -> Vec<Vec<u32>> {
            self.cells.chunks(self.w).map(|row| row.to_vec()).collect()
        }
    }

    impl PixelSink for Grid {
        fn set(&mut self, x: i32, y: i32, v: u32) {
            let (x, y) = (x as usize, y as usize);
            self.cells[y * self.w + x] = v;
        }

        fn get(&self, x: i32, y: i32) -> u32 {
            self.cells[y as usize * self.w + x as usize]
        }
    }

    // <META - ROLE : golden fill group, all 3 cases, same as tools_basic.test.mjs L196-204>
    #[test]
    fn golden_fill_cases() {
        for case in group("fill") {
            let mut grid = Grid::from_json(&case["grid"]);
            let w = grid.w;
            let h = grid.h;
            let x = usize_field(case, "x");
            let y = usize_field(case, "y");
            let new = crate::fixtures::u32_field(case, "new");
            let seed = grid.cells[y * w + x];
            flood_fill_scanline(&mut grid, w, h, x, y, seed, new);
            let want: Vec<Vec<u32>> = case["expect"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    r.as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_u64().unwrap() as u32)
                        .collect()
                })
                .collect();
            assert_eq!(grid.to_json(), want, "fill {case}");
        }
    }

    // <META - ROLE : golden stroke group, all 7 cases, same as core_basic.test.mjs L217-234>
    #[test]
    fn golden_stroke_cases() {
        for case in group("stroke") {
            let w = usize_field(case, "W");
            let h = usize_field(case, "H");
            let n = usize_field(case, "n");
            let points: Vec<(i32, i32)> = case["points"]
                .as_array()
                .expect("points")
                .iter()
                .map(|p| (p[0].as_i64().unwrap() as i32, p[1].as_i64().unwrap() as i32))
                .collect();
            let got = stroke_mask(&points, n, w, h);
            let got_rows: Vec<String> = got
                .chunks(w)
                .map(|row| row.iter().map(|&v| if v { '#' } else { '.' }).collect())
                .collect();
            assert_eq!(
                got_rows,
                crate::fixtures::mask_to_rows(&crate::fixtures::rows_to_mask(case)),
                "stroke n={n}"
            );
        }
    }

    #[test]
    fn a_stroke_starting_off_canvas_paints_only_what_lands_inside() {
        // The negative-origin golden case: a 3px pen from (-3,2) to (6,2) over a
        // 10x6 canvas covers x 0..6 on rows 2..4 — three rows, because a 3px
        // footprint is 3 rows tall, and nothing wraps to the far right edge.
        let got = stroke_mask(&[(-3, 2), (6, 2)], 3, 10, 6);
        let rows: Vec<String> = got
            .chunks(10)
            .map(|r| r.iter().map(|&v| if v { '#' } else { '.' }).collect())
            .collect();
        assert_eq!(
            rows,
            vec![
                "..........",
                "########..",
                "########..",
                "########..",
                "..........",
                "..........",
            ]
        );
    }

    #[test]
    fn a_single_sample_is_a_dot_not_a_line() {
        let got = stroke_mask(&[(4, 4)], 3, 10, 10);
        assert_eq!(got.iter().filter(|&&v| v).count(), 9);
    }

    #[test]
    fn filling_a_region_with_its_own_colour_is_a_no_op() {
        let mut g = Grid::new(4, 4);
        for y in 0..4 {
            for x in 0..4 {
                g.cells[y * 4 + x] = 5;
            }
        }
        flood_fill_scanline(&mut g, 4, 4, 1, 1, 5, 5);
        assert!(g.cells.iter().all(|&v| v == 5));
    }

    #[test]
    fn an_enclosed_hole_is_filled_separately() {
        // The JS golden's ring grid: the outer zero region and the inner hole
        // are two disconnected seeds of the same value, so filling the outer
        // ring must leave the hole alone.
        let mut g = Grid::new(5, 5);
        let ring = [
            [0, 0, 0, 0, 0],
            [0, 1, 1, 1, 0],
            [0, 1, 0, 1, 0],
            [0, 1, 1, 1, 0],
            [0, 0, 0, 0, 0],
        ];
        for (y, row) in ring.iter().enumerate() {
            for (x, v) in row.iter().enumerate() {
                g.cells[y * 5 + x] = *v;
            }
        }
        flood_fill_scanline(&mut g, 5, 5, 0, 0, 0, 7);
        assert_eq!(
            g.cells,
            [
                7, 7, 7, 7, 7, //
                7, 1, 1, 1, 7, //
                7, 1, 0, 1, 7, //
                7, 1, 1, 1, 7, //
                7, 7, 7, 7, 7,
            ]
        );
    }

    #[test]
    fn a_seed_outside_the_region_touches_nothing() {
        let mut g = Grid::new(4, 4);
        flood_fill_scanline(&mut g, 4, 4, 0, 0, 3, 9);
        assert!(g.cells.iter().all(|&v| v == 0));
    }
}
