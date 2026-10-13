//! The round pen: footprint, Bresenham enumeration and stamping.
//!
//! Ported from `src/features/pen/brush.js` (the JS tree's SSOT; the duplicate in
//! `core/raster/raster_brush.js` was removed upstream).
//!
//! The 1·2·3px footprints are the interesting part: the rule
//! `(2x+1-n)² + (2y+1-n)² <= n²` converges to a circle for large `n` and to a
//! sensible square-ish dot for `n <= 3`, which is what the `brush` golden group
//! pins.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::constants::{PEN_MAX, PEN_MIN};
use crate::errors::{DrawToolError, EngineResult};

/// A cached circular footprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrushFootprint {
    pub n: usize,
    /// `(n - 1) >> 1`, the offset from the stamp centre to the mask's origin.
    pub offset: usize,
    /// Row-major `n * n`, 1 = painted.
    pub mask: Vec<u8>,
    /// Per row, the horizontal runs of painted cells as `(start, len)`.
    ///
    /// The disc rule makes every row one contiguous run, so this is `n` runs for
    /// the whole stamp — a pen-64 footprint goes from 3,228 `set` calls to 64
    /// [`PixelSink::set_run`] calls, and the mask bytes are then never touched
    /// again after the first build. Derived from `mask`, never a second source
    /// of truth: `brush_footprint_runs_match_mask` pins them together.
    pub runs: Vec<Vec<(usize, usize)>>,
}

impl BrushFootprint {
    #[inline]
    pub fn is_set(&self, x: usize, y: usize) -> bool {
        self.mask[y * self.n + x] == 1
    }
}

fn cache() -> &'static std::sync::Mutex<HashMap<usize, BrushFootprint>> {
    static CACHE: OnceLock<std::sync::Mutex<HashMap<usize, BrushFootprint>>> = OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Size-guarded, cached footprint for pen diameter `n`.
pub fn brush_footprint(n: usize) -> EngineResult<BrushFootprint> {
    if !(PEN_MIN as usize..=PEN_MAX as usize).contains(&n) {
        return Err(DrawToolError::out_of_range(format!(
            "pen size {n} out of range"
        )));
    }
    let mut guard = cache().lock().expect("footprint cache poisoned");
    if let Some(fp) = guard.get(&n) {
        return Ok(fp.clone());
    }
    let mut mask = vec![0u8; n * n];
    let nn = (n * n) as i64;
    for y in 0..n as i64 {
        for x in 0..n as i64 {
            let lhs = (2 * x + 1 - n as i64).pow(2) + (2 * y + 1 - n as i64).pow(2);
            if lhs <= nn {
                mask[(y as usize) * n + x as usize] = 1;
            }
        }
    }
    let fp = BrushFootprint {
        n,
        offset: (n - 1) >> 1,
        runs: row_runs(&mask, n),
        mask,
    };
    guard.insert(n, fp.clone());
    Ok(fp)
}

/// The horizontal runs of painted cells, one `Vec` per row.
///
/// Scanning left to right and emitting a run at each transition keeps this in
/// step with `mask` by construction, whatever the footprint turns out to be —
/// an n=1 or n=2 pen has rows that are not contiguous in general.
fn row_runs(mask: &[u8], n: usize) -> Vec<Vec<(usize, usize)>> {
    let mut runs = Vec::with_capacity(n);
    for y in 0..n {
        let row = &mask[y * n..(y + 1) * n];
        let mut out = Vec::new();
        let mut x = 0;
        while x < n {
            if row[x] == 1 {
                let start = x;
                while x < n && row[x] == 1 {
                    x += 1;
                }
                out.push((start, x - start));
            } else {
                x += 1;
            }
        }
        runs.push(out);
    }
    runs
}

