//! Display/export compositing — ported from `src/render/composite.js`.
//!
//! This is the **read** half of the engine: it turns a stack of layer bitmaps
//! into the pixels the screen, the PNG export and the eyedropper all see.
//! Background handling is deliberately absent here, exactly as in the JS tree
//! (docs/render.md).
//!
//! Every path that reads pixels funnels through [`composite_chunk`] or
//! [`sample_pixel`], so a colour cannot differ between screen, file and
//! eyedropper (docs/contract.md §1-1). The blend math itself is
//! [`crate::blend::over_blend`]; this module only decides *which* layers
//! contribute and in what order.

use crate::blend::over_blend;
use crate::chunkstore::{AlphaMask, EMPTY_MASK};
use crate::chunkstore::{Chunk, ChunkStore};
use crate::constants::{CHUNK_LEN, CHUNK_PX};
use crate::errors::{DrawToolError, EngineResult};
use crate::pixel::Rgba;
use rayon::prelude::*;

/// Bytes one 64-pixel word covers: 64 pixels × RGBA.
const WORD_BYTES: usize = 64 * 4;

/// What the compositor needs to know about one layer.
///
/// A plain struct rather than a trait on purpose: `model::Layer` builds one with
/// a one-line `as_source()`, so the dependency direction stays `composite ← model`
/// instead of turning into a cycle. `Copy` so the layer list can be walked by
/// value, and it carries no allocation.
#[derive(Debug, Clone, Copy)]
pub struct LayerSource<'a> {
    pub store: Option<&'a ChunkStore>,
    /// 0..=1. The JS tree treated anything `<= 0` as invisible.
    pub opacity: f64,
    /// The layer blend mode. `""` and `"normal"` both mean plain source-over.
    pub mode: &'a str,
    pub visible: bool,
}

impl LayerSource<'_> {
    /// A visible, opaque, `normal`-mode layer — the common case.
    pub fn plain(store: &ChunkStore) -> LayerSource<'_> {
        LayerSource {
            store: Some(store),
            opacity: 1.0,
            mode: "normal",
            visible: true,
        }
    }

    /// `layer.visible !== true` in JS — only literal `true` counts as visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// `!(opacity > 0)` in JS — false means "this layer contributes nothing".
    ///
    /// Spelled out rather than as `opacity <= 0.0` because the two disagree on
    /// NaN: `!(NaN > 0)` is `true`, so a NaN opacity hides the layer. That is
    /// the JS behaviour and the safe one, so it is written down here instead of
    /// being left to a reader's guess.
    pub fn is_transparent(&self) -> bool {
        self.opacity.is_nan() || self.opacity <= 0.0
    }
}

/// `visibleSources` — invisible, fully transparent and chunk-less layers drop.
///
/// The chunk is resolved **once per layer per call**, not once per pixel: that
/// is what the JS did, and it also means a layer that never wrote to this chunk
/// costs one map lookup instead of 1024.
fn visible_sources<'a>(layers: &[LayerSource<'a>], cx: u32, cy: u32) -> Vec<Src<'a>> {
    let mut out = Vec::with_capacity(layers.len());
    for l in layers {
        if !l.is_visible() || l.is_transparent() {
            continue;
        }
        let Some(store) = l.store else { continue };
        let Some((data, mask)) = store.get_chunk_with_mask(cx, cy) else {
            continue;
        };
        out.push(Src {
            data,
            mask,
            opacity: l.opacity,
            mode: l.mode,
            normal_opaque: l.mode == "normal" && l.opacity == 1.0,
        });
    }
    out
}

/// One layer resolved **once per chunk**, carrying everything the pixel loop
/// needs so the loop never goes back to the store.
///
/// Two things are hoisted here that used to be recomputed 1024 times:
///
/// * the chunk itself, which the JS `visibleSources` already hoisted — now
///   hoisted together with its [`AlphaMask`];
/// * `mode == "normal" && opacity == 1.0`, which was a **string comparison per
///   (pixel, layer)** — 6.6e7 of them for one full-canvas composite of a
///   64-layer document.
struct Src<'a> {
    data: &'a Chunk,
    mask: &'a AlphaMask,
    opacity: f64,
    mode: &'a str,
    /// The shortcut precondition, minus the per-pixel alpha test.
    normal_opaque: bool,
}

