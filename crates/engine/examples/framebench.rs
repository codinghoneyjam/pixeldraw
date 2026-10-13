//! The **frame** benchmark — the one path no benchmark has ever measured.
//!
//! Run:
//!
//! ```text
//! cargo run -p pixel-engine --release --example framebench
//! ```
//!
//! `docs/perf_js_vs_rust.md` §11 measured the *engine* in isolation: strokes,
//! fills, a whole-canvas composite, a flatten. None of it is what the user waits
//! for. What they wait for is `show_canvas` in `crates/app/src/canvas_view.rs`,
//! once per egui frame, and the contract puts a **6 ms** budget on part of it
//! (`FRAME_BUDGET_MS`, docs/contract.md §1-2).
//!
//! This measures that frame stage by stage, so an optimisation can be aimed at a
//! number instead of at a hunch:
//!
//! ```text
//!   doc().cloned()            <- render_frame line 118
//!   comp.update(...)          <- repaint the dirty chunks
//!   draw_display_into(...)    <- paint the device framebuffer
//!   upload (framebuffer copy) <- CanvasTexture::upload
//! ```
//!
//! Four scenarios, because they have almost nothing in common:
//!
//! | scenario | what the user is doing |
//! |---|---|
//! | idle | nothing dirty — what every frame looks like when they are not drawing |
//! | drag, small pen | 2 chunks dirtied — the overwhelmingly common case |
//! | drag, big brush | ~60 chunks dirtied — a pen-64 sweep |
//! | cold open | every chunk dirty, once |

use std::hint::black_box;
use std::time::Instant;

use pixel_engine::comp_cache::{Budget, CompCache};
use pixel_engine::constants::FRAME_BUDGET_MS;
use pixel_engine::display::{DisplayParams, Framebuffer, GridMode, draw_display_into};
use pixel_engine::model::NewDocumentOptions;
use pixel_engine::model::document::new_document;
use pixel_engine::raster::brush::{stroke_point, stroke_segment};
use pixel_engine::view::{View, fit_view};

const W: u32 = 2048;
const H: u32 = 1088;
/// The window is 1280×820 CSS px (shell.rs), minus the toolbars in practice.
const CSS_W: f64 = 1040.0;
const CSS_H: f64 = 640.0;
const DPR: f64 = 1.0;

const RUNS: usize = 9;

fn median_ms(runs: usize, mut f: impl FnMut()) -> f64 {
    let mut s = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        f();
        s.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    s.sort_by(f64::total_cmp);
    s[s.len() / 2]
}

fn row(name: &str, ms: f64, budget: bool) {
    let verdict = if budget {
        let v = ms / FRAME_BUDGET_MS as f64;
        let mark = if v <= 1.0 { "OK  " } else { "OVER" };
        format!("  {v:>5.1}x {mark}")
    } else {
        String::new()
    };
    println!("{name:<44} | {ms:>8.3} ms |{verdict}");
}

fn header(title: &str) {
    println!("\n=== {title} ===");
    println!("{:<44} | {:>11} | vs 6ms budget", "stage", "median");
}

