//! Flatten + export — the ported `src/io/export_png.js`.
//!
//! `flattenToRgba` is the **file** half of the compositor: it turns the layer
//! stack into the RGBA buffer a PNG is cut from. The display compositor in
//! [`crate::composite`] does the same arithmetic chunk by chunk; this one does
//! it pixel by pixel over the whole canvas, and that difference in shape is
//! exactly what `docs/perf_js_vs_rust.md` measures.
//!
//! **Blend parity.** Every pixel that is not caught by the direct-replace
//! shortcut goes through [`crate::blend::over_blend`] — the single blend
//! implementation the JS tree shared between display, file, eyedropper and
//! merge-down (docs/contract.md §1-1). Reimplementing the math here would let
//! the screen and the file disagree, which is the one thing the contract forbids.
//!
//! **Shape of the loop.** The arithmetic per pixel is the JS arithmetic, in the
//! JS order. What changed is the *iteration*: this walks chunk row → chunk →
//! 64-pixel word → pixel, so the layer stack is resolved once per chunk instead
//! of once per pixel per layer (1.4e8 `BTreeMap` probes for one flatten), and
//! the per-chunk coverage map rejects the 64-pixel words no layer touches.
//! Band-parallel because chunk rows share nothing.

use crate::blend::over_blend;
use crate::chunkstore::{AlphaMask, Chunk, EMPTY_MASK};
use crate::composite::LayerSource;
use crate::constants::{CHUNK_LEN, CHUNK_PX};
use crate::pixel::{Rgba, unpack_rgba};
use rayon::prelude::*;

/// Bytes one 64-pixel word covers: 64 pixels × RGBA.
const WORD_BYTES: usize = 64 * 4;

/// Export knobs, mirroring the JS `flattenToRgba(doc, opts)` object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlattenOptions {
    /// JS `includeBackground`.
    pub include_background: bool,
}

impl Default for FlattenOptions {
    fn default() -> Self {
        // JS: `{ includeBackground = true } = {}`.
        Self {
            include_background: true,
        }
    }
}

/// One layer resolved for one chunk, carrying everything the pixel loop reads.
///
/// `normal_opaque` is `mode == "normal" && opacity == 1.0`, hoisted out of the
/// pixel loop: the JS inner loop compared a string per (pixel, layer), which is
/// 1.4e8 string comparisons in one flatten.
struct Source<'a> {
    data: &'a Chunk,
    mask: &'a AlphaMask,
    opacity: f64,
    mode: &'a str,
    normal_opaque: bool,
}

/// `basePixel` — what every layer composites over.
///
/// The JS version resolved `canvas.background`: `[0,0,0,0]` when the caller
/// opted out or the document said `"transparent"`, otherwise `parseHex(bg)`.
/// **The hex branch is out of scope here** — this engine has no document model
/// yet (Phase 1.12 owns `canvas`), so there is nothing to parse and the base
/// pixel is transparent whichever way the flag points. That is also exactly
/// what the Phase 0.1 JS benchmark measured (its document was transparent), so
/// the two sides of `docs/perf_js_vs_rust.md` compare like with like. When
/// `model` lands, this is the one place a hex background gets read.
#[inline]
fn base_pixel(opts: FlattenOptions) -> Rgba {
    let _ = opts.include_background;
    [0, 0, 0, 0]
}