/// `compositeChunk` — composite the visible layers into one 32×32 RGBA chunk.
///
/// Returns `true` when any output pixel ended up non-transparent.
///
/// **What the JS guards became.** `assertDoc` and `assertOut` both raised
/// `INVALID_STATE`; here the layer list is a slice and the output is a
/// `&mut [u8]` whose length is checked below, so the only guard left is the
/// buffer length, which still raises the same code.
///
/// **Three changes, none of them to the arithmetic.** Each pixel still walks
/// the same layers in the same order, through the same `over_blend`, with the
/// same shortcut and the same "alpha 0 clears RGB" normalisation — only the
/// order in which *independent* pixels are visited changed, and pixels are
/// independent by construction.
///
/// 1. **Per-layer word skip.** Inside a 64-pixel word, a layer whose coverage
///    map has no bit set there cannot contribute, so it is skipped wholesale:
///    one `u64` test instead of 64 alpha loads and 64 branches. On a sparse
///    document that is ~95% of the pairs.
/// 2. **Layer-outer, pixel-inner.** The old nest touched 64 *different* 4 KB
///    buffers per output pixel. Interchanged, each layer streams one 256-byte
///    run of its own chunk and the output is walked once per layer instead of
///    once per pixel — sequential on both sides, and a loop shape LLVM can
///    vectorise at the x86-64 baseline.
/// 3. **Coverage map union, once per chunk.** A word no layer touches can only
///    leave the accumulator at `[0,0,0,0]`, so it is cleared with one `fill`
///    instead of 64×64 comparisons.
pub fn composite_chunk(
    layers: &[LayerSource<'_>],
    cx: u32,
    cy: u32,
    out: &mut [u8],
) -> EngineResult<bool> {
    if out.len() != CHUNK_LEN {
        return Err(DrawToolError::invalid_state(format!(
            "out must be a {CHUNK_LEN}-byte RGBA chunk, got {}",
            out.len()
        )));
    }
    let srcs = visible_sources(layers, cx, cy);
    if srcs.is_empty() {
        out.fill(0);
        return Ok(false);
    }
    let union = coverage_union(&srcs);
    let mut any_visible = false;
    for (w, bits) in union.iter().enumerate() {
        let base = w * WORD_BYTES;
        if *bits == 0 {
            out[base..base + WORD_BYTES].fill(0);
            continue;
        }
        // Every pixel of the word starts at the transparent accumulator.
        out[base..base + WORD_BYTES].fill(0);
        for s in &srcs {
            if s.mask[w] == 0 {
                continue;
            }
            let row = &s.data[base..base + WORD_BYTES];
            for (slot, px) in out[base..base + WORD_BYTES]
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(row.as_chunks::<4>().0.iter())
            {
                let sa8 = px[3];
                if sa8 == 0 {
                    continue;
                }
                // The direct-replace shortcut is only valid for source-over:
                // every other mode has to run the math even when the source is
                if s.normal_opaque && sa8 == 255 {
                    slot.copy_from_slice(&[px[0], px[1], px[2], 255]);
                    continue;
                }
                let acc: Rgba = [slot[0], slot[1], slot[2], slot[3]];
                slot.copy_from_slice(&over_blend(
                    acc,
                    [px[0], px[1], px[2], sa8],
                    s.opacity,
                    s.mode,
                ));
            }
        }
        for slot in out[base..base + WORD_BYTES]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
        {
            if slot[3] != 0 {
                any_visible = true;
            } else {
                // Alpha 0 keeps RGB cleared, so a transparent chunk is all zeros
                // and `is_all_zero` can prune it.
                slot[0] = 0;
                slot[1] = 0;
                slot[2] = 0;
            }
        }
    }
    Ok(any_visible)
}

/// Which pixels *any* of these layers touches, as a coverage map.
///
/// `OR` is the right operator for the composite: a pixel is worth visiting if at
/// least one layer has alpha there. A pixel no layer covers starts at the
/// transparent accumulator and is written as four zero bytes, so skipping it is
/// not an approximation — it is the same answer.
///
/// Note what this does **not** buy on a deep stack: 64 layers each covering a
/// quarter of the canvas OR together to nearly every word, so the union is
/// dense and the real filtering has to happen per layer. That is what the
/// per-layer word test inside [`composite_chunk`] is for.
#[inline]
fn coverage_union(srcs: &[Src<'_>]) -> AlphaMask {
    let mut union = EMPTY_MASK;
    for s in srcs {
        for (u, m) in union.iter_mut().zip(s.mask.iter()) {
            *u |= *m;
        }
    }
    union
}

/// Composite every chunk of the canvas into one flat `width*height*4` buffer.
///
/// Chunk rows are independent, so the bands go to rayon. This is the
/// whole-canvas shape (a frame rebuild, a PNG export); painting a few visible
/// chunks stays on [`composite_chunk`], which is single-threaded on purpose —
/// the UI thread owns the store and cannot lend it out.
pub fn composite_canvas(layers: &[LayerSource<'_>], width: u32, height: u32) -> Vec<u8> {
    let cx_total = (width / CHUNK_PX) as usize;
    let cy_total = (height / CHUNK_PX) as usize;
    let nchunks = cx_total * cy_total;
    // Work at chunk granularity: the flat chunk index is canvas-neutral — it
    // only describes *which* chunk to composite — and each chunk writes its own
    // 32×32 RGBA block, so the per-chunk buffers never overlap.
    let mut flat = vec![[0u8; CHUNK_LEN]; nchunks];
    flat.par_iter_mut().enumerate().for_each(|(i, chunk)| {
        let cx = (i % cx_total) as u32;
        let cy = (i / cx_total) as u32;
        let _ = composite_chunk(layers, cx, cy, chunk);
    });
    // Scatter the independent chunks back into the canvas-strided buffer.
    let mut buf = vec![0u8; (width * height * 4) as usize];
    let stride = (width * 4) as usize;
    let row_bytes = CHUNK_PX as usize * 4;
    for cy in 0..cy_total {
        for cx in 0..cx_total {
            let chunk = &flat[cy * cx_total + cx];
            let col = cx * row_bytes;
            for r in 0..CHUNK_PX as usize {
                let src = r * row_bytes;
                let dst = (cy * CHUNK_PX as usize + r) * stride + col;
                buf[dst..dst + row_bytes].copy_from_slice(&chunk[src..src + row_bytes]);
            }
        }
    }
    buf
}

/// The single-threaded twin of [`composite_canvas`]: the same bands, the same
/// order, no rayon. It exists so the cost of the parallelism can be measured
/// apart from the cost of the algorithm, and for callers already on a worker.
pub fn composite_canvas_serial(layers: &[LayerSource<'_>], width: u32, height: u32) -> Vec<u8> {
    let mut buf = vec![0u8; (width * height * 4) as usize];
    paint_bands(layers, width, &mut buf, false);
    buf
}

/// Composite chunk row `cy` of `buf`, band by band.
fn paint_bands(layers: &[LayerSource<'_>], width: u32, buf: &mut [u8], parallel: bool) {
    let cx_total = (width / CHUNK_PX) as usize;
    let band_bytes = (width * CHUNK_PX * 4) as usize;
    let stride = (width * 4) as usize;
    let row_bytes = CHUNK_PX as usize * 4;
    let paint = |band: &mut [u8], cy: u32| {
        let mut chunk = [0u8; CHUNK_LEN];
        for cx in 0..cx_total {
            let _ = composite_chunk(layers, cx as u32, cy, &mut chunk);
            // A chunk's rows are 32 pixels wide but a canvas row is `width`
            // pixels, so they land `stride` bytes apart in the band rather than
            let col = cx * row_bytes;
            for (r, slot) in chunk.chunks_exact(row_bytes).enumerate() {
                let dst = r * stride + col;
                band[dst..dst + row_bytes].copy_from_slice(slot);
            }
        }
    };
    if parallel {
        buf.par_chunks_mut(band_bytes)
            .enumerate()
            .for_each(|(cy, band)| paint(band, cy as u32));
    } else {
        buf.chunks_mut(band_bytes)
            .enumerate()
            .for_each(|(cy, band)| paint(band, cy as u32));
    }
}

/// `samplePixel` — the composite colour under one canvas pixel.
///
/// `width`/`height` are the canvas size; a pixel outside them (or a negative
/// coordinate, which the JS `Number.isInteger` guard also rejected) reads back
/// fully transparent.
pub fn sample_pixel(layers: &[LayerSource<'_>], x: i32, y: i32, width: i32, height: i32) -> Rgba {
    if x < 0 || y < 0 || x >= width || y >= height {
        return [0, 0, 0, 0];
    }
    let px = CHUNK_PX as i32;
    let cx = (x / px) as u32;
    let cy = (y / px) as u32;
    let o = (((y - cy as i32 * px) * px + (x - cx as i32 * px)) * 4) as usize;
    let mut acc: Rgba = [0, 0, 0, 0];
    for l in layers {
        if !l.is_visible() || l.is_transparent() {
            continue;
        }
        let Some(store) = l.store else { continue };
        let Some(data) = store.get_chunk(cx, cy) else {
            continue;
        };
        let sa8 = data[o + 3];
        if sa8 == 0 {
            continue;
        }
        if l.mode == "normal" && sa8 == 255 && l.opacity == 1.0 {
            acc = [data[o], data[o + 1], data[o + 2], 255];
            continue;
        }
        acc = over_blend(
            acc,
            [data[o], data[o + 1], data[o + 2], sa8],
            l.opacity,
            l.mode,
        );
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunkstore::ChunkStore;
    use crate::chunkstore::PixelWriter;
    use crate::raster::brush::stroke_point;

    const RED: u32 = 0xff0000ff;
    // a<<24 | b<<16 | g<<8 | r — 0x0000ffff would be *transparent* (alpha 0),
    // which silently drops the layer out of the composite.
    const BLUE: u32 = 0xffff0000;

    /// A 64×64 store with one dot at `(x, y)` in the given colour.
    fn dotted(color: u32, x: i32, y: i32) -> ChunkStore {
        let mut store = ChunkStore::new(64, 64).expect("valid size");
        {
            let mut w = PixelWriter::new(&mut store, "layer-0001");
            stroke_point(&mut w, x, y, 4, color);
        }
        store
    }

    // <META - ROLE : parity with tests/composite.test.mjs layer-order cases | L20-60>
    // `doc.layers` is bottom-to-top: the panel reverses it to draw top-first
    #[test]
    fn visible_source_order_decides_the_top_layer() {
        let red = dotted(RED, 32, 32);
        let blue = dotted(BLUE, 32, 32);
        let mut out = [0u8; CHUNK_LEN];

        let blue_on_top = [LayerSource::plain(&red), LayerSource::plain(&blue)];
        let any = composite_chunk(&blue_on_top, 1, 1, &mut out).expect("ok");
        assert!(any);
        assert_eq!(&out[32 * 4..32 * 4 + 4], &[0, 0, 0xff, 0xff]);

        let red_on_top = [LayerSource::plain(&blue), LayerSource::plain(&red)];
        composite_chunk(&red_on_top, 1, 1, &mut out).expect("ok");
        assert_eq!(&out[32 * 4..32 * 4 + 4], &[0xff, 0, 0, 0xff]);
    }

    #[test]
    fn an_empty_layer_contributes_nothing() {
        let red = dotted(RED, 0, 0);
        let empty = ChunkStore::new(64, 64).expect("valid size");
        let mut out = [0u8; CHUNK_LEN];
        let layers = [LayerSource::plain(&red), LayerSource::plain(&empty)];
        assert!(composite_chunk(&layers, 0, 0, &mut out).expect("ok"));
        assert_eq!(&out[0..4], &[0xff, 0, 0, 0xff]);
        // The empty layer never allocated a chunk, so it cost one map miss.
        assert!(empty.get_chunk(0, 0).is_none());
        assert_eq!(empty.chunk_count(), 0);
    }

    #[test]
    fn no_visible_layers_is_a_transparent_chunk() {
        let red = dotted(RED, 4, 4);
        let hidden = LayerSource {
            visible: false,
            ..LayerSource::plain(&red)
        };
        let transparent = LayerSource {
            opacity: 0.0,
            ..LayerSource::plain(&red)
        };
        let mut out = [0xffu8; CHUNK_LEN];
        assert!(!composite_chunk(&[hidden], 0, 0, &mut out).expect("ok"));
        assert!(
            out.iter().all(|&b| b == 0),
            "hidden layers clear the buffer"
        );
        assert!(!composite_chunk(&[transparent], 0, 0, &mut out).expect("ok"));
    }

    #[test]
    fn semi_transparent_layer_blends_through_the_opacity() {
        let red = dotted(RED, 8, 8);
        let mut out = [0u8; CHUNK_LEN];
        let half = LayerSource {
            opacity: 0.5,
            ..LayerSource::plain(&red)
        };
        assert!(composite_chunk(&[half], 0, 0, &mut out).expect("ok"));
        // over_blend on a transparent backdrop: keep the colour, halve the alpha.
        // rnd(0.5 * 255) = floor(128.0) = 128, not 127 — the half goes up.
        let o = 8 * CHUNK_PX as usize * 4 + 8 * 4;
        assert_eq!(&out[o..o + 4], &[0xff, 0, 0, 128]);
        assert_eq!(sample_pixel(&[half], 8, 8, 64, 64), [0xff, 0, 0, 128]);
    }

    #[test]
    fn a_blend_mode_runs_the_math_even_when_the_source_is_opaque() {
        let red = dotted(RED, 8, 8);
        let blue = dotted(BLUE, 8, 8);
        let mut out = [0u8; CHUNK_LEN];
        let multiply = LayerSource {
            mode: "multiply",
            ..LayerSource::plain(&red)
        };
        let layers = [LayerSource::plain(&blue), multiply];
        assert!(composite_chunk(&layers, 0, 0, &mut out).expect("ok"));
        let o = 8 * CHUNK_PX as usize * 4 + 8 * 4;
        // multiply on a fully opaque backdrop must not hit the direct-replace
        // shortcut, or the colour would stay pure red.
        assert_ne!(&out[o..o + 4], &[0xff, 0, 0, 0xff]);
    }

    #[test]
    fn sample_pixel_out_of_canvas_is_transparent() {
        let red = dotted(RED, 0, 0);
        let layers = [LayerSource::plain(&red)];
        assert_eq!(sample_pixel(&layers, -1, 0, 64, 64), [0, 0, 0, 0]);
        assert_eq!(sample_pixel(&layers, 64, 0, 64, 64), [0, 0, 0, 0]);
        assert_eq!(sample_pixel(&layers, 0, 64, 64, 64), [0, 0, 0, 0]);
        assert_eq!(sample_pixel(&layers, 0, 0, 64, 64), [0xff, 0, 0, 0xff]);
    }

    #[test]
    fn sample_pixel_matches_composite_chunk() {
        // (33, 40) lives in chunk (1, 1) at local (1, 8) — the chunk buffer is
        // chunk-relative, not canvas-relative.
        let red = dotted(RED, 33, 40);
        let blue = dotted(BLUE, 33, 40);
        let half = LayerSource {
            opacity: 0.5,
            ..LayerSource::plain(&blue)
        };
        let layers = [LayerSource::plain(&red), half];
        let mut out = [0u8; CHUNK_LEN];
        composite_chunk(&layers, 1, 1, &mut out).expect("ok");
        // Chunk-local offset: row 8, column 1.
        let o = (8 * CHUNK_PX as usize + 1) * 4;
        assert_eq!(&out[o..o + 4], sample_pixel(&layers, 33, 40, 64, 64));
    }

    #[test]
    fn a_wrong_sized_output_buffer_is_a_coded_error() {
        let red = dotted(RED, 0, 0);
        let layers = [LayerSource::plain(&red)];
        let mut small = [0u8; 16];
        match composite_chunk(&layers, 0, 0, &mut small) {
            Err(e) => assert_eq!(e.code, crate::errors::CODE_INVALID_STATE),
            Ok(_) => panic!("a 16-byte buffer must be rejected"),
        }
    }

    #[test]
    fn a_transparent_result_clears_its_channels() {
        // A chunk can legitimately hold a pixel with RGB set and alpha 0 (a
        // raw `putChunk` from a file, or an erase that left the colour bytes).
        let mut store = ChunkStore::new(64, 64).expect("valid size");
        let mut raw = Box::new([0u8; CHUNK_LEN]);
        raw[0] = 0x11;
        raw[1] = 0x22;
        raw[2] = 0x33;
        raw[3] = 0x00;
        store.put_chunk(0, 0, Some(&raw[..])).expect("valid chunk");

        let mut out = [0xffu8; CHUNK_LEN];
        assert!(!composite_chunk(&[LayerSource::plain(&store)], 0, 0, &mut out).expect("ok"));
        assert!(
            out.iter().all(|&b| b == 0),
            "RGB must be cleared with alpha"
        );
    }
}
