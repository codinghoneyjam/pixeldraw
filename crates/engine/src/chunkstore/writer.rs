//! `PixelWriter` — one stroke's before-snapshot and dirty set.
//!
//! Every `set` through here records the chunk's pre-image the first time it is
//! touched, so `finish` can emit an exact diff and `discard` can restore. The
//! writer must be closed by exactly one of [`PixelWriter::finish`] or
//! [`PixelWriter::discard`].
//!
//! **Why the bookkeeping looks like this.** `docs/perf_js_vs_rust.md` §4-1
//! measured the first version at **185 ns per pixel write**, 25× the cost of the
//! store write it guards. Two things caused it:
//!
//! * `dirty` was a `Vec` probed with `contains`, so recording a chunk was a
//!   linear scan. A pen=64 stroke touches thousands of chunks, making the whole
//!   stroke quadratic in chunk count.
//! * `before` was probed through a `BTreeMap` entry on *every* pixel, although
//!   a pen footprint writes ~3,000 consecutive pixels into the same chunk.
//!
//! So `dirty` is a key set and `last_key` memoises the chunk the previous pixel
//! landed in. A pixel inside a chunk already recorded costs one integer
//! comparison instead of a tree probe plus a vector scan. Measured effect: §2-1
//! of that document, where every write-path item went from 6–9× slower than JS
//! to 1.3–4× faster.

use std::collections::{BTreeMap, BTreeSet};

use crate::chunkstore::store::{Changeset, Chunk, as_bytes, chunk_eq, is_all_zero};
use crate::constants::chunk_coords;
use crate::constants::chunk_key;

#[derive(Debug)]
pub struct PixelWriter<'a> {
    store: &'a mut crate::chunkstore::store::ChunkStore,
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
    pub fn new(
        store: &'a mut crate::chunkstore::store::ChunkStore,
        layer_id: impl Into<String>,
    ) -> Self {
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
            // `copy_chunk` returns `None` for an absent chunk, and that `None`
            // *is* the before-image: it lets both `finish` and `discard` put a
            // brand-new chunk back to *gone* rather than to all-zero.
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

    /// The chunks this writer has touched so far, in `(cy, cx)` order.
    ///
    /// Unlike `take_dirty` this does **not** drain: it lets a caller inspect the
    /// touched set without breaking the memo.
    pub fn touched(&self) -> Vec<(u32, u32)> {
        self.dirty.iter().copied().map(chunk_coords).collect()
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
            // `before` is `None` exactly when the chunk did not exist, so this
            // comparison is "did the chunk's existence or bytes change".
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
            // `before` is `None` when the chunk did not exist before the stroke,
            // so restoring passes `None` and the chunk is removed rather than
            // left behind as all-zero bytes.
            let bytes: Option<&[u8]> = before.as_deref().map(|b| &b[..]);
            // Ignoring the error is safe: the coords came from a live key.
            let _ = self.store.put_chunk(cx, cy, bytes);
        }
        self.take_dirty()
    }
}