/// The busy document, built through the real painting path.
fn busy_document(layers: usize) -> pixel_engine::model::Document {
    let mut doc =
        new_document(NewDocumentOptions::default().size(W, H)).expect("valid canvas size");
    // Each layer needs its **own** id. `Layer::clone` copies the id too, so
    // cloning a layer and calling `get_layer_mut(id)` paints the same one 64
    // times and leaves the rest empty — which silently understates the very
    // number this benchmark exists to measure.
    while doc.layers.len() < layers {
        let n = doc.layers.len() + 1;
        let id = format!("layer-{n:04}");
        let mut l = doc.layers[0].clone();
        l.id = id.clone();
        l.name = format!("Layer {n}");
        doc.layers.push(l);
    }
    let ids: Vec<String> = doc.layers.iter().map(|l| l.id.clone()).collect();
    for (li, id) in ids.iter().enumerate() {
        let layer = doc.get_layer_mut(id).expect("layer id");
        let mut store = std::mem::replace(&mut layer.store, {
            pixel_engine::chunkstore::ChunkStore::new(W, H).expect("valid canvas size")
        });
        {
            use pixel_engine::chunkstore::PixelWriter;
            let mut w = PixelWriter::new(&mut store, id.clone());
            for k in 0..24i32 {
                stroke_segment(
                    &mut w,
                    (k * 97) % W as i32,
                    (k * 61) % H as i32,
                    ((k * 97) + 400) % W as i32,
                    ((k * 61) + 300) % H as i32,
                    5,
                    if k % 3 == 0 { 0x80ff_0000 } else { 0xff00_00ff },
                );
            }
            w.finish();
        }
        let layer = doc.get_layer_mut(id).expect("layer id");
        layer.store = store;
        layer.opacity = if li % 4 == 0 { 0.5 } else { 1.0 };
        layer.blend = [
            "normal", "multiply", "screen", "overlay", "darken", "lighten",
        ][li % 6];
    }
    doc
}

/// Dirty `n` chunks by painting into the last layer, then report which.
fn dirty_by_painting(doc: &mut pixel_engine::model::Document, pen: usize, span: i32) {
    let id = doc.layers.last().expect("a layer").id.clone();
    let layer = doc.get_layer_mut(&id).expect("layer id");
    let mut store = std::mem::replace(
        &mut layer.store,
        pixel_engine::chunkstore::ChunkStore::new(W, H).expect("valid canvas size"),
    );
    {
        use pixel_engine::chunkstore::PixelWriter;
        let mut w = PixelWriter::new(&mut store, id.clone());
        let mut x = 10i32;
        let mut touched: Vec<(u32, u32)> = Vec::new();
        while x < span {
            stroke_point(&mut w, x, 300, pen, 0x00ff00ff);
            x += (pen as i32).max(1);
        }
        touched.extend(w.take_dirty());
        w.finish();
    }
    let layer = doc.get_layer_mut(&id).expect("layer id");
    layer.store = store;
}

/// A frame, measured in its pieces. `visible` is `None` exactly as `session_glue`
/// passes it for a full repaint, and `Some(..)` as `canvas_view` does.
struct Frame {
    doc: pixel_engine::model::Document,
    comp: CompCache,
    fb: Framebuffer,
    view: View,
}

impl Frame {
    fn new(layers: usize) -> Self {
        let doc = busy_document(layers);
        let mut comp = CompCache::new();
        comp.ensure(W, H);
        let fb = Framebuffer::new(CSS_W as u32, CSS_H as u32);
        let view = fit_view(W as f64, H as f64, CSS_W, CSS_H);
        let mut f = Frame {
            doc,
            comp,
            fb,
            view,
        };
        f.cold_open();
        f
    }

    /// Every chunk dirty — what `invalidate_all` on a document replace does.
    fn cold_open(&mut self) {
        self.comp.invalidate_all();
        while self.comp.update_budget(&self.doc, None, Budget::frame()) {}
    }

    fn mark_dirty(&mut self, chunks: &[(u32, u32)]) {
        self.comp.invalidate_chunks(chunks);
    }

