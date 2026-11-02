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
                let m = ellipse_mask(w.parse().unwrap(), h.parse().unwrap()).expect("valid size");
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
