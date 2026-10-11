//! Sparse chunk storage + the stroke change tracker.
//!
//! Ported 1:1 from `src/core/chunkstore.js`. A layer is a `Map` of 32×32
//! chunks holding `[u8; CHUNK_LEN]` each; empty chunks are never created, so a
//! mostly-empty 2048×1088 layer costs almost nothing.
//!
//! **Mutation rule** (docs/contract.md §2): nobody touches a chunk outside
//! [`PixelWriter`] or a Command's `do`/`undo`. Out-of-canvas `set` is silently
//! ignored, which is why callers never clip.

use std::collections::{BTreeMap, BTreeSet};

use crate::constants::{CHUNK_LEN, chunk_coords, chunk_key};
use crate::errors::{DrawToolError, EngineResult};

/// `true` when every byte is zero — the emptiness test for pruning.
///
/// A byte scan rather than the JS side's `Uint32Array` view: LLVM vectorises it
/// into the same word-at-a-time loop, with no `unsafe` (the workspace forbids it).
pub fn is_all_zero(data: &[u8; CHUNK_LEN]) -> bool {
    data.iter().all(|&b| b == 0)
}

/// One 32×32 RGBA chunk, packed `r,g,b,a` little-endian like the JS view.
pub type Chunk = [u8; CHUNK_LEN];

/// The sparse layer bitmap.
///
/// A `BTreeMap` keeps keys ordered, so `for_each_chunk` and serialization stay
/// deterministic without the JS side's explicit `sort` (contract §2 rule 8).
#[derive(Debug, Clone)]
pub struct ChunkStore {
    pub width_px: u32,
    pub height_px: u32,
    pub chunks_x: u32,
    pub chunks_y: u32,
    chunks: BTreeMap<u64, Box<Chunk>>,
}

impl ChunkStore {
    /// Validates the canvas size the way the JS constructor did.
    pub fn new(width_px: u32, height_px: u32) -> EngineResult<Self> {
        crate::constants::require_valid_canvas_size(width_px, height_px)?;
        Ok(Self {
            width_px,
            height_px,
            chunks_x: width_px / crate::constants::CHUNK_PX,
            chunks_y: height_px / crate::constants::CHUNK_PX,
            chunks: BTreeMap::new(),
        })
    }

    /// Byte offset of a pixel inside its chunk.
    #[inline]
    fn offset(x: u32, y: u32) -> usize {
        let lx = (x & (crate::constants::CHUNK_PX - 1)) as usize;
        let ly = (y & (crate::constants::CHUNK_PX - 1)) as usize;
        (ly * crate::constants::CHUNK_PX as usize + lx) * 4
    }

    #[inline]
    fn in_canvas(&self, x: u32, y: u32) -> bool {
        x < self.width_px && y < self.height_px
    }

