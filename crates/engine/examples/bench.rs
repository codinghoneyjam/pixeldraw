//! Phase 0.1 mirror — the Rust side of `tools/bench_baseline.mjs`.
//!
//! Run: `cargo run -p pixel-engine --release --example bench`
//!
//! The workload, the labels and the median-of-N rule are deliberately the same
//! ones the JS benchmark uses, so `docs/perf_js_vs_rust.md` can put the two
//! numbers in one table. Where the JS item has no Rust counterpart yet
//! (`documentToJson` — Phase 1.13 owns `io/serialize`) it is **omitted**, not
//! stubbed: a missing number is not a passing number.
//!
//! ```text
//! cargo run -p pixel-engine --release --example bench
//! ```
//!
//! Two items are timed with a *fresh* store per sample, because the flood fill
//! mutates its own input and the JS file explains why: re-timing an
//! already-filled canvas would measure the early-out instead of the algorithm.

use pixel_engine::chunkstore::{ChunkStore, PixelWriter};
use pixel_engine::composite::{self, LayerSource, composite_canvas, composite_canvas_serial};
use pixel_engine::constants::{CHUNK_LEN, CHUNK_PX};
use pixel_engine::io::export::{FlattenOptions, flatten_to_rgba, flatten_to_rgba_serial};
use pixel_engine::io::png::encode_png;
use pixel_engine::kernels::flood_fill_scanline;
use pixel_engine::raster::brush::{stroke_point, stroke_segment};
use pixel_engine::raster::shape_raster::{FillMode, ShapeKind, ShapeSpec, apply_shape};
use rayon::prelude::*;
use std::time::Instant;

// ---------------------------------------------------------------------------
// CONFIG — identical to the JS `CONFIG` block.
// ---------------------------------------------------------------------------

const WIDTH: u32 = 2048;
const HEIGHT: u32 = 1088;
const LAYERS: usize = 64;
const PEN_SIZES: [usize; 3] = [1, 8, 64];
const FILL_RUNS: usize = 3;
const RUNS: usize = 5;
const PNG_RUNS: usize = 3;

/// JS `RED = packRGBA(255, 0, 0, 255)`.
const RED: u32 = 0xff00_00ff;
/// JS `BLUE = packRGBA(0, 0, 255, 128)`.
const BLUE: u32 = 0x80ff_0000;
/// JS `packRGBA(0, 255, 0, 255)`, the flood-fill colour.
const GREEN: u32 = 0xff00_ff00;

/// The six layer blend modes, in the order the JS busy document cycles them.
const MODES: [&str; 6] = [
    "normal", "multiply", "screen", "overlay", "darken", "lighten",
];

/// What the doc can state about the build these numbers came from. These are
/// the workspace `[profile.release]` values in the root `Cargo.toml`; cargo
/// exposes no runtime API for reading the active profile back.
const PROFILE: &str =
    "opt-level=3, lto=thin, codegen-units=1, panic=abort (workspace [profile.release])";

// ---------------------------------------------------------------------------
// Timers — the same three shapes the JS file uses.
// ---------------------------------------------------------------------------

/// `timeIt` — median of N runs of the same closure.
fn time_it(name: &str, runs: usize, mut f: impl FnMut()) {
    let mut samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t0 = Instant::now();
        f();
        samples.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    report(name, samples);
}

