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
mod tests;
