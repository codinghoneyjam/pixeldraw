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
        mask,
    };
    guard.insert(n, fp.clone());
    Ok(fp)
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
pub trait PixelSink {
    fn set(&mut self, x: i32, y: i32, packed: u32);
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
}

/// Stamp one footprint at an integer pixel. The atomic "one click" operation.
pub fn stamp_brush<S: PixelSink>(sink: &mut S, x: i32, y: i32, n: usize, packed: u32) {
    let fp = match brush_footprint(n) {
        Ok(fp) => fp,
        Err(_) => return,
    };
    let x0 = x - fp.offset as i32;
    let y0 = y - fp.offset as i32;
    for by in 0..n {
        for bx in 0..n {
            if fp.is_set(bx, by) {
                sink.set(x0 + bx as i32, y0 + by as i32, packed);
            }
        }
    }
}

/// Fill the Bresenham path between two points with a round pen.
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
    for_each_bresenham(x0, y0, x1, y1, |x, y| {
        stamp_with(&fp, sink, x, y, packed);
    });
}

/// A single dot — the click path.
pub fn stroke_point<S: PixelSink>(sink: &mut S, x: i32, y: i32, n: usize, packed: u32) {
    stamp_brush(sink, x, y, n, packed);
}

/// Stamp with an already-resolved footprint, so a whole stroke reuses one cache hit.
fn stamp_with<S: PixelSink>(fp: &BrushFootprint, sink: &mut S, x: i32, y: i32, packed: u32) {
    let x0 = x - fp.offset as i32;
    let y0 = y - fp.offset as i32;
    for by in 0..fp.n {
        for bx in 0..fp.n {
            if fp.is_set(bx, by) {
                sink.set(x0 + bx as i32, y0 + by as i32, packed);
            }
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
}