/// `timeFresh` — one fresh context per sample, for ops that mutate their input.
fn time_fresh<T>(
    name: &str,
    runs: usize,
    mut make_ctx: impl FnMut() -> T,
    mut op: impl FnMut(&mut T),
) {
    let mut samples = Vec::with_capacity(runs);
    for _ in 0..runs {
        let mut ctx = make_ctx();
        let t0 = Instant::now();
        op(&mut ctx);
        samples.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    report(name, samples);
}

/// `{name} | median ms | min | max`.
fn report(name: &str, mut samples: Vec<f64>) {
    samples.sort_by(f64::total_cmp);
    let median = samples[samples.len() / 2];
    let min = samples[0];
    let max = samples[samples.len() - 1];
    println!("{name:<38} | {median:>10.3} ms | {min:>9.3} | {max:>9.3}");
}

// ---------------------------------------------------------------------------
// The busy document — every layer carries content, the worst case for compositing.
// ---------------------------------------------------------------------------

struct LayerMeta {
    opacity: f64,
    mode: &'static str,
}

/// A stand-in for `doc.layers`: the stores plus the two fields the compositor
/// and the flattener read. There is no `Session`/`Document` in the engine yet
/// (Phase 1.12), so the bench builds this directly through `PixelWriter`.
struct BusyDoc {
    stores: Vec<ChunkStore>,
    meta: Vec<LayerMeta>,
}

/// `makeBusyDoc` — diagonal strokes give every layer a spread of chunks plus
/// alpha variety.
fn make_busy_doc() -> BusyDoc {
    let mut stores = Vec::with_capacity(LAYERS);
    let mut meta = Vec::with_capacity(LAYERS);
    for li in 0..LAYERS {
        let mut store = ChunkStore::new(WIDTH, HEIGHT).expect("valid canvas size");
        {
            let mut w = PixelWriter::new(&mut store, format!("layer-{li:04}"));
            for k in 0..24i32 {
                stroke_segment(
                    &mut w,
                    (k * 97) % WIDTH as i32,
                    (k * 61) % HEIGHT as i32,
                    ((k * 97) + 400) % WIDTH as i32,
                    ((k * 61) + 300) % HEIGHT as i32,
                    5,
                    if k % 3 == 0 { BLUE } else { RED },
                );
            }
            let _ = w.finish();
        }
        stores.push(store);
        meta.push(LayerMeta {
            opacity: if li % 4 == 0 { 0.5 } else { 1.0 },
            mode: MODES[li % 6],
        });
    }
    BusyDoc { stores, meta }
}

impl BusyDoc {
    /// `Vec<LayerSource>`, bottom-to-top, first element = bottom.
    fn sources(&self) -> Vec<LayerSource<'_>> {
        self.stores
            .iter()
            .zip(&self.meta)
            .map(|(store, m)| LayerSource {
                store: Some(store),
                opacity: m.opacity,
                mode: m.mode,
                visible: true,
            })
            .collect()
    }
}

/// `Math.round(v)` = `floor(v + 0.5)`, the repo's one rounding rule.
fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

// ---------------------------------------------------------------------------
// Items
// ---------------------------------------------------------------------------

fn bench_strokes() {
    for n in PEN_SIZES {
        let mut store = ChunkStore::new(WIDTH, HEIGHT).expect("valid canvas size");
        let mut w = PixelWriter::new(&mut store, "layer-0001");
        // Same writer for every sample, exactly like the JS closure: the canvas
        // accumulates, which is what a real drag looks like.
        time_it(&format!("stroke pen={n} (20 segs)"), RUNS, || {
            for k in 0..20i32 {
                stroke_segment(
                    &mut w,
                    10,
                    (k * 13) % HEIGHT as i32,
                    WIDTH as i32 - 10,
                    (k * 29) % HEIGHT as i32,
                    n,
                    RED,
                );
            }
        });
        assert!(
            !w.take_dirty().is_empty(),
            "stroke produced no dirty chunks"
        );
        let _ = w.finish();
    }
}

fn bench_stroke_point() {
    let mut store = ChunkStore::new(WIDTH, HEIGHT).expect("valid canvas size");
    let mut w = PixelWriter::new(&mut store, "layer-0001");
    time_it("strokePoint pen=64 (1 dot)", RUNS, || {
        stroke_point(&mut w, 512, 512, 64, RED);
    });
    let _ = w.finish();
}

fn bench_flood_fill() {
    for label in ["empty canvas", "canvas already RED"] {
        let seed = if label == "empty canvas" { 0 } else { RED };
        // The walk mutates its input, so each sample gets a FRESH store:
        // re-timing an already-filled canvas would measure the early-out
        // instead of the algorithm.
        time_fresh(
            &format!("floodFill {label}"),
            FILL_RUNS,
            || {
                let mut store = ChunkStore::new(WIDTH, HEIGHT).expect("valid canvas size");
                if label != "empty canvas" {
                    let mut w = PixelWriter::new(&mut store, "layer-0001");
                    for y in 0..HEIGHT {
                        for x in 0..WIDTH {
                            w.set(x, y, RED);
                        }
                    }
                    w.take_dirty();
                }
                store
            },
            |store| {
                let mut w = PixelWriter::new(store, "layer-0001");
                flood_fill_scanline(&mut w, WIDTH as usize, HEIGHT as usize, 0, 0, seed, GREEN);
                w.take_dirty();
            },
        );
    }
}

