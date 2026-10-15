//! `chunkstore` tests. The `core_basic` cases mirror `tests/core_basic.test.mjs`;
//! the writer ones cover the memoised dirty set (`docs/perf_js_vs_rust.md` §4-1).

use super::store::{Changeset, ChunkStore, is_all_zero};
use super::writer::PixelWriter;
use crate::constants::CHUNK_LEN;
use crate::pixel::{TRANSPARENT, pack_rgba};

fn store() -> ChunkStore {
    ChunkStore::new(64, 64).expect("valid size")
}

const RED: u32 = 0xff0000ff;
const GREEN: u32 = 0x00ff00ff;
/// Opaque green, `a<<24 | b<<16 | g<<8 | r`. `GREEN` above is the JS test
/// fixture's *transparent* green (alpha 0) and must not be used where a covered
/// pixel is meant.
const OPAQUE_GREEN: u32 = 0xff00ff00;
/// Opaque blue, `a<<24 | b<<16 | g<<8 | r`.
const BLUE: u32 = 0xffff0000;

// ---- the store ----

#[test]
fn sparse_storage_creates_no_empty_chunks() {
    let mut s = store();
    assert_eq!(s.chunk_count(), 0);
    assert_eq!(s.get_pixel(0, 0), TRANSPARENT);
    s.set_pixel(0, 0, pack_rgba(255.0, 0.0, 0.0, 255.0));
    assert_eq!(s.chunk_count(), 1);
    assert_eq!(s.get_pixel(0, 0), RED);
}

#[test]
fn out_of_canvas_is_ignored_not_raised() {
    let mut s = store();
    s.set_pixel(9999, 9999, 0xffffffff);
    assert_eq!(s.chunk_count(), 0);
    assert_eq!(s.get_pixel(64, 0), TRANSPARENT);
}

#[test]
fn setting_transparent_keeps_the_chunk() {
    let mut s = store();
    s.set_pixel(1, 1, RED);
    s.set_pixel(1, 1, TRANSPARENT);
    // JS parity: the chunk stays, only the pixel clears.
    assert_eq!(s.chunk_count(), 1);
    assert_eq!(s.get_pixel(1, 1), TRANSPARENT);
}

#[test]
fn put_chunk_range_guards() {
    let mut s = store();
    assert!(s.put_chunk(99, 0, Some(&[0u8; CHUNK_LEN])).is_err());
    assert!(s.put_chunk(0, 0, Some(&[0u8; 16])).is_err());
    assert!(s.put_chunk(0, 0, Some(&[0u8; CHUNK_LEN])).is_ok());
    assert!(s.put_chunk(0, 0, None).is_ok());
    assert_eq!(s.chunk_count(), 0);
}

#[test]
fn for_each_chunk_is_key_ascending() {
    let mut s = ChunkStore::new(128, 64).unwrap();
    s.set_pixel(96, 0, RED); // chunk (3,0)
    s.set_pixel(0, 32, RED); // chunk (0,1)
    s.set_pixel(0, 0, RED); // chunk (0,0)
    let order: Vec<(u32, u32)> = {
        let mut v = Vec::new();
        s.for_each_chunk(|cx, cy, _| v.push((cx, cy)));
        v
    };
    // JS parity: `forEachChunk` sorts the numeric key `cy*64+cx`, so the order
    // is key-ascending, NOT row-major. (0,1) is key 64, (3,0) is key 3.
    assert_eq!(order, vec![(0, 0), (3, 0), (0, 1)]);
}

#[test]
fn prune_empty_removes_only_zero_chunks() {
    let mut s = store();
    s.set_pixel(0, 0, RED);
    s.set_pixel(40, 40, RED);
    s.set_pixel(40, 40, TRANSPARENT); // chunk (1,1) still allocated
    assert_eq!(s.chunk_count(), 2);
    assert_eq!(s.prune_empty(), 1);
    assert_eq!(s.chunk_count(), 1);
}

#[test]
fn clone_deep_shares_no_buffers() {
    let mut s = store();
    s.set_pixel(0, 0, RED);
    let mut c = s.clone_deep();
    c.set_pixel(0, 0, GREEN);
    assert_eq!(s.get_pixel(0, 0), RED);
    assert_eq!(c.get_pixel(0, 0), GREEN);
}

#[test]
fn resize_returns_removed_chunks() {
    let mut s = ChunkStore::new(128, 128).unwrap();
    s.set_pixel(96, 96, RED);
    let removed = s.resize_to(64, 64).expect("valid size");
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].0, 3);
    assert_eq!(removed[0].1, 3);
    assert_eq!(s.width_px, 64);
    assert_eq!(s.chunk_count(), 0);
    assert!(s.resize_to(33, 64).is_err());
}

#[test]
fn invalid_canvas_size_raises_the_contract_code() {
    match ChunkStore::new(31, 32) {
        Err(e) => assert_eq!(e.code, crate::errors::CODE_CANVAS_SIZE_INVALID),
        Ok(_) => panic!("31x32 must be rejected"),
    }
}