/// `flattenToRgba` — composite every visible layer into a full-canvas RGBA
/// buffer.
///
/// The arithmetic is deliberately the JS arithmetic: layers in bottom-to-top
/// array order (`layers[0]` is the bottom, so the last entry paints last), the
/// `mode === "normal" && sa8 === 255 && op === 1` direct-replace shortcut,
/// [`over_blend`] for everything else, and the "alpha 0 ⇒ clear RGB"
/// normalisation at the end so a transparent pixel never keeps a stale colour
/// (the same rule `is_all_zero` pruning depends on).
///
/// **What is different from the JS loop is only the iteration.** The JS original
/// called `store.getPixel(x, y)` once per layer per pixel — 1.4e8 `Map` probes
/// for one flatten of a 64-layer document, which is why this file used to be the
/// slowest item in the benchmark by an order of magnitude. Here the walk is
///
/// ```text
/// chunk row (parallel) → chunk → 64-pixel word → pixel → layer
/// ```
///
/// so the layer stack costs **one probe per layer per chunk** instead of one per
/// layer per pixel, and the coverage map decides which 64-pixel words any layer
/// touches at all. A word nobody touches can only leave the accumulator at the
/// base pixel, so it is written as a flat run instead of 64×64 alpha loads.
/// Bands are independent, so they go to rayon.
///
/// [`flatten_to_rgba_serial`] is the same thing on one thread, kept so the cost
/// of the parallelism can be read apart from the cost of the algorithm.
pub fn flatten_to_rgba(
    layers: &[LayerSource<'_>],
    width: u32,
    height: u32,
    opts: FlattenOptions,
) -> Vec<u8> {
    flatten_bands(layers, width, height, opts, true)
}

/// The single-threaded twin of [`flatten_to_rgba`].
pub fn flatten_to_rgba_serial(
    layers: &[LayerSource<'_>],
    width: u32,
    height: u32,
    opts: FlattenOptions,
) -> Vec<u8> {
    flatten_bands(layers, width, height, opts, false)
}

fn flatten_bands(
    layers: &[LayerSource<'_>],
    width: u32,
    height: u32,
    opts: FlattenOptions,
    parallel: bool,
) -> Vec<u8> {
    let base = base_pixel(opts);
    let w = width as usize;
    let band_bytes = (width * CHUNK_PX * 4) as usize;
    let mut out = vec![0u8; w * height as usize * 4];

    // `srcs` in JS: visible layers, `opacity > 0`, carrying a store. Chunk
    // *presence* is not filtered — an absent chunk reads back as pixel 0, and
    let visible: Vec<&LayerSource<'_>> = layers
        .iter()
        .filter(|l| l.is_visible() && !l.is_transparent() && l.store.is_some())
        .collect();

    let paint = |band: &mut [u8], cy: u32| {
        let cx_total = width / CHUNK_PX;
        let stride = width as usize * 4;
        let row_bytes = CHUNK_PX as usize * 4;
        let mut srcs: Vec<Source<'_>> = Vec::with_capacity(visible.len());
        let mut chunk_out = [0u8; CHUNK_LEN];
        for cx in 0..cx_total {
            srcs.clear();
            for l in &visible {
                let store = l.store.expect("filtered to Some(store) above");
                let Some((data, mask)) = store.get_chunk_with_mask(cx, cy) else {
                    continue;
                };
                srcs.push(Source {
                    data,
                    mask,
                    opacity: l.opacity,
                    mode: l.mode,
                    normal_opaque: l.mode == "normal" && l.opacity == 1.0,
                });
            }
            flatten_chunk(&srcs, base, &mut chunk_out);
            // A chunk is 32 pixels wide but a canvas row is `width` pixels, so
            // the chunk's own rows are **not** contiguous in the band: they land
            let col = cx as usize * row_bytes;
            for (r, slot) in chunk_out.chunks_exact(row_bytes).enumerate() {
                let dst = r * stride + col;
                band[dst..dst + row_bytes].copy_from_slice(slot);
            }
        }
    };

    if parallel {
        out.par_chunks_mut(band_bytes)
            .enumerate()
            .for_each(|(cy, band)| paint(band, cy as u32));
    } else {
        out.chunks_mut(band_bytes)
            .enumerate()
            .for_each(|(cy, band)| paint(band, cy as u32));
    }
    out
}

/// One chunk's worth of output.
///
/// Same three changes as [`crate::composite::composite_chunk`] and the same
/// reason: per-layer word skip, layer-outer/pixel-inner, and one coverage union
/// per chunk. The arithmetic — layer order, the `normal && sa == 255 && op == 1`
/// shortcut, [`over_blend`], the alpha-0 RGB clear — is the JS arithmetic, and
/// each pixel still walks the layers in the same order.
fn flatten_chunk(srcs: &[Source<'_>], base: Rgba, chunk_out: &mut [u8]) {
    if srcs.is_empty() {
        // Nothing here contributes, so every pixel is the base pixel with the
        // alpha-0 normalisation applied.
        let flat = normalised_base(base);
        for slot in chunk_out.as_chunks_mut::<4>().0.iter_mut() {
            slot.copy_from_slice(&flat);
        }
        return;
    }
    let union = coverage_union(srcs);
    for (w, bits) in union.iter().enumerate() {
        let at = w * WORD_BYTES;
        if *bits == 0 {
            // No layer has alpha here, so the accumulator never leaves `base`.
            let flat = normalised_base(base);
            for slot in chunk_out[at..at + WORD_BYTES]
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
            {
                slot.copy_from_slice(&flat);
            }
            continue;
        }
        // Start the word at the base pixel.
        for slot in chunk_out[at..at + WORD_BYTES]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
        {
            slot.copy_from_slice(&base);
        }
        for src in srcs {
            if src.mask[w] == 0 {
                continue;
            }
            let row = &src.data[at..at + WORD_BYTES];
            for (slot, px) in chunk_out[at..at + WORD_BYTES]
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(row.as_chunks::<4>().0.iter())
            {
                let sa8 = px[3];
                if sa8 == 0 {
                    continue;
                }
                let [sr, sg, sb, _] = unpack_rgba(u32::from_le_bytes([px[0], px[1], px[2], px[3]]));
                // Same normal-mode-only shortcut the display compositor uses.
                if src.normal_opaque && sa8 == 255 {
                    slot.copy_from_slice(&[sr, sg, sb, 255]);
                    continue;
                }
                let acc: Rgba = [slot[0], slot[1], slot[2], slot[3]];
                slot.copy_from_slice(&over_blend(acc, [sr, sg, sb, sa8], src.opacity, src.mode));
            }
        }
        for slot in chunk_out[at..at + WORD_BYTES]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
        {
            if slot[3] == 0 {
                slot[0] = 0;
                slot[1] = 0;
                slot[2] = 0;
            }
        }
    }
}

/// The base pixel after the alpha-0 normalisation: alpha 0 clears the RGB.
#[inline]
fn normalised_base(base: Rgba) -> Rgba {
    if base[3] == 0 { [0, 0, 0, 0] } else { base }
}

/// Which pixels any of these layers touches, as a coverage map.
///
/// As in the compositor, the union of a deep stack is nearly everything, so this
/// only pays off for shallow stacks; the per-layer word test inside
/// [`flatten_chunk`] is what filters a sparse document.
#[inline]
fn coverage_union(srcs: &[Source<'_>]) -> AlphaMask {
    let mut union = EMPTY_MASK;
    for s in srcs {
        for (u, m) in union.iter_mut().zip(s.mask.iter()) {
            *u |= *m;
        }
    }
    union
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunkstore::{ChunkStore, PixelWriter};
    use crate::constants::CHUNK_LEN;
    use crate::raster::brush::stroke_point;

    const RED: u32 = 0xff0000ff;
    /// Opaque blue: `a<<24 | b<<16 | g<<8 | r`.
    const BLUE: u32 = 0xffff0000;
    /// Half-alpha blue, the colour the JS benchmark alternates into its strokes.
    const BLUE_128: u32 = 0x80ff0000;

    /// A `size × size` store with one dot at `(x, y)`.
    fn dotted(color: u32, x: i32, y: i32, size: u32) -> ChunkStore {
        let mut store = ChunkStore::new(size, size).expect("valid size");
        {
            let mut w = PixelWriter::new(&mut store, "layer-0001");
            stroke_point(&mut w, x, y, 4, color);
        }
        store
    }

    /// A store whose chunk (0,0) holds one pixel with RGB set and alpha 0 — the
    /// shape a raw `putChunk` from a file, or an erase that left the colour
    /// bytes behind, produces.
    fn ghost_rgb() -> ChunkStore {
        let mut store = ChunkStore::new(32, 32).expect("valid size");
        let mut raw = Box::new([0u8; CHUNK_LEN]);
        raw[0] = 0x11;
        raw[1] = 0x22;
        raw[2] = 0x33;
        raw[3] = 0x00;
        store.put_chunk(0, 0, Some(&raw[..])).expect("valid chunk");
        store
    }

    // <META - ROLE : parity with tests/io_basic.test.mjs flattenToRgba cases | L1-40>
    #[test]
    fn the_shortcut_replaces_rather_than_blends() {
        let red = dotted(RED, 4, 4, 64);
        let blue = dotted(BLUE, 4, 4, 64);
        let layers = [LayerSource::plain(&red), LayerSource::plain(&blue)];
        let out = flatten_to_rgba(&layers, 64, 64, FlattenOptions::default());
        // Both are `normal` + opaque + opacity 1, so the shortcut fires and the
        // LAST layer wins outright — no source-over math, no rounded alpha.
        let o = (4 * 64 + 4) * 4;
        assert_eq!(&out[o..o + 4], &[0x00, 0x00, 0xff, 0xff]);
        // ... and it is byte-identical to what the display compositor says.
        let mut chunk = [0u8; CHUNK_LEN];
        crate::composite::composite_chunk(&layers, 0, 0, &mut chunk).expect("ok");
        assert_eq!(
            &out[o..o + 4],
            &chunk[(4 * 32 + 4) * 4..(4 * 32 + 4) * 4 + 4]
        );
    }

    #[test]
    fn a_blend_mode_refuses_the_shortcut() {
        let red = dotted(RED, 4, 4, 64);
        let blue = dotted(BLUE, 4, 4, 64);
        let multiply = LayerSource {
            mode: "multiply",
            ..LayerSource::plain(&blue)
        };
        let layers = [LayerSource::plain(&red), multiply];
        let out = flatten_to_rgba(&layers, 64, 64, FlattenOptions::default());
        let o = (4 * 64 + 4) * 4;
        // multiply on an opaque backdrop must run the math even though the
        // source is fully opaque; pure blue would mean the shortcut fired.
        assert_ne!(&out[o..o + 4], &[0x00, 0x00, 0xff, 0xff]);
    }

    #[test]
    fn half_opacity_refuses_the_shortcut() {
        let blue = dotted(BLUE, 4, 4, 64);
        let half_blue = LayerSource {
            opacity: 0.5,
            ..LayerSource::plain(&blue)
        };
        let layers = [half_blue];
        let out = flatten_to_rgba(&layers, 64, 64, FlattenOptions::default());
        let o = (4 * 64 + 4) * 4;
        // over_blend on a transparent backdrop: colour kept, alpha halved.
        assert_eq!(&out[o..o + 4], &[0x00, 0x00, 0xff, 0x80]);
    }

    #[test]
    fn alpha_zero_clears_rgb() {
        // A pixel whose alpha is 0 must come out all four bytes zero, or the
        // exported PNG would carry a colour nobody can see and the store could
        let out = flatten_to_rgba(
            &[LayerSource::plain(&ghost_rgb())],
            32,
            32,
            FlattenOptions::default(),
        );
        assert_eq!(&out[0..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn a_half_alpha_source_keeps_its_rgb() {
        let out = flatten_to_rgba(
            &[LayerSource::plain(&dotted(BLUE_128, 4, 4, 64))],
            64,
            64,
            FlattenOptions::default(),
        );
        let o = (4 * 64 + 4) * 4;
        assert_eq!(&out[o..o + 4], &[0x00, 0x00, 0xff, 0x80]);
    }

    #[test]
    fn layer_order_is_bottom_to_top() {
        let red = dotted(RED, 4, 4, 64);
        let blue = dotted(BLUE, 4, 4, 64);
        let o = (4 * 64 + 4) * 4;
        let blue_on_top = flatten_to_rgba(
            &[LayerSource::plain(&red), LayerSource::plain(&blue)],
            64,
            64,
            FlattenOptions::default(),
        );
        assert_eq!(&blue_on_top[o..o + 4], &[0x00, 0x00, 0xff, 0xff]);
        let red_on_top = flatten_to_rgba(
            &[LayerSource::plain(&blue), LayerSource::plain(&red)],
            64,
            64,
            FlattenOptions::default(),
        );
        assert_eq!(&red_on_top[o..o + 4], &[0xff, 0x00, 0x00, 0xff]);
    }

    #[test]
    fn hidden_and_transparent_layers_contribute_nothing() {
        let red = dotted(RED, 4, 4, 64);
        let o = (4 * 64 + 4) * 4;
        for layer in [
            LayerSource {
                visible: false,
                ..LayerSource::plain(&red)
            },
            LayerSource {
                opacity: 0.0,
                ..LayerSource::plain(&red)
            },
        ] {
            let out = flatten_to_rgba(&[layer], 64, 64, FlattenOptions::default());
            assert_eq!(&out[o..o + 4], &[0, 0, 0, 0]);
        }
    }

    #[test]
    fn no_layers_is_a_transparent_canvas() {
        let out = flatten_to_rgba(&[], 8, 8, FlattenOptions::default());
        assert_eq!(out.len(), 8 * 8 * 4);
        assert!(out.iter().all(|&b| b == 0));
    }

    #[test]
    fn the_buffer_is_width_times_height_times_four() {
        let red = dotted(RED, 0, 0, 64);
        let out = flatten_to_rgba(
            &[LayerSource::plain(&red)],
            64,
            64,
            FlattenOptions::default(),
        );
        assert_eq!(out.len(), 64 * 64 * 4);
        // Both option states agree today, because there is no background
        // colour to resolve (see `base_pixel`).
        let without = flatten_to_rgba(
            &[LayerSource::plain(&red)],
            64,
            64,
            FlattenOptions {
                include_background: false,
            },
        );
        assert_eq!(out, without);
    }
}