    /// Packed pixel; 0 when outside the canvas or the chunk is absent.
    #[inline]
    pub fn get_pixel(&self, x: u32, y: u32) -> u32 {
        if !self.in_canvas(x, y) {
            return 0;
        }
        let Some(data) = self.chunks.get(&chunk_key(x >> 5, y >> 5)) else {
            return 0;
        };
        let o = Self::offset(x, y);
        u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]])
    }

    /// Command/`PixelWriter`-only write. Out of canvas is ignored; `packed == 0`
    /// clears the pixel but leaves the chunk in place (JS parity).
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, packed: u32) {
        if !self.in_canvas(x, y) {
            return;
        }
        let key = chunk_key(x >> 5, y >> 5);
        if packed == 0 {
            if let Some(data) = self.chunks.get_mut(&key) {
                let o = Self::offset(x, y);
                data[o..o + 4].fill(0);
            }
            return;
        }
        let data = self
            .chunks
            .entry(key)
            .or_insert_with(|| Box::new([0u8; CHUNK_LEN]));
        let o = Self::offset(x, y);
        data[o..o + 4].copy_from_slice(&packed.to_le_bytes());
    }

    /// The stored buffer, or `None`.
    #[inline]
    pub fn get_chunk(&self, cx: u32, cy: u32) -> Option<&Chunk> {
        self.chunks.get(&chunk_key(cx, cy)).map(|b| &**b)
    }

    /// A copy, for snapshots and undo material.
    pub fn copy_chunk(&self, cx: u32, cy: u32) -> Option<Box<Chunk>> {
        self.chunks.get(&chunk_key(cx, cy)).cloned()
    }

    /// `None` deletes, out-of-range or wrong length raises `OUT_OF_RANGE`.
    /// The input is defensively copied, as the JS `data.slice()` was.
    pub fn put_chunk(&mut self, cx: u32, cy: u32, data: Option<&[u8]>) -> EngineResult<()> {
        if cx >= self.chunks_x || cy >= self.chunks_y {
            return Err(DrawToolError::chunk_out_of_canvas(cx, cy));
        }
        match data {
            None => {
                self.chunks.remove(&chunk_key(cx, cy));
                Ok(())
            }
            Some(bytes) => {
                if bytes.len() != CHUNK_LEN {
                    return Err(DrawToolError::out_of_range(
                        "chunk data must be a 4096-byte array",
                    ));
                }
                let mut chunk = Box::new([0u8; CHUNK_LEN]);
                chunk.copy_from_slice(bytes);
                self.chunks.insert(chunk_key(cx, cy), chunk);
                Ok(())
            }
        }
    }

    /// Key-ascending walk, so serialization is deterministic.
    pub fn for_each_chunk(&self, mut f: impl FnMut(u32, u32, &Chunk)) {
        for (&key, data) in &self.chunks {
            let (cx, cy) = chunk_coords(key);
            f(cx, cy, data);
        }
    }

    /// Number of stored chunks; empty ones are not counted.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Delete all-zero chunks, returning how many went.
    pub fn prune_empty(&mut self) -> usize {
        let dead: Vec<u64> = self
            .chunks
            .iter()
            .filter(|(_, d)| is_all_zero(d))
            .map(|(k, _)| *k)
            .collect();
        for k in &dead {
            self.chunks.remove(k);
        }
        dead.len()
    }

    /// Deep clone sharing no buffers.
    pub fn clone_deep(&self) -> Self {
        Self {
            width_px: self.width_px,
            height_px: self.height_px,
            chunks_x: self.chunks_x,
            chunks_y: self.chunks_y,
            chunks: self.chunks.clone(),
        }
    }

    /// Resize, returning the chunks that fell off the edge (redo material).
    pub fn resize_to(
        &mut self,
        new_w: u32,
        new_h: u32,
    ) -> EngineResult<Vec<(u32, u32, Box<Chunk>)>> {
        crate::constants::require_valid_canvas_size(new_w, new_h)?;
        let nx = new_w / crate::constants::CHUNK_PX;
        let ny = new_h / crate::constants::CHUNK_PX;
        let mut removed = Vec::new();
        let keys: Vec<u64> = self.chunks.keys().copied().collect();
        for key in keys {
            let (cx, cy) = chunk_coords(key);
            let outside = cx >= nx || cy >= ny;
            if outside && let Some(data) = self.chunks.remove(&key) {
                removed.push((cx, cy, data));
            }
        }
        self.width_px = new_w;
        self.height_px = new_h;
        self.chunks_x = nx;
        self.chunks_y = ny;
        Ok(removed)
    }
}

/// One chunk's before/after pair inside a changeset.
pub type ChunkDiff = (u32, u32, Option<Box<Chunk>>, Option<Box<Chunk>>);

/// The change shape both `finish` and the undo budget speak in.
#[derive(Debug, Clone, Default)]
pub struct Changeset {
    pub layer_id: String,
    pub chunks: Vec<ChunkDiff>,
}

impl Changeset {
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Total bytes of `before` + `after`, the Undo budget's currency.
    pub fn byte_size(&self) -> usize {
        self.chunks
            .iter()
            .map(|(_, _, b, a)| {
                b.as_ref().map_or(0, |_| CHUNK_LEN) + a.as_ref().map_or(0, |_| CHUNK_LEN)
            })
            .sum()
    }
}