/// Integer-grid Bresenham walk, endpoint inclusive.
///
/// The closure receives every cell from `(x0,y0)` to `(x1,y1)`, in order.
pub fn for_each_bresenham(
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    mut cb: impl FnMut(i32, i32),
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        cb(x0, y0);
        if x0 == x1 && y0 == y1 {
            return;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// The `set` surface a stamp needs. [`crate::chunkstore::PixelWriter`] is the
/// production implementation; tests use a recorder.
///
/// **The run-shaped methods.** `set`/`get` are one pixel at a time and every
/// producer of pixels here actually produces *spans*: a round pen is one run per
/// row, a scanline fill is one run per seed, a polygon edge is one run per row.
/// Writing a span through `set` costs a ledger update and a map probe per pixel,
/// which is what made the pen-64 stroke take seconds. So the trait carries the
/// span shapes, and every one of them has a **default body that is the per-pixel
/// loop** — a recording sink that implements only `set`/`get` therefore still
/// sees exactly the cells it saw before, in the same order, and every existing
/// test and golden keeps its meaning. [`crate::chunkstore::PixelWriter`]
/// overrides them with chunk-resolved versions.
pub trait PixelSink {
    fn set(&mut self, x: i32, y: i32, packed: u32);
    fn get(&self, x: i32, y: i32) -> u32;

    /// `len` pixels of one colour starting at `(x, y)`.
    fn set_run(&mut self, x: i32, y: i32, len: usize, color: &[u8; 4]) {
        let packed = u32::from_le_bytes(*color);
        for i in 0..len as i32 {
            self.set(x + i, y, packed);
        }
    }

    /// Paint over the maximal run of `seed` on row `y` that contains `x`,
    /// and report it as `(left, right)` **inclusive**.
    ///
    /// `w`/`h` bound the walk exactly as `flood_fill_scanline` did. `None` means
    /// `(x, y)` is out of bounds or is not `seed`, i.e. there is no span here.
    fn fill_span(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        seed: u32,
        fill: u32,
    ) -> Option<(i32, i32)> {
        if y < 0 || y >= h || x < 0 || x >= w || self.get(x, y) != seed {
            return None;
        }
        let mut l = x;
        while l > 0 && self.get(l - 1, y) == seed {
            l -= 1;
        }
        let mut r = x;
        while r + 1 < w && self.get(r + 1, y) == seed {
            r += 1;
        }
        self.set_run(l, y, (r - l + 1) as usize, &fill.to_le_bytes());
        Some((l, r))
    }

    /// Append every run of `seed` inside `[lo, hi)` of row `y`, left to right.
    fn seed_runs(&self, y: i32, lo: i32, hi: i32, seed: u32, out: &mut Vec<(i32, i32)>) {
        let mut i = lo;
        while i < hi {
            if self.get(i, y) == seed {
                let s = i;
                while i < hi && self.get(i, y) == seed {
                    i += 1;
                }
                out.push((s, i));
            } else {
                i += 1;
            }
        }
    }
}

impl PixelSink for crate::chunkstore::PixelWriter<'_> {
    #[inline]
    fn set(&mut self, x: i32, y: i32, packed: u32) {
        // Out-of-canvas is dropped here rather than wrapping: the JS tree's
        // `stampBrush` relied on `writer.set` ignoring negatives, and a bare
        // `u32` cast would land the pixel at the far corner of the canvas.
        if x < 0 || y < 0 {
            return;
        }
        crate::chunkstore::PixelWriter::set(self, x as u32, y as u32, packed);
    }

    #[inline]
    fn get(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 {
            return 0;
        }
        crate::chunkstore::PixelWriter::get(self, x as u32, y as u32)
    }

    /// All clipping — including "trim a run that starts off-canvas" — lives in
    /// [`crate::chunkstore::PixelWriter::set_run`], so there is one rule, not two.
    #[inline]
    fn set_run(&mut self, x: i32, y: i32, len: usize, color: &[u8; 4]) {
        crate::chunkstore::PixelWriter::set_run(self, x, y, len, color);
    }

    /// Chunk-stepped: the span edges come from [`PixelWriter::row_seed_runs`],
    /// and the fill is one `set_run` instead of `r - l + 1` writes.
    fn fill_span(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        seed: u32,
        fill: u32,
    ) -> Option<(i32, i32)> {
        if y < 0 || y >= h || x < 0 || x >= w || self.pixel_at(x, y) != seed {
            return None;
        }
        let mut buf = Vec::new();
        // Left edge: the run in [0, x+1) that actually contains x.
        self.row_seed_runs(y, 0, x + 1, seed, &mut buf);
        let l = buf.last()?.0;
        // Right edge: the first run at or after x, extended to the row end.
        buf.clear();
        self.row_seed_runs(y, x, w, seed, &mut buf);
        let r = buf.first()?.1 - 1;
        crate::chunkstore::PixelWriter::set_run(
            self,
            l,
            y,
            (r - l + 1) as usize,
            &fill.to_le_bytes(),
        );
        Some((l, r))
    }

    fn seed_runs(&self, y: i32, lo: i32, hi: i32, seed: u32, out: &mut Vec<(i32, i32)>) {
        crate::chunkstore::PixelWriter::row_seed_runs(self, y, lo, hi, seed, out);
    }
}

/// Stamp one footprint at an integer pixel. The atomic "one click" operation.
pub fn stamp_brush<S: PixelSink>(sink: &mut S, x: i32, y: i32, n: usize, packed: u32) {
    let fp = match brush_footprint(n) {
        Ok(fp) => fp,
        Err(_) => return,
    };
    stamp_with(&fp, sink, x, y, packed);
}

/// Fill the Bresenham path between two points with a round pen.
///
/// **The stamps overlap, and the overlap is thrown away.** A pen-64 footprint is
/// 3,228 pixels wide but the walk advances it by *one* pixel per step, so
/// consecutive stamps repaint ~98% of what the previous one painted: a 2,028-px
/// segment writes 6.6M pixels to cover 131k of them.
///
/// So this call remembers which canvas columns of each row *it* has already
/// painted with `packed`, in a [`PaintedRun`] list, and writes only what is new.
/// The pixels skipped are skipped because they already hold the colour being
/// written — so the store bytes, the before-images in the ledger and the dirty
/// chunk set are all bit-identical to the naive walk. Only the number of writes
/// changes, which nothing observes: [`stroke_segment`] returns `()`.
///
/// The memo lives in this call and nowhere else, which is what makes it sound.
/// A sink reused across calls can be painted red, green, red again, and only a
/// per-call memo knows that everything in it is still the colour we are writing.
/// A disjoint run also resets its row's entry rather than merging, so the entry
/// is always one *contiguous* interval this call painted — the only shape in
/// which "skip what is inside it" is justified.
///
/// Diagonal and back-tracking paths lose some of the win (a row's interval is
/// replaced when the walk leaves it and comes back), and lose none of the
/// correctness.
pub fn stroke_segment<S: PixelSink>(
    sink: &mut S,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    n: usize,
    packed: u32,
) {
    let fp = match brush_footprint(n) {
        Ok(fp) => fp,
        Err(_) => return,
    };
    // A 1px pen has no overlap to skip: consecutive stamps land on new pixels
    // every time. Paying for the memo there measured as a ~2x regression, so the
    // degenerate case keeps the original one-cell-per-step walk.
    if fp.n == 1 {
        for_each_bresenham(x0, y0, x1, y1, |x, y| sink.set(x, y, packed));
        return;
    }
    let color = packed.to_le_bytes();
    // The cursor is carried across stamps rather than reset per stamp. A stroke
    // walks rows forwards, so restarting it at 0 made the first footprint row of
    // every step re-walk the whole memo from the top - ~23M wasted iterations for
    // one pen-64 bench item. A walk that goes *backwards* is still correct: the
    // cursor stops advancing, the row reads as "before the cursor", and that path
    // searches.
    let mut cursor = 0;
    let mut memo: Vec<PaintedRun> = Vec::new();
    for_each_bresenham(x0, y0, x1, y1, |x, y| {
        stamp_with_memo(&fp, &mut memo, &mut cursor, sink, x, y, &color);
    });
}

/// A canvas row, and the columns of it a single [`stroke_segment`] call has
/// already filled with its stroke colour.
#[derive(Clone, Copy)]
struct PaintedRun {
    row: i32,
    /// Inclusive-exclusive canvas column interval, known to hold the colour.
    lo: i32,
    hi: i32,
}

/// A single dot — the click path.
pub fn stroke_point<S: PixelSink>(sink: &mut S, x: i32, y: i32, n: usize, packed: u32) {
    stamp_brush(sink, x, y, n, packed);
}

/// Stamp with an already-resolved footprint, so a whole stroke reuses one cache hit.
///
/// Row by row, run by run. The cells written, their colours and their order are
/// the same as the cell-by-cell loop this replaced — a run is only ever a
/// maximal group of *consecutive* painted cells on one row, and rows are still
/// visited top to bottom — so the golden `stroke`/`brush` groups and the file
/// bytes are unchanged.
fn stamp_with<S: PixelSink>(fp: &BrushFootprint, sink: &mut S, x: i32, y: i32, packed: u32) {
    let color = packed.to_le_bytes();
    for (by, row) in fp.runs.iter().enumerate() {
        let y = y - fp.offset as i32 + by as i32;
        for &(start, len) in row {
            sink.set_run(x - fp.offset as i32 + start as i32, y, len, &color);
        }
    }
}

/// [`stamp_with`] plus the stroke-local overlap skip described on
/// [`stroke_segment`].
fn stamp_with_memo<S: PixelSink>(
    fp: &BrushFootprint,
    memo: &mut Vec<PaintedRun>,
    cursor: &mut usize,
    sink: &mut S,
    x: i32,
    y: i32,
    color: &[u8; 4],
) {
    let x0 = x - fp.offset as i32;
    let y0 = y - fp.offset as i32;
    // `memo` is sorted by row and a stamp visits rows in ascending order, so one
    // cursor walks both: forward it costs the number of rows the walk has left,
    // and only a walk that comes *back* to a row it has already passed pays for
    // a search. It is carried **across** stamps by the caller — see
    // `stroke_segment`.
    for (by, row) in fp.runs.iter().enumerate() {
        let cy = y0 + by as i32;
        while *cursor < memo.len() && memo[*cursor].row < cy {
            *cursor += 1;
        }
        for &(start, len) in row {
            let lo = x0 + start as i32;
            let hi = lo + len as i32;
            let idx = match memo.get(*cursor) {
                Some(e) if e.row == cy => *cursor,
                // The walk came back down to a row it has already passed.
                Some(e) if e.row > cy => memo[..*cursor]
                    .binary_search_by_key(&cy, |e| e.row)
                    .unwrap_or_else(|i| i),
                // `*cursor == memo.len()`: a new row at the end.
                _ => *cursor,
            };
            let known = memo.get(idx).copied().filter(|e| e.row == cy);
            match known {
                // Overlapping or touching: the union stays contiguous, so the
                // new memo interval is the hull and only the two overhangs need
                // writing.
                Some(e) if e.lo <= hi && lo <= e.hi => {
                    if lo < e.lo {
                        sink.set_run(lo, cy, (e.lo - lo) as usize, color);
                    }
                    if e.hi < hi {
                        sink.set_run(e.hi, cy, (hi - e.hi) as usize, color);
                    }
                    memo[idx] = PaintedRun {
                        row: cy,
                        lo: e.lo.min(lo),
                        hi: e.hi.max(hi),
                    };
                }
                // Disjoint: forget the old interval rather than pretend the two
                // are one, and write this run whole.
                Some(_) => {
                    sink.set_run(lo, cy, len, color);
                    memo[idx] = PaintedRun { row: cy, lo, hi };
                }
                None => {
                    sink.set_run(lo, cy, len, color);
                    memo.insert(idx, PaintedRun { row: cy, lo, hi });
                }
            }
            *cursor = idx;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunkstore::{ChunkStore, PixelWriter};
    use crate::pixel::pack_rgba;
    use crate::raster::mask::{ellipse_mask, rrect_mask};

    const RED: u32 = 0xff0000ff;

    // A recording sink, so the stroke tests can assert on the emitted cells
    // rather than on a store's side effects.
    #[derive(Default)]
    struct Recorder {
        cells: Vec<(i32, i32)>,
    }

    impl PixelSink for Recorder {
        fn set(&mut self, x: i32, y: i32, _: u32) {
            self.cells.push((x, y));
        }

        fn get(&self, _x: i32, _y: i32) -> u32 {
            0
        }
    }

    // <META - ROLE : golden brush group, 14 sizes | L150-170>
    #[test]
    fn golden_brush_footprints() {
        for case in crate::fixtures::group("brush") {
            let n = crate::fixtures::usize_field(case, "n");
            let fp = brush_footprint(n).expect("valid size");
            let want = crate::fixtures::rows_to_mask(case);
            let got: Vec<String> = fp
                .mask
                .chunks(n)
                .map(|row| {
                    row.iter()
                        .map(|&v| if v == 1 { '#' } else { '.' })
                        .collect()
                })
                .collect();
            assert_eq!(got, crate::fixtures::mask_to_rows(&want), "brush n={n}");
        }
    }

    #[test]
    fn golden_brush_hashes_pin_the_big_ones() {
        // `hashes` pins the masks too large to inline: sha256 of the "#"/"."
        // rows joined by "\n", the same rule tools/gen_fixtures.py uses.
        for case in crate::fixtures::group("hashes") {
            let what = crate::fixtures::str_field(case, "what");
            let want = crate::fixtures::str_field(case, "sha256");
            let got = match what.split_once(' ') {
                Some(("brush", n)) => {
                    let n: usize = n.parse().expect("brush size");
                    let fp = brush_footprint(n).expect("valid size");
                    fp.mask
                        .chunks(n)
                        .map(|row| {
                            row.iter()
                                .map(|&v| if v == 1 { '#' } else { '.' })
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>()
                }
                Some(("ellipse", dims)) => {
                    let (w, h) = dims.split_once('x').expect("WxH");
                    let m =
                        ellipse_mask(w.parse().unwrap(), h.parse().unwrap()).expect("valid size");
                    m.rows()
                }
                Some(("rrect", dims)) => {
                    let (w, rest) = dims.split_once('x').expect("WxH");
                    let (h, r) = rest.split_once(' ').expect("H rR");
                    let r: usize = r.trim_start_matches('r').parse().expect("radius");
                    let m = rrect_mask(w.parse().unwrap(), h.parse().unwrap(), r as i64)
                        .expect("valid size");
                    m.rows()
                }
                Some(("outline", _)) => continue, // covered by the outline group's rows
                _ => continue,
            };
            use sha2::{Digest, Sha256};
            let digest = format!("{:x}", Sha256::digest(got.join("\n").as_bytes()));
            assert_eq!(digest, want, "{what}");
        }
    }

    // <META - ROLE : golden bresenham group, 7 cases | L172-200>
    #[test]
    fn golden_bresenham_cases() {
        for case in crate::fixtures::group("bresenham") {
            let p0 = case["p0"].as_array().unwrap();
            let p1 = case["p1"].as_array().unwrap();
            let want: Vec<(i32, i32)> = case["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| (p[0].as_i64().unwrap() as i32, p[1].as_i64().unwrap() as i32))
                .collect();
            let mut got = Vec::new();
            for_each_bresenham(
                p0[0].as_i64().unwrap() as i32,
                p0[1].as_i64().unwrap() as i32,
                p1[0].as_i64().unwrap() as i32,
                p1[1].as_i64().unwrap() as i32,
                |x, y| got.push((x, y)),
            );
            assert_eq!(got, want, "bresenham {p0:?} -> {p1:?}");
        }
    }

    #[test]
    fn pen_size_guard() {
        assert!(brush_footprint(0).is_err());
        assert!(brush_footprint(65).is_err());
        assert!(brush_footprint(1).is_ok());
        assert!(brush_footprint(64).is_ok());
    }

    #[test]
    fn offsets_follow_the_js_rule() {
        assert_eq!(brush_footprint(1).unwrap().offset, 0);
        assert_eq!(brush_footprint(2).unwrap().offset, 0);
        assert_eq!(brush_footprint(3).unwrap().offset, 1);
        assert_eq!(brush_footprint(64).unwrap().offset, 31);
    }

    #[test]
    fn stroke_segment_fills_the_line() {
        let mut rec = Recorder::default();
        stroke_segment(&mut rec, 0, 0, 4, 0, 1, RED);
        assert_eq!(rec.cells, vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]);
    }

    #[test]
    fn stamping_through_a_pixel_writer_matches_the_mask() {
        let mut store = ChunkStore::new(64, 64).unwrap();
        {
            let mut w = PixelWriter::new(&mut store, "layer-0001");
            stroke_point(&mut w, 32, 32, 3, RED);
            let fp = brush_footprint(3).unwrap();
            for by in 0..3 {
                for bx in 0..3 {
                    if fp.is_set(bx, by) {
                        let x = 32 - fp.offset as i32 + bx as i32;
                        let y = 32 - fp.offset as i32 + by as i32;
                        assert_eq!(w.get(x as u32, y as u32), RED, "missing pixel at ({x},{y})");
                    }
                }
            }
        }
        assert_eq!(store.get_pixel(32, 32), RED);
    }

    #[test]
    fn negative_coordinates_are_dropped_not_wrapped() {
        let mut store = ChunkStore::new(64, 64).unwrap();
        {
            let mut w = PixelWriter::new(&mut store, "layer-0001");
            stroke_point(&mut w, -5, -5, 8, pack_rgba(0.0, 255.0, 0.0, 255.0));
        }
        // Nothing landed, and nothing wrapped into the far corners.
        assert_eq!(store.chunk_count(), 0);
    }

    /// The overlap skip in `stroke_segment` must not change a single pixel.
    ///
    /// It is checked against `stroke_mask`, which is the *naive* walk: a
    /// `MaskSink` takes the default per-pixel `set_run`, so the two paths share
    /// no code past `BrushFootprint`. Several geometries, because the memo
    /// behaves differently on a monotone line, a steep diagonal and a path that
    /// doubles back over itself — only the last exercises the "disjoint run
    /// resets the row" branch.
    #[test]
    fn the_overlap_skip_paints_the_same_pixels_as_the_naive_walk() {
        let cases: [(&[(i32, i32)], usize); 6] = [
            (&[(0, 0), (47, 3)], 1),           // a 1px pen has no overlap to skip
            (&[(0, 0), (47, 0)], 8),           // perfectly monotone
            (&[(0, 0), (60, 40)], 16),         // 3:2 diagonal
            (&[(60, 0), (2, 34)], 32),         // steep, walks left
            (&[(4, 4), (60, 4), (6, 30)], 12), // doubles back over itself
            (&[(0, 0), (0, 0)], 64),           // one step at the largest pen
        ];
        let (w, h) = (128u32, 64u32);
        for (points, n) in cases {
            let mut store = ChunkStore::new(w, h).expect("valid size");
            {
                let mut writer = PixelWriter::new(&mut store, "layer-0001");
                if points.len() == 1 {
                    stroke_point(&mut writer, points[0].0, points[0].1, n, RED);
                } else {
                    for pair in points.windows(2) {
                        stroke_segment(
                            &mut writer,
                            pair[0].0,
                            pair[0].1,
                            pair[1].0,
                            pair[1].1,
                            n,
                            RED,
                        );
                    }
                }
                writer.finish();
            }
            let want = crate::kernels::stroke_mask(points, n, w as usize, h as usize);
            let mut bad = 0;
            for y in 0..h {
                for x in 0..w {
                    let painted = store.get_pixel(x, y) == RED;
                    if painted != want[y as usize * w as usize + x as usize] {
                        bad += 1;
                    }
                }
            }
            assert_eq!(bad, 0, "points {points:?} pen {n}");
        }
    }

    /// A run that starts off the left edge keeps the part that lands inside —
    /// `set` clips per pixel, so the run form has to clip per pixel too.
    #[test]
    fn a_run_starting_off_canvas_is_trimmed_not_dropped() {
        let mut store = ChunkStore::new(64, 64).unwrap();
        {
            let mut w = PixelWriter::new(&mut store, "layer-0001");
            w.set_run(-6, 10, 12, &RED.to_le_bytes());
        }
        let row: Vec<u32> = (0..12).map(|x| store.get_pixel(x, 10)).collect();
        assert_eq!(
            row,
            vec![RED, RED, RED, RED, RED, RED, 0, 0, 0, 0, 0, 0],
            "a run starting at x=-6 paints canvas x 0..=5 and nothing else"
        );
    }
}