fn bench_shapes() {
    let mut store = ChunkStore::new(WIDTH, HEIGHT).expect("valid canvas size");
    let mut w = PixelWriter::new(&mut store, "layer-0001");

    // The 32-gon, the same construction the JS file inlines.
    let polygon: Vec<[i32; 2]> = (0..32)
        .map(|i| {
            let a = (f64::from(i) / 32.0) * std::f64::consts::PI * 2.0;
            [
                js_round(512.0 + a.cos() * 500.0) as i32,
                js_round(512.0 + a.sin() * 500.0) as i32,
            ]
        })
        .collect();

    let shapes: [(&str, ShapeSpec); 4] = [
        (
            "ellipse 1024x1024 fill",
            ShapeSpec {
                kind: ShapeKind::Ellipse,
                stroke_width: 1,
                fill_mode: FillMode::Fill,
                bbox: Some([0, 0, 1024, 1024]),
                p0: None,
                p1: None,
                radius: 0,
                points: vec![],
            },
        ),
        (
            "rrect 1024x1024 r=64 outline sw=4",
            ShapeSpec {
                kind: ShapeKind::Rrect,
                stroke_width: 4,
                fill_mode: FillMode::Outline,
                bbox: Some([0, 0, 1024, 1024]),
                p0: None,
                p1: None,
                radius: 64,
                points: vec![],
            },
        ),
        (
            "line 2048x1088 sw=8",
            ShapeSpec {
                kind: ShapeKind::Line,
                stroke_width: 8,
                fill_mode: FillMode::Outline,
                bbox: None,
                p0: Some((0, 0)),
                p1: Some((2047, 1079)),
                radius: 0,
                points: vec![],
            },
        ),
        (
            "polygon 32-gon 1000px fill",
            ShapeSpec {
                kind: ShapeKind::Polygon,
                stroke_width: 1,
                fill_mode: FillMode::Fill,
                bbox: None,
                p0: None,
                p1: None,
                radius: 0,
                points: polygon,
            },
        ),
    ];

    for (label, spec) in &shapes {
        time_it(&format!("applyShape {label}"), RUNS, || {
            apply_shape(&mut w, spec, RED, BLUE).expect("valid shape spec");
        });
    }
    let _ = w.finish();
}

fn bench_composite() {
    let doc = make_busy_doc();
    let layers = doc.sources();
    let cx_total = (WIDTH / CHUNK_PX) as usize;
    let cy_total = (HEIGHT / CHUNK_PX) as usize;
    let total = cx_total * cy_total;
    let mut out = vec![0u8; CHUNK_LEN];
    time_it(
        &format!("composite all {total} chunks x {LAYERS} layers"),
        RUNS,
        || {
            for cy in 0..cy_total {
                for cx in 0..cx_total {
                    composite::composite_chunk(&layers, cx as u32, cy as u32, &mut out)
                        .expect("valid chunk coords");
                }
            }
        },
    );

    // Same chunks, same `composite_chunk`, different scheduling: chunk rows are
    // independent, so the rows go to rayon. This row exists so the cost of the
    // *parallelism* is readable on its own, not folded into the algorithm's.
    time_it(
        &format!("composite all {total} chunks x {LAYERS} layers (rayon)"),
        RUNS,
        || {
            (0..cy_total).into_par_iter().for_each(|cy| {
                let mut out = vec![0u8; CHUNK_LEN];
                for cx in 0..cx_total {
                    composite::composite_chunk(&layers, cx as u32, cy as u32, &mut out)
                        .expect("valid chunk coords");
                }
            });
        },
    );

    // The canvas-shaped entry point, which also scatters each chunk into the
    // wide RGBA buffer. Kept separate because it does strictly more work than
    // the two rows above (one full-canvas buffer instead of one chunk buffer).
    time_it(
        &format!("compositeCanvas {WIDTH}x{HEIGHT} (rayon)"),
        RUNS,
        || {
            let buf = composite_canvas(&layers, WIDTH, HEIGHT);
            assert_eq!(buf.len(), (WIDTH * HEIGHT * 4) as usize);
        },
    );
    time_it(
        &format!("compositeCanvas {WIDTH}x{HEIGHT} (1 thread)"),
        RUNS,
        || {
            let buf = composite_canvas_serial(&layers, WIDTH, HEIGHT);
            assert_eq!(buf.len(), (WIDTH * HEIGHT * 4) as usize);
        },
    );
}