#[test]
fn is_all_zero_detects_any_byte() {
    let mut c = Box::new([0u8; CHUNK_LEN]);
    assert!(is_all_zero(&c));
    c[CHUNK_LEN - 1] = 1;
    assert!(!is_all_zero(&c));
}

// ---- PixelWriter ----

#[test]
fn writer_emits_an_exact_diff() {
    let mut s = store();
    s.set_pixel(0, 0, 0x11223344);
    let mut w = PixelWriter::new(&mut s, "layer-0001");
    w.set(1, 1, RED);
    w.set(0, 0, GREEN);
    let dirty = w.take_dirty();
    assert_eq!(dirty, vec![(0, 0)]);
    let cs = w.finish().expect("changeset");
    assert_eq!(cs.layer_id, "layer-0001");
    assert_eq!(cs.chunks.len(), 1);
    let (cx, cy, before, after) = &cs.chunks[0];
    assert_eq!((*cx, *cy), (0, 0));
    assert_eq!(before.as_ref().unwrap()[0..4], [0x44, 0x33, 0x22, 0x11]);
    // 0x00ff00ff was the LAST write to (0,0), so `after` holds that value.
    assert_eq!(after.as_ref().unwrap()[0..4], [0xff, 0x00, 0xff, 0x00]);
}

#[test]
fn writer_returns_none_when_nothing_changed() {
    let mut s = store();
    s.set_pixel(0, 0, RED);
    let mut w = PixelWriter::new(&mut s, "layer-0001");
    w.set(0, 0, RED); // same value
    assert!(w.finish().is_none(), "an identical write is not a change");
}

#[test]
fn emptied_chunk_is_dropped_from_the_store() {
    let mut s = store();
    s.set_pixel(0, 0, RED);
    assert_eq!(s.chunk_count(), 1);
    let mut w = PixelWriter::new(&mut s, "layer-0001");
    w.set(0, 0, TRANSPARENT);
    let cs = w.finish().expect("changeset");
    assert_eq!(cs.chunks.len(), 1);
    assert!(cs.chunks[0].3.is_none(), "after must be None");
    assert_eq!(s.chunk_count(), 0, "the store must have dropped it");
}

#[test]
fn discard_restores_the_before_image() {
    let mut s = store();
    s.set_pixel(0, 0, RED);
    {
        let mut w = PixelWriter::new(&mut s, "layer-0001");
        w.set(0, 0, GREEN);
        w.set(5, 5, GREEN);
        let dirty = w.discard();
        assert_eq!(dirty, vec![(0, 0)]);
    }
    assert_eq!(s.get_pixel(0, 0), RED);
    assert_eq!(s.get_pixel(5, 5), TRANSPARENT);
}

#[test]
fn discard_of_a_new_chunk_removes_it_entirely() {
    // `before` records a placeholder for a chunk that did not exist; discarding
    // must put the store back to "no chunk", not "all-zero chunk".
    let mut s = store();
    {
        let mut w = PixelWriter::new(&mut s, "layer-0001");
        w.set(0, 0, RED);
        w.discard();
    }
    assert_eq!(s.chunk_count(), 0, "the chunk must be gone, not emptied");
    assert_eq!(s.get_pixel(0, 0), TRANSPARENT);
}

#[test]
fn out_of_canvas_write_is_not_dirty() {
    let mut s = store();
    let mut w = PixelWriter::new(&mut s, "layer-0001");
    w.set(500, 500, RED);
    assert!(w.take_dirty().is_empty());
    assert!(w.finish().is_none());
}

#[test]
fn byte_size_counts_both_sides() {
    let mut s = store();
    s.set_pixel(0, 0, RED);
    let mut w = PixelWriter::new(&mut s, "layer-0001");
    w.set(0, 0, GREEN);
    let cs = w.finish().unwrap();
    assert_eq!(cs.byte_size(), CHUNK_LEN * 2);
    assert_eq!(Changeset::default().byte_size(), 0);
}

// ---- the memoised dirty set (docs/perf_js_vs_rust.md §4-1) ----

#[test]
fn draining_mid_stroke_then_writing_again_re_reports_the_chunk() {
    // `take_dirty` empties the set, so the "last chunk" memo must be reset with
    // it. Otherwise the next write into the same chunk would see the memo hit
    let mut s = ChunkStore::new(128, 128).expect("valid size");
    let mut w = PixelWriter::new(&mut s, "layer-0001");
    w.set(0, 0, RED);
    w.set(5, 5, RED);
    assert_eq!(w.take_dirty(), vec![(0, 0)]);
    // A second pixel in the SAME chunk, after the drain.
    w.set(6, 6, RED);
    assert_eq!(w.take_dirty(), vec![(0, 0)], "the chunk must re-appear");
    // A different chunk, then back to the first one.
    w.set(96, 96, RED); // chunk (3,3)
    w.set(7, 7, RED); // back in chunk (0,0)
    assert_eq!(w.take_dirty(), vec![(0, 0), (3, 3)]);
    assert!(w.finish().is_some());
}

