//! Bresenham enumeration, stamping, and the overlap-skipping stroke memo.

use super::footprint::{BrushFootprint, brush_footprint};
use super::sink::PixelSink;

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
    if fp.n == 1 {
        for_each_bresenham(x0, y0, x1, y1, |x, y| sink.set(x, y, packed));
        return;
    }
    let color = packed.to_le_bytes();
    // The cursor is carried across stamps rather than reset per stamp. A stroke
    // walks rows forwards, so restarting it at 0 made the first footprint row of
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