fn bench_flatten_and_png() {
    let doc = make_busy_doc();
    let layers = doc.sources();
    let expected = (WIDTH * HEIGHT * 4) as usize;

    time_it("flattenToRgba 2048x1088", RUNS, || {
        let rgba = flatten_to_rgba(&layers, WIDTH, HEIGHT, FlattenOptions::default());
        assert_eq!(rgba.len(), expected);
    });

    // The single-threaded twin, so the parallelism is readable on its own.
    time_it("flattenToRgba 2048x1088 (1 thread)", RUNS, || {
        let rgba = flatten_to_rgba_serial(&layers, WIDTH, HEIGHT, FlattenOptions::default());
        assert_eq!(rgba.len(), expected);
    });

    // The JS `exportPngBytes` includes the flatten, so this one does too.
    time_it("exportPngBytes 2048x1088", PNG_RUNS, || {
        let rgba = flatten_to_rgba(&layers, WIDTH, HEIGHT, FlattenOptions::default());
        assert_eq!(rgba.len(), expected);
        let bytes = encode_png(&rgba, WIDTH, HEIGHT).expect("valid png");
        // Cheap sanity guard, the same spirit as the JS `rgba.length` check.
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    });
}

// ---------------------------------------------------------------------------
// Warm-up + main
// ---------------------------------------------------------------------------

/// One cheap pass over every measured path so the first sample is not the
/// sample that pays for cold caches and an empty footprint cache. The pen=64
/// stroke is deliberately shortened to two segments — it is the same code path,
/// which is what needs warming, and a full 20-segment warm-up would cost more
/// than the measurement itself.
fn warm_up() {
    let mut store = ChunkStore::new(WIDTH, HEIGHT).expect("valid canvas size");
    {
        let mut w = PixelWriter::new(&mut store, "layer-0001");
        for n in PEN_SIZES {
            for k in 0..2i32 {
                stroke_segment(
                    &mut w,
                    10,
                    (k * 13) % HEIGHT as i32,
                    WIDTH as i32 - 10,
                    (k * 29) % HEIGHT as i32,
                    n,
                    RED,
                );
            }
        }
        stroke_point(&mut w, 512, 512, 64, RED);
        let spec = ShapeSpec {
            kind: ShapeKind::Ellipse,
            stroke_width: 1,
            fill_mode: FillMode::Fill,
            bbox: Some([0, 0, 1024, 1024]),
            p0: None,
            p1: None,
            radius: 0,
            points: vec![],
        };
        apply_shape(&mut w, &spec, RED, BLUE).expect("valid shape spec");
        w.take_dirty();
    }
    let mut warm = ChunkStore::new(WIDTH, HEIGHT).expect("valid canvas size");
    {
        let mut w = PixelWriter::new(&mut warm, "layer-0001");
        flood_fill_scanline(&mut w, WIDTH as usize, HEIGHT as usize, 0, 0, 0, GREEN);
        w.take_dirty();
    }
    let doc = make_busy_doc();
    let layers = doc.sources();
    let mut out = vec![0u8; CHUNK_LEN];
    for cy in 0..(HEIGHT / CHUNK_PX) {
        for cx in 0..(WIDTH / CHUNK_PX) {
            composite::composite_chunk(&layers, cx, cy, &mut out).expect("valid chunk coords");
        }
    }
    let rgba = flatten_to_rgba(&layers, WIDTH, HEIGHT, FlattenOptions::default());
    encode_png(&rgba, WIDTH, HEIGHT).expect("valid png");
    println!("warm-up done\n");
}

fn main() {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    println!("[pixeldraw] Rust engine bench — {WIDTH}x{HEIGHT}, {LAYERS} layers");
    println!("cargo {PROFILE}");
    println!("pixel-engine v{VERSION} · items mirror tools/bench_baseline.mjs\n");

    warm_up();

    bench_strokes();
    bench_stroke_point();
    bench_flood_fill();
    bench_shapes();
    bench_composite();
    bench_flatten_and_png();

    println!("\nomitted: documentToJson (save) + json size — Phase 1.13 io/serialize");
}
