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

#[derive(Clone, Debug)]
pub struct StrokeState {
    pub(crate) layer_id: String,
    pub(crate) before: BTreeMap<u64, Option<Box<Chunk>>>,
    pub(crate) dirty: BTreeSet<u64>,
    pub(crate) last_key: Option<u64>,
}

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
    pub fn resume(store: &'a mut crate::chunkstore::store::ChunkStore, state: StrokeState) -> Self {
        Self {
            store,
            layer_id: state.layer_id.clone(),
            before: state.before,
            dirty: state.dirty,
            last_key: state.last_key,
            done: false,
        }
    }

    pub fn suspend(self) -> StrokeState {
        StrokeState {
            layer_id: self.layer_id.clone(),
            before: self.before,
            dirty: self.dirty,
            last_key: self.last_key,
        }
    }
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
        self.touch_key(chunk_key(x >> 5, y >> 5));
    }

    /// The ledger update for one chunk, memoised on the chunk the previous write
    /// landed in.
    ///
    /// `set_run` knows its chunk key directly, so it skips the coordinate
    /// decomposition `touch` does — the same memo either way.
    fn touch_key(&mut self, key: u64) {
        if self.last_key == Some(key) {
            return;
        }
        self.before.entry(key).or_insert_with(|| {
            let (cx, cy) = chunk_coords(key);
            // `copy_chunk` returns `None` for an absent chunk, and that `None`
            // *is* the before-image: it lets both `finish` and `discard` put a
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

    /// Write a horizontal run of pixels with the same colour.
    ///
    /// This is the span write the whole engine now uses: a brush row, a
    /// flood-fill span, a scanline polygon edge. It replaces `len` calls to
    /// [`Self::set`] — `len` chunk probes, `len` ledger updates and `len`
    /// 4-byte writes — with one of each per 32-pixel chunk.
    ///
    /// Byte-for-byte equivalent to the `set` loop, including the rule that a
    /// transparent run never brings a chunk into existence and the rule that a
    /// run starting off-canvas is **trimmed, not dropped**: `set` clips each
    /// pixel, so a footprint centred on the left edge still paints the columns
    /// that land inside.
    pub fn set_run(&mut self, x: i32, y: i32, len: usize, color: &[u8; 4]) {
        if self.done {
            panic!("PixelWriter is finished"); // INVALID_STATE in the JS tree
        }
        if len == 0 || y < 0 {
            return;
        }
        let (mut x, mut len) = (x, len);
        if x < 0 {
            len = len.saturating_sub(x.unsigned_abs() as usize);
            x = 0;
        }
        if len == 0 {
            return;
        }
        let (x, y) = (x as u32, y as u32);
        if x >= self.store.width_px || y >= self.store.height_px {
            return;
        }
        let mut left = len;
        let mut cx = x;
        while left > 0 {
            let lx = cx & 31;
            let n = (left as u32).min(32 - lx) as usize;
            self.touch_key(chunk_key(cx >> 5, y >> 5));
            self.store.set_run(cx >> 5, y, lx, n, color);
            cx += n as u32;
            left -= n;
        }
    }

    /// The packed pixel at `(x, y)`, 0 outside the canvas.
    #[inline]
    pub fn pixel_at(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 {
            return 0;
        }
        self.store.get_pixel(x as u32, y as u32)
    }

    /// Append the runs of `seed` on row `y` inside `[lo, hi)`, left to right.
    ///
    /// **Why this is chunk-stepped and not a `get` loop.** The scanline fill
    /// calls this once per neighbour row per span, and a naive version pays a
    /// `BTreeMap` probe *per pixel* — 2048 probes for one canvas row, which was
    /// the measured cost of the old fill. Here the row is walked one chunk at a
    /// time: one probe buys 32 pixel comparisons out of a 4 KB buffer that is
    /// already hot in L1.
    ///
    /// An absent chunk reads back as 32 transparent pixels, which is what
    /// `get_pixel` returns there; the run continues across the gap only when
    /// `seed` is 0, exactly as the per-pixel walk would.
    pub fn row_seed_runs(&self, y: i32, lo: i32, hi: i32, seed: u32, out: &mut Vec<(i32, i32)>) {
        if y < 0 || y >= self.store.height_px as i32 || lo >= hi {
            return;
        }
        let y_u = y as u32;
        let hi = hi.min(self.store.width_px as i32);
        let mut x = lo.max(0);
        // `start` is the first pixel of the run currently being built, if any.
        let mut start: Option<i32> = None;
        while x < hi {
            let chunk_row_stop = (((x as u32 >> 5) + 1) << 5) as i32;
            let stop = hi.min(chunk_row_stop);
            match self.store.get_chunk_row(x as u32 >> 5, y_u, x as u32 & 31) {
                Some(row) => {
                    let mut i = x;
                    for px in row.as_chunks::<4>().0.iter() {
                        let hit = u32::from_le_bytes([px[0], px[1], px[2], px[3]]) == seed;
                        if hit {
                            start.get_or_insert(i);
                        } else if let Some(s) = start.take() {
                            out.push((s, i));
                        }
                        i += 1;
                        if i >= stop {
                            break;
                        }
                    }
                }
                // Absent chunk: 32 transparent pixels, decided without reading.
                None => {
                    if seed == 0 {
                        start.get_or_insert(x);
                    } else if let Some(s) = start.take() {
                        out.push((s, x));
                    }
                }
            }
            x = stop;
        }
        if let Some(s) = start {
            out.push((s, hi));
        }
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
            let bytes: Option<&[u8]> = before.as_deref().map(|b| &b[..]);
            // Ignoring the error is safe: the coords came from a live key.
            let _ = self.store.put_chunk(cx, cy, bytes);
        }
        self.take_dirty()
    }
}