#[test]
fn a_wide_stroke_records_every_chunk_it_touched_in_order() {
    // A pen=64 stroke spans several chunks per axis, so the dirty set has to be
    // complete — not just the last chunk of a run.
    let mut s = ChunkStore::new(128, 128).expect("valid size");
    {
        let mut w = PixelWriter::new(&mut s, "layer-0001");
        crate::raster::brush::stroke_point(&mut w, 32, 32, 64, RED);
        let dirty = w.take_dirty();
        // The 64px footprint is a DISC, not a square (3,228 of 4,096 cells), so
        // it straddles the chunk grid rather than filling it. What matters is
        assert_eq!(dirty.len(), 8, "a disc this size straddles 8 chunks");
        assert_eq!(dirty[0], (0, 0));
        assert!(
            dirty
                .windows(2)
                .all(|p| (p[0].1, p[0].0) <= (p[1].1, p[1].0)),
            "(cy,cx) order must hold without a sort: {dirty:?}"
        );
    }
    // And key order really is (cy,cx) order, with no sort anywhere.
    let mut w2 = PixelWriter::new(&mut s, "layer-0001");
    w2.set(96, 0, GREEN); // chunk (3,0)
    w2.set(0, 32, GREEN); // chunk (0,1)
    w2.set(0, 0, GREEN); // chunk (0,0)
    w2.set(64, 32, GREEN); // chunk (2,1)
    assert_eq!(w2.take_dirty(), vec![(0, 0), (3, 0), (0, 1), (2, 1)]);
    let _ = w2.finish();
}

#[test]
fn many_writes_inside_one_chunk_record_it_once() {
    // The whole point of the memo: a footprint writes thousands of pixels into
    // one chunk, and each of those must not re-probe the bookkeeping.
    let mut s = ChunkStore::new(64, 64).expect("valid size");
    let mut w = PixelWriter::new(&mut s, "layer-0001");
    for i in 0..512u32 {
        w.set(i % 32, (i / 32) % 32, RED);
    }
    assert_eq!(w.take_dirty(), vec![(0, 0)], "512 writes, one chunk");
    w.discard();
}

#[test]
fn touched_reports_without_draining() {
    let mut s = store();
    let mut w = PixelWriter::new(&mut s, "layer-0001");
    w.set(0, 0, RED);
    assert_eq!(w.touched(), vec![(0, 0)]);
    // Not drained, so `take_dirty` still sees it.
    assert_eq!(w.take_dirty(), vec![(0, 0)]);
    w.finish();
}

/// The coverage map is a cache of "alpha != 0", so it must agree with the
/// bytes after every route into a chunk. This is the invariant the composite
/// and flatten fast paths lean on; without it they would silently skip pixels.
#[test]
fn the_alpha_mask_never_drifts_from_the_bytes() {
    let mut s = ChunkStore::new(128, 64).expect("valid size");
    let mut w = PixelWriter::new(&mut s, "layer-0001");
    // A spread of routes: single pixels, runs, a transparent clear, an
    // out-of-canvas write, a raw put and a chunk that straddles the 32 boundary.
    w.set(0, 0, RED);
    w.set(31, 31, OPAQUE_GREEN);
    w.set(32, 0, RED);
    w.set_run(0, 5, 10, &BLUE.to_le_bytes());
    w.set(0, 5, 0);
    w.set(200, 0, RED);
    let _ = w.finish();

    let mut raw = Box::new([0u8; crate::constants::CHUNK_LEN]);
    raw[0] = 0x11;
    raw[3] = 0x40; // alpha 40: covered
    raw[8] = 0x22; // alpha 0: not covered
    s.put_chunk(1, 1, Some(&raw[..])).expect("valid chunk");

    for cx in 0..4u32 {
        for cy in 0..2u32 {
            if let Some((data, mask)) = s.get_chunk_with_mask(cx, cy) {
                assert_eq!(
                    *mask,
                    crate::chunkstore::alpha::from_chunk(data),
                    "mask drifted in chunk ({cx},{cy})"
                );
            }
        }
    }
}

/// `set_run` must leave exactly the pixels the per-pixel loop would, including
/// when the run straddles a chunk seam.
#[test]
fn a_run_writes_the_same_bytes_as_the_pixel_loop() {
    let mut runs = ChunkStore::new(128, 32).expect("valid size");
    let mut pixels = ChunkStore::new(128, 32).expect("valid size");
    let cases: [(u32, usize, [u8; 4]); 4] = [
        (20, 3, [1, 2, 3, 4]),
        (63, 3, [5, 6, 7, 8]),
        (100, 2, [9, 10, 11, 255]),
        (0, 32, [12, 13, 14, 15]),
    ];
    for (x, len, c) in cases {
        let mut lo = x;
        let mut left = len;
        while left > 0 {
            let lx = lo & 31;
            let n = (left as u32).min(32 - lx) as usize;
            runs.set_run(lo >> 5, 0, lx, n, &c);
            lo += n as u32;
            left -= n;
        }
        for i in 0..len {
            pixels.set_pixel(
                x + i as u32,
                0,
                crate::pixel::pack_rgba_bytes(c[0], c[1], c[2], c[3]),
            );
        }
    }
    for cx in 0..4u32 {
        assert_eq!(runs.get_chunk(cx, 0), pixels.get_chunk(cx, 0), "chunk {cx}");
    }
}