/// One stroke's before-snapshot and dirty set.
///
/// Every `set` through here records the chunk's pre-image the first time it is
/// touched, so `finish` can emit an exact diff and `discard` can restore. The
/// writer must be closed by exactly one of [`PixelWriter::finish`] or
/// [`PixelWriter::discard`].
#[derive(Debug)]
pub struct PixelWriter<'a> {
    store: &'a mut ChunkStore,
    pub layer_id: String,
    before: BTreeMap<u64, Option<Box<Chunk>>>,
    /// Keys of the chunks this writer has touched, drained by `take_dirty`.
    dirty: BTreeSet<u64>,
    /// Chunk the previous `set` landed in, so a whole footprint touches the
    /// bookkeeping once rather than once per pixel.
    last_key: Option<u64>,
    done: bool,
}

impl<'a> PixelWriter<'a> {
    pub fn new(store: &'a mut ChunkStore, layer_id: impl Into<String>) -> Self {
        Self {
            store,
            layer_id: layer_id.into(),
            before: BTreeMap::new(),
            dirty: BTreeSet::new(),
            last_key: None,
            done: false,
        }
    }

    fn touch(&mut self, x: u32, y: u32) {
        let key = chunk_key(x >> 5, y >> 5);
        if self.last_key == Some(key) {
            return;
        }
        self.before.entry(key).or_insert_with(|| {
            let (cx, cy) = chunk_coords(key);
            self.store.copy_chunk(cx, cy)
        });
        self.dirty.insert(key);
        self.last_key = Some(key);
    }

    /// Bounds-checked write; out of canvas is ignored.
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, packed: u32) {
        if self.done {
            panic!("PixelWriter is finished"); // INVALID_STATE in the JS tree
        }
        if x >= self.store.width_px || y >= self.store.height_px {
            return;
        }
        self.touch(x, y);
        self.store.set_pixel(x, y, packed);
    }

    /// Read-through of the current value.
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> u32 {
        self.store.get_pixel(x, y)
    }

    /// `(cy, cx)`-ordered dirty list; empties the internal set so a mid-stroke
    /// render loop can drain it repeatedly.
    ///
    /// A `BTreeSet<u64>` of chunk keys is used because `chunk_key = cy*64+cx`,
    /// so key order **is** `(cy, cx)` order — the sort the old `Vec` needed on
    /// every drain is gone. The memo is reset too: otherwise a write to the
    /// chunk the previous pixel landed in would skip re-inserting it after the
    /// set was just emptied, and the drained chunk would go missing.
    pub fn take_dirty(&mut self) -> Vec<(u32, u32)> {
        let keys = std::mem::take(&mut self.dirty);
        self.last_key = None;
        keys.into_iter().map(chunk_coords).collect()
    }

    /// Consume the stroke into a changeset, or `None` when nothing changed.
    ///
    /// A chunk that ended up all-zero is dropped from the store and recorded with
    /// `after = None`, so an erased region leaves no trace in the file.
    pub fn finish(mut self) -> Option<Changeset> {
        self.done = true;
        let mut chunks = Vec::new();
        for (&key, before) in &self.before {
            let (cx, cy) = chunk_coords(key);
            let mut after = self.store.copy_chunk(cx, cy);
            if let Some(a) = &after
                && is_all_zero(a)
            {
                // Safe: the key came from a live before-image.
                let _ = self.store.put_chunk(cx, cy, None);
                after = None;
            }
            if chunk_eq(as_bytes(before.as_deref()), as_bytes(after.as_deref())) {
                continue;
            }
            chunks.push((cx, cy, before.clone(), after));
        }
        if chunks.is_empty() {
            return None;
        }
        Some(Changeset {
            layer_id: self.layer_id.clone(),
            chunks,
        })
    }

    /// Restore every before-image and return the dirty list (cancel path).
    pub fn discard(mut self) -> Vec<(u32, u32)> {
        self.done = true;
        for (&key, before) in &self.before {
            let (cx, cy) = chunk_coords(key);
            let bytes: Option<&[u8]> = before.as_deref().map(|b| &b[..]);
            // Ignoring the error is safe: the coords came from a live key.
            let _ = self.store.put_chunk(cx, cy, bytes);
        }
        self.take_dirty()
    }
}