    fn params(&self) -> DisplayParams<'_> {
        DisplayParams {
            view: self.view,
            canvas_w: W,
            canvas_h: H,
            css_w: CSS_W,
            css_h: CSS_H,
            dpr: DPR,
            background: &self.doc.canvas.background,
            grid: GridMode::Off,
            comp: Some(self.comp.comp()),
        }
    }

    /// The four stages `show_canvas` runs, timed separately.
    fn measure_stages(&mut self) -> (f64, f64, f64, f64) {
        let clone = median_ms(RUNS, || {
            // `canvas_view.rs:118` — `state.session.doc().cloned()`
            let d = black_box(&self.doc).clone();
            black_box(&d);
        });
        let update = median_ms(RUNS, || {
            let mut c = self.comp.clone();
            c.invalidate_chunks(&[(10, 9)]);
            while c.update_budget(&self.doc, None, Budget::frame()) {}
        });
        let draw = median_ms(RUNS, || {
            let mut fb = std::mem::replace(&mut self.fb, Framebuffer::new(1, 1));
            draw_display_into(&self.params(), &mut fb);
            self.fb = fb;
        });
        // `CanvasTexture::upload` starts with `ColorImage::from_rgba_unmultiplied`,
        // which copies every byte before the GPU ever sees it.
        let upload = median_ms(RUNS, || {
            let src = black_box(&self.fb.rgba);
            let mut dst = vec![0u8; src.len()];
            dst.copy_from_slice(src);
            black_box(dst);
        });
        (clone, update, draw, upload)
    }
}

fn main() {
    println!("[pixeldraw] framebench — one egui frame, stage by stage");
    println!(
        "canvas {W}x{H}, device {:.0}x{:.0}, budget {FRAME_BUDGET_MS} ms\n",
        CSS_W, CSS_H
    );

    for layers in [1usize, 64] {
        let mut f = Frame::new(layers);
        let chunks = f
            .doc
            .layers
            .last()
            .map(|l| l.store.chunk_count())
            .unwrap_or(0);

        println!("\n############ {layers} layer(s), {chunks} chunks on the top layer ############");
        let (clone, update, draw, upload) = f.measure_stages();

        header("stages, one dirty chunk");
        row("doc().cloned()  (canvas_view.rs:118)", clone, true);
        row("comp.update()  repaint 1 chunk", update, true);
        row("draw_display_into()  whole device fb", draw, true);
        row("texture upload copy  whole device fb", upload, true);
        let total = clone + update + draw + upload;
        row("TOTAL", total, true);

        // A real drag: how many chunks a pen-8 sweep over 900 px dirties.
        let mut drag = Frame::new(layers);
        dirty_by_painting(&mut drag.doc, 8, 900);
        let n = drag.doc.layers.last().expect("layer").store.chunk_count();
        let mut keys: Vec<(u32, u32)> = Vec::new();
        drag.doc
            .layers
            .last()
            .expect("layer")
            .store
            .for_each_chunk(|cx, cy, _| keys.push((cx, cy)));
        println!("\n(pen-8 sweep left {n} chunks; the drag scenario dirties a slice of them)");

        drag.mark_dirty(&keys[..keys.len().min(60)]);
        let t60 = median_ms(RUNS, || {
            let mut c = drag.comp.clone();
            c.invalidate_chunks(&keys[..keys.len().min(60)]);
            while c.update_budget(&drag.doc, None, Budget::frame()) {}
        });
        header("comp.update() with a drag's worth of dirty chunks");
        row(
            "60 dirty chunks, 1 layer",
            median_ms(RUNS, || {
                let mut c = drag.comp.clone();
                c.invalidate_chunks(&keys[..keys.len().min(60)]);
                while c.update_budget(&drag.doc, None, Budget::frame()) {}
            }),
            true,
        );
        let _ = t60;

        let mut cold = Frame {
            doc: busy_document(layers),
            comp: CompCache::new(),
            fb: Framebuffer::new(CSS_W as u32, CSS_H as u32),
            view: fit_view(W as f64, H as f64, CSS_W, CSS_H),
        };
        cold.comp.ensure(W, H);
        cold.comp.invalidate_all();
        let cold_ms = median_ms(1, || {
            while cold.comp.update_budget(&cold.doc, None, Budget::frame()) {}
        });
        header("cold open — every chunk dirty");
        row("first frame after a document", cold_ms, true);
    }

    println!("\nNote: `Budget::frame()` is 6 ms and applies only to the off-screen");
    println!("pass in paint_dirty. `doc().cloned()`, draw_display_into and the");
    println!("texture upload are outside it, which is exactly why a frame can");
    println!("blow the budget no matter how well the budget behaves.");
}
