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
