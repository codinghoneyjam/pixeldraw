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
use crate::chunkstore::{Chunk, ChunkStore};
use crate::constants::{CHUNK_LEN, CHUNK_PX};
use crate::errors::{DrawToolError, EngineResult};
use crate::pixel::Rgba;

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
fn visible_sources<'a>(
    layers: &[LayerSource<'a>],
    cx: u32,
    cy: u32,
) -> Vec<(&'a Chunk, f64, &'a str)> {
    let mut out = Vec::with_capacity(layers.len());
    for l in layers {
        if !l.is_visible() || l.is_transparent() {
            continue;
        }
        let Some(store) = l.store else { continue };
        let Some(data) = store.get_chunk(cx, cy) else {
            continue;
        };
        out.push((data, l.opacity, l.mode));
    }
    out
}

/// `compositeChunk` — composite the visible layers into one 32×32 RGBA chunk.
///
/// Returns `true` when any output pixel ended up non-transparent.
///
/// **What the JS guards became.** `assertDoc` and `assertOut` both raised
/// `INVALID_STATE`; here the layer list is a slice and the output is a
/// `&mut [u8]` whose length is checked below, so the only guard left is the
/// buffer length, which still raises the same code.
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
    let mut any_visible = false;
    for i in 0..(CHUNK_PX * CHUNK_PX) as usize {
        let o = i * 4;
        let mut acc: Rgba = [0, 0, 0, 0];
        for (data, opacity, mode) in &srcs {
            let sa8 = data[o + 3];
            if sa8 == 0 {
                continue;
            }
            // The direct-replace shortcut is only valid for source-over: every
            // other mode has to run the math even when the source is opaque.
            if *mode == "normal" && sa8 == 255 && *opacity == 1.0 {
                acc = [data[o], data[o + 1], data[o + 2], 255];
                continue;
            }
            acc = over_blend(
                acc,
                [data[o], data[o + 1], data[o + 2], sa8],
                *opacity,
                mode,
            );
        }
        out[o] = acc[0];
        out[o + 1] = acc[1];
        out[o + 2] = acc[2];
        out[o + 3] = acc[3];
        if acc[3] != 0 {
            any_visible = true;
        } else {
            // Alpha 0 keeps RGB cleared, so a transparent chunk is all zeros
            // and `isAllZero` can prune it.
            out[o] = 0;
            out[o + 1] = 0;
            out[o + 2] = 0;
        }
    }
    Ok(any_visible)
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
    // (`panel_layers.js` `topFirst`), and `compositeChunk` accumulates in array
    // order, so the LAST entry paints last.
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
        // The composite must still leave the output all-zero, or `isAllZero`
        // could no longer prune it and the erased region would keep a trace.
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
