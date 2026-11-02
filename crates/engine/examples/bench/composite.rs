//! Composite benchmark workloads.

use super::super::*;
use pixel_engine::composite::{self, composite_canvas, composite_canvas_serial};
use pixel_engine::io::export::{FlattenOptions, flatten_to_rgba, flatten_to_rgba_serial};
use pixel_engine::io::png::encode_png;
use rayon::prelude::*;

pub(crate) fn bench_composite() {
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

pub(crate) fn bench_flatten_and_png() {
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
