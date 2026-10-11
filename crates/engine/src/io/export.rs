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

use crate::blend::over_blend;
use crate::chunkstore::ChunkStore;
use crate::composite::LayerSource;
use crate::pixel::{Rgba, unpack_rgba};

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

/// One layer resolved into the three things the inner loop reads, mirroring the
/// JS `srcs.push({ store, opacity, mode })`.
struct Source<'a> {
    store: &'a ChunkStore,
    opacity: f64,
    mode: &'a str,
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
/// The loop is deliberately the JS loop: **per pixel**, layers in
/// bottom-to-top array order (`layers[0]` is the bottom, so the last entry
/// paints last), the `mode === "normal" && sa8 === 255 && op === 1`
/// direct-replace shortcut, [`over_blend`] for everything else, and the
/// "alpha 0 ⇒ clear RGB" normalisation at the end so a transparent pixel never
/// keeps a stale colour (the same rule `isAllZero` pruning depends on).
///
/// Unlike the display compositor this does **not** hoist the chunk lookup out
/// of the pixel loop — the JS original called `store.getPixel(x, y)` once per
/// layer per pixel, and that is what this port does. The cost of that choice is
/// the single biggest number in `docs/perf_js_vs_rust.md` and Phase 5.3
/// (run-level skipping) is where it goes away.
pub fn flatten_to_rgba(
    layers: &[LayerSource<'_>],
    width: u32,
    height: u32,
    opts: FlattenOptions,
) -> Vec<u8> {
    let base = base_pixel(opts);
    let w = width as usize;
    let h = height as usize;
    let mut out = vec![0u8; w * h * 4];

    // `srcs` in JS: visible layers, `opacity > 0`, carrying a store. Chunk
    // *presence* is not filtered — an absent chunk reads back as pixel 0, and
    // the `sa8 === 0` skip below already treats that as "contributes nothing".
    let srcs: Vec<Source<'_>> = layers
        .iter()
        .filter(|l| l.is_visible() && !l.is_transparent() && l.store.is_some())
        .map(|l| Source {
            store: l.store.expect("filtered to Some(store) above"),
            opacity: l.opacity,
            mode: l.mode,
        })
        .collect();

    for y in 0..h {
        for x in 0..w {
            let mut acc = base;
            for src in &srcs {
                let px = src.store.get_pixel(x as u32, y as u32);
                let sa8 = (px >> 24) as u8;
                if sa8 == 0 {
                    continue;
                }
                let [sr, sg, sb, _] = unpack_rgba(px);
                let op = src.opacity;
                let mode = src.mode;
                // Same normal-mode-only shortcut the display compositor uses.
                if mode == "normal" && sa8 == 255 && op == 1.0 {
                    acc = [sr, sg, sb, 255];
                    continue;
                }
                acc = over_blend(acc, [sr, sg, sb, sa8], op, mode);
            }
            let o = (y * w + x) * 4;
            out[o] = acc[0];
            out[o + 1] = acc[1];
            out[o + 2] = acc[2];
            out[o + 3] = acc[3];
            if acc[3] == 0 {
                out[o] = 0;
                out[o + 1] = 0;
                out[o + 2] = 0;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunkstore::PixelWriter;
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
        // no longer prune the chunk as empty.
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