/// `Option<&Chunk>` → `Option<&[u8]>`: `Box<[u8; N]>::as_deref` keeps the array
/// type and will not coerce to a slice on its own.
fn as_bytes(c: Option<&Chunk>) -> Option<&[u8]> {
    c.map(|b| &b[..])
}

/// Byte comparison of two optional chunks.
fn chunk_eq(a: Option<&[u8]>, b: Option<&[u8]>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pixel::{TRANSPARENT, pack_rgba};

    fn store() -> ChunkStore {
        ChunkStore::new(64, 64).expect("valid size")
    }

    // <META - ROLE : parity with tests/core_basic.test.mjs chunkstore cases | L280-330>
    #[test]
    fn sparse_storage_creates_no_empty_chunks() {
        let mut s = store();
        assert_eq!(s.chunk_count(), 0);
        assert_eq!(s.get_pixel(0, 0), TRANSPARENT);
        s.set_pixel(0, 0, pack_rgba(255.0, 0.0, 0.0, 255.0));
        assert_eq!(s.chunk_count(), 1);
        assert_eq!(s.get_pixel(0, 0), 0xff0000ff);
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
        s.set_pixel(1, 1, 0xff0000ff);
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
        s.set_pixel(96, 0, 0xff0000ff); // chunk (3,0)
        s.set_pixel(0, 32, 0xff0000ff); // chunk (0,1)
        s.set_pixel(0, 0, 0xff0000ff); // chunk (0,0)
        let order: Vec<(u32, u32)> = {
            let mut v = Vec::new();
            s.for_each_chunk(|cx, cy, _| v.push((cx, cy)));
            v
        };
        // JS parity: `forEachChunk` sorts the numeric key `cy*64+cx`, so the
        // order is key-ascending, NOT row-major. (0,1) is key 64, (3,0) is key 3.
        assert_eq!(order, vec![(0, 0), (3, 0), (0, 1)]);
    }

    #[test]
    fn prune_empty_removes_only_zero_chunks() {
        let mut s = store();
        s.set_pixel(0, 0, 0xff0000ff);
        s.set_pixel(40, 40, 0xff0000ff);
        s.set_pixel(40, 40, TRANSPARENT); // chunk (1,1) still allocated
        assert_eq!(s.chunk_count(), 2);
        assert_eq!(s.prune_empty(), 1);
        assert_eq!(s.chunk_count(), 1);
    }

    #[test]
    fn clone_deep_shares_no_buffers() {
        let mut s = store();
        s.set_pixel(0, 0, 0xff0000ff);
        let mut c = s.clone_deep();
        c.set_pixel(0, 0, 0x00ff00ff);
        assert_eq!(s.get_pixel(0, 0), 0xff0000ff);
        assert_eq!(c.get_pixel(0, 0), 0x00ff00ff);
    }

    #[test]
    fn resize_returns_removed_chunks() {
        let mut s = ChunkStore::new(128, 128).unwrap();
        s.set_pixel(96, 96, 0xff0000ff);
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

    // ---- PixelWriter ----

    #[test]
    fn writer_emits_an_exact_diff() {
        let mut s = store();
        s.set_pixel(0, 0, 0x11223344);
        let mut w = PixelWriter::new(&mut s, "layer-0001");
        w.set(1, 1, 0xff0000ff);
        w.set(0, 0, 0x00ff00ff);
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
        s.set_pixel(0, 0, 0xff0000ff);
        let mut w = PixelWriter::new(&mut s, "layer-0001");
        w.set(0, 0, 0xff0000ff); // same value
        assert!(w.finish().is_none(), "an identical write is not a change");
    }

    #[test]
    fn emptied_chunk_is_dropped_from_the_store() {
        let mut s = store();
        s.set_pixel(0, 0, 0xff0000ff);
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
        s.set_pixel(0, 0, 0xff0000ff);
        {
            let mut w = PixelWriter::new(&mut s, "layer-0001");
            w.set(0, 0, 0x00ff00ff);
            w.set(5, 5, 0x00ff00ff);
            let dirty = w.discard();
            assert_eq!(dirty, vec![(0, 0)]);
        }
        assert_eq!(s.get_pixel(0, 0), 0xff0000ff);
        assert_eq!(s.get_pixel(5, 5), TRANSPARENT);
    }

    #[test]
    fn out_of_canvas_write_is_not_dirty() {
        let mut s = store();
        let mut w = PixelWriter::new(&mut s, "layer-0001");
        w.set(500, 500, 0xff0000ff);
        assert!(w.take_dirty().is_empty());
        assert!(w.finish().is_none());
    }

    #[test]
    fn byte_size_counts_both_sides() {
        let mut s = store();
        s.set_pixel(0, 0, 0xff0000ff);
        let mut w = PixelWriter::new(&mut s, "layer-0001");
        w.set(0, 0, 0x00ff00ff);
        let cs = w.finish().unwrap();
        assert_eq!(cs.byte_size(), CHUNK_LEN * 2);
        assert_eq!(Changeset::default().byte_size(), 0);
    }

    // ---- the memoised dirty set (docs/perf_js_vs_rust.md §4-1) ----

    #[test]
    fn draining_mid_stroke_then_writing_again_re_reports_the_chunk() {
        // `take_dirty` empties the set, so the "last chunk" memo must be reset
        // with it. Otherwise the next write into the same chunk would see the
        // memo hit and skip the insert, and the render loop would miss it.
        let mut s = ChunkStore::new(128, 128).expect("valid size");
        let mut w = PixelWriter::new(&mut s, "layer-0001");
        w.set(0, 0, 0xff0000ff);
        w.set(5, 5, 0xff0000ff);
        assert_eq!(w.take_dirty(), vec![(0, 0)]);
        // Second pixel in the SAME chunk, after the drain.
        w.set(6, 6, 0xff0000ff);
        assert_eq!(w.take_dirty(), vec![(0, 0)], "the chunk must re-appear");
        // A different chunk, then back to the first one.
        w.set(96, 96, 0xff0000ff); // chunk (3,3)
        w.set(7, 7, 0xff0000ff); // back in chunk (0,0)
        assert_eq!(w.take_dirty(), vec![(0, 0), (3, 3)]);
        assert!(w.finish().is_some());
    }

    #[test]
    fn a_wide_stroke_records_every_chunk_it_touched_in_order() {
        // A pen=64 stroke spans several chunks per axis, so the dirty set has to
        // be complete — not just the last chunk of a run.
        let mut s = ChunkStore::new(128, 128).expect("valid size");
        {
            let mut w = PixelWriter::new(&mut s, "layer-0001");
            crate::raster::brush::stroke_point(&mut w, 32, 32, 64, 0xff0000ff);
            let dirty = w.take_dirty();
            // The 64px footprint is a DISC, not a square (3,228 of 4,096 cells),
            // so it straddles the chunk grid rather than filling it. Eight chunks
            // fall inside the disc around (32,32); what matters here is that the
            // set is complete and in (cy,cx) order, not that a corner was hit.
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
        w2.set(96, 0, 0x00ff00ff); // chunk (3,0)
        w2.set(0, 32, 0x00ff00ff); // chunk (0,1)
        w2.set(0, 0, 0x00ff00ff); // chunk (0,0)
        w2.set(64, 32, 0x00ff00ff); // chunk (2,1)
        assert_eq!(w2.take_dirty(), vec![(0, 0), (3, 0), (0, 1), (2, 1)]);
        let _ = w2.finish();
    }

    #[test]
    fn many_writes_inside_one_chunk_record_it_once() {
        // The whole point of the memo: a footprint writes thousands of pixels
        // into one chunk, and each of those must not re-probe the bookkeeping.
        let mut s = ChunkStore::new(64, 64).expect("valid size");
        let mut w = PixelWriter::new(&mut s, "layer-0001");
        for i in 0..512u32 {
            w.set(i % 32, (i / 32) % 32, 0xff0000ff);
        }
        assert_eq!(w.take_dirty(), vec![(0, 0)], "512 writes, one chunk");
        w.discard();
    }

    #[test]
    fn is_all_zero_detects_any_byte() {
        let mut c = Box::new([0u8; CHUNK_LEN]);
        assert!(is_all_zero(&c));
        c[CHUNK_LEN - 1] = 1;
        assert!(!is_all_zero(&c));
    }
}
