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
//!   comp.update(...)          <- repaint the dirty chunks (PERF-1: pass 1
//!                                composites visible chunks on rayon; the
//!                                blits stay ordered on the calling thread)
//!   draw_display_into(...)    <- paint the device framebuffer (UI-5: skipped
//!                                when the stamp and the overlay are unchanged)
//!   texture upload            <- CanvasTexture::present (UI-5: whole device fb
//!                                on a stamp change, one damage box otherwise,
//!                                nothing when skipped)
//! ```
//!
//! The frame used to clone the document and upload the whole framebuffer every
//! frame; both are gone (`render_frame` borrows, `UploadTracker::begin` decides),
//! so the rows below measure what actually runs.
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
use pixel_engine::view::{View, fit_view, visible_chunk_range};

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

/// The stages one changed frame runs, in the order `show_canvas` runs them.
struct Stages {
    /// `comp.update()` — repaint one dirty chunk.
    update: f64,
    /// `draw_display_into()` — the whole device framebuffer.
    draw: f64,
    /// The upload's CPU half (`ColorImage::from_rgba_unmultiplied` copy) for
    /// the whole device fb — paid when the stamp changed.
    upload: f64,
    /// The same copy for a hover frame's damage box: the pen crosshair
    /// (`r = 5*dpr` around the cursor) as one small box.
    hover: f64,
    /// The footprint wash at `zoom >= 2`: `pen-32 * zoom-2` device px.
    wash: f64,
}

/// UI-5's region copy — the same row gathering `CanvasTexture::upload_region`
/// does (`w*h*4` bytes out of a strided framebuffer).
fn region_copy_ms(fb: &Framebuffer, w: usize, h: usize) -> f64 {
    median_ms(RUNS, || {
        let src = black_box(&fb.rgba);
        let stride = fb.width as usize * 4;
        let mut dst = vec![0u8; w * h * 4];
        for row in 0..h {
            dst[row * w * 4..(row + 1) * w * 4]
                .copy_from_slice(&src[row * stride..row * stride + w * 4]);
        }
        black_box(dst);
    })
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

    /// The frame stages `show_canvas` runs, timed separately.
    fn measure_stages(&mut self) -> Stages {
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
        let hover = region_copy_ms(&self.fb, 16, 16);
        let wash = region_copy_ms(&self.fb, 64, 64);
        Stages {
            update,
            draw,
            upload,
            hover,
            wash,
        }
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
        let s = f.measure_stages();

        header("stages, one dirty chunk");
        row("comp.update()  repaint 1 chunk", s.update, true);
        row("draw_display_into()  whole device fb", s.draw, true);
        row("texture upload copy  whole device fb", s.upload, true);
        row(
            "TOTAL (redraw + full upload)",
            s.update + s.draw + s.upload,
            true,
        );

        header("UI-5 — the upload a hover frame pays instead");
        row("crosshair box 16x16", s.hover, false);
        row("footprint wash 64x64 (zoom>=2)", s.wash, false);

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
        header("comp.update() with a drag's worth of dirty chunks");
        row(
            "60 dirty chunks, 1 layer (session_glue: no visible range)",
            median_ms(RUNS, || {
                let mut c = drag.comp.clone();
                c.invalidate_chunks(&keys[..keys.len().min(60)]);
                while c.update_budget(&drag.doc, None, Budget::frame()) {}
            }),
            true,
        );

        // PERF-1: pass 1 — the visible chunks, no budget — which is what
        // `canvas_view` actually drives. The twin runs the same frame with the
        // composites on one thread; everything else is identical.
        let vis = visible_chunk_range(f.view, W as f64, H as f64, CSS_W, CSS_H);
        let fit = vis.map(|v| ((v.cx1 - v.cx0 + 1) as usize) * ((v.cy1 - v.cy0 + 1) as usize));
        let drag_layers = drag.doc.layer_sources();
        let n60 = keys.len().min(60);
        let mut drag_par = drag.comp.clone();
        let mut drag_ser = drag.comp.clone();
        println!(
            "\n(fit view sees {fit:?} of the {}x{} chunk grid)",
            W / 32,
            H / 32
        );
        header("PERF-1 — pass 1: shipped scheduler vs one thread");
        row(
            &format!("drag, {n60} visible chunks x {layers}L (shipped)"),
            median_ms(RUNS, || {
                drag_par.invalidate_chunks(&keys[..n60]);
                drag_par.paint_dirty_budget(&drag_layers, vis, Budget::Unlimited);
                black_box(drag_par.comp());
            }),
            false,
        );
        row(
            &format!("drag, {n60} visible chunks x {layers}L (1-thread twin)"),
            median_ms(RUNS, || {
                drag_ser.invalidate_chunks(&keys[..n60]);
                drag_ser.paint_dirty_budget_serial(&drag_layers, vis, Budget::Unlimited);
                black_box(drag_ser.comp());
            }),
            false,
        );

        // The cold grid: every chunk visible and dirty, one pass.
        let cold_layers = f.doc.layer_sources();
        let mut cold_par = CompCache::new();
        cold_par.ensure(W, H);
        let mut cold_ser = cold_par.clone();
        let all: Vec<(u32, u32)> = (0..H / 32)
            .flat_map(|cy| (0..W / 32).map(move |cx| (cx, cy)))
            .collect();
        let n_all = all.len();
        row(
            &format!("cold grid, {n_all} visible chunks x {layers}L (shipped)"),
            median_ms(5, || {
                cold_par.invalidate_chunks(&all);
                cold_par.paint_dirty_budget(&cold_layers, vis, Budget::Unlimited);
                black_box(cold_par.comp());
            }),
            false,
        );
        row(
            &format!("cold grid, {n_all} visible chunks x {layers}L (1-thread twin)"),
            median_ms(5, || {
                cold_ser.invalidate_chunks(&all);
                cold_ser.paint_dirty_budget_serial(&cold_layers, vis, Budget::Unlimited);
                black_box(cold_ser.comp());
            }),
            false,
        );

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
    println!("pass in paint_dirty. Pass 1 (every visible chunk) paints regardless —");
    println!("which is the pass PERF-1 parallelises. draw_display_into and the");
    println!("texture upload sit outside the budget too: UI-5 makes them");
    println!("conditional (skip when nothing changed) rather than faster, and turns");
    println!("a hover frame's upload into one damage box.");
}
