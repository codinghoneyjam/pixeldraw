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
