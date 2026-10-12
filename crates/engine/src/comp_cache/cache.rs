//! The cache itself — the composite buffer, the dirty set and the two flags.
//!
//! Split out of the facade so the JS field-for-field translation
//! (`src/render/renderer.js` `_comp`/`_dirty`/`_dirtyAll`) stays readable in one
//! place; the painting and the event rules live beside it.

use std::collections::BTreeSet;

use crate::constants::{CHUNK_PX, chunk_key};

/// The composited canvas plus the set of chunks that still owe a repaint.
///
/// JS `CanvasRenderer` held these as fields (`_comp`, `_dirty`, `_dirtyAll`); the
/// `_compW`/`_compH` pair collapsed into [`CompCache::width`]·[`CompCache::height`]
/// because the buffer is a `Vec<u8>` here rather than a canvas element.
#[derive(Debug, Clone)]
pub struct CompCache {
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// `width * height * 4` straight-alpha RGBA. A chunk nobody has painted yet
    /// reads transparent, so a half-painted frame is never garbage.
    pub(crate) comp: Vec<u8>,
    /// JS `_dirty`: the chunk keys awaiting a repaint.
    ///
    /// A `BTreeSet` here versus a JS `Set`: iteration is `(cy, cx)` order rather
    /// than insertion order, so under a cut-off budget a *different* chunk is
    /// left for the next frame — never a different pixel (docs/render.md).
    pub(crate) dirty: BTreeSet<u64>,
    /// JS `_dirtyAll` — "everything", including chunks not enumerated yet. It is
    /// expanded into `dirty` by [`CompCache::expand_dirty_all`] (`_update`), so
    /// the flag also works before a document has ever been seen.
    pub(crate) dirty_all: bool,
    /// JS `_setDocument` cut a **new** offscreen canvas every time a document
    /// arrived, even when the size was unchanged; the next `ensure` must do the
    /// same instead of no-op'ing.
    pub(crate) reensure: bool,
    /// Chunks the most recent [`CompCache::paint_dirty`] actually painted.
    /// Statistics for tests and the HUD; nothing in the cache reads it.
    pub(crate) last_painted: usize,
}

impl CompCache {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            comp: Vec::new(),
            dirty: BTreeSet::new(),
            // No buffer yet, so nothing can count as composited. The JS reached
            // the same state through `_setDocument` on attach.
            dirty_all: true,
            reensure: false,
            last_painted: 0,
        }
    }

    /// `ensureComp` — re-cut the buffer when the canvas size changes.
    ///
    /// A fresh buffer is transparent and every chunk of it is dirty, so a resize
    /// costs one full repaint, exactly as the JS `c.width = w` reassignment did.
    /// Same size and no pending `DocumentReplaced` is a no-op.
    pub fn ensure(&mut self, canvas_w: u32, canvas_h: u32) {
        let len = comp_len(canvas_w, canvas_h);
        let unchanged = self.width == canvas_w
            && self.height == canvas_h
            && self.comp.len() == len
            && !self.reensure;
        if unchanged {
            return;
        }
        self.width = canvas_w;
        self.height = canvas_h;
        self.comp = vec![0u8; len];
        self.dirty.clear();
        self.dirty_all = true;
        self.reensure = false;
    }

    /// `invalidateAll` — the flag only; the expansion happens in `update`, so the
    /// call is legal for a cache that has not seen a document yet.
    pub fn invalidate_all(&mut self) {
        self.dirty_all = true;
    }

    /// `invalidateChunks(chunks)`.
    ///
    /// The JS integer/non-negative guards are the `u32` type here. Out-of-canvas
    /// chunks are **skipped**, which the JS did not do (it painted them
    /// harmlessly): a chunk column past the edge has nowhere to blit to, and a
    /// size whose width is not a multiple of `CHUNK_PX` would run off the buffer.
    pub fn invalidate_chunks(&mut self, chunks: &[(u32, u32)]) {
        for &(cx, cy) in chunks {
            if !self.contains_chunk(cx, cy) {
                continue;
            }
            self.dirty.insert(chunk_key(cx, cy));
        }
    }

    pub fn comp(&self) -> &[u8] {
        &self.comp
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Chunks that still owe a repaint, with the un-expanded `dirty_all` counted
    /// as the whole grid — the number `_update` would repaint if it ran now.
    pub fn dirty_len(&self) -> usize {
        if self.dirty_all {
            self.chunk_count()
        } else {
            self.dirty.len()
        }
    }

    /// `last_painted` — how many chunks the last `paint_dirty` composited.
    pub fn last_painted(&self) -> usize {
        self.last_painted
    }

    /// `for cy in 0..h/CHUNK_PX, for cx in 0..w/CHUNK_PX` — the JS expansion loop.
    pub(crate) fn expand_dirty_all(&mut self) {
        if !self.dirty_all {
            return;
        }
        self.dirty.clear();
        for cy in 0..self.chunks_y() {
            for cx in 0..self.chunks_x() {
                self.dirty.insert(chunk_key(cx, cy));
            }
        }
        self.dirty_all = false;
    }

    /// `DocumentReplaced` arrived: the next `ensure` re-cuts even at equal size.
    pub(crate) fn request_reensure(&mut self) {
        self.reensure = true;
    }

    pub(crate) fn chunks_x(&self) -> u32 {
        self.width / CHUNK_PX
    }

    pub(crate) fn chunks_y(&self) -> u32 {
        self.height / CHUNK_PX
    }

    pub(crate) fn chunk_count(&self) -> usize {
        (self.chunks_x() * self.chunks_y()) as usize
    }

    /// Whether `(cx, cy)` names a chunk the canvas actually covers.
    pub(crate) fn contains_chunk(&self, cx: u32, cy: u32) -> bool {
        cx < self.chunks_x() && cy < self.chunks_y()
    }
}

impl Default for CompCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Row bytes of one chunk inside `comp` — `CHUNK_PX * 4`.
pub(crate) const CHUNK_ROW: usize = CHUNK_PX as usize * 4;

/// Byte length of the composite buffer for a canvas size.
pub(crate) fn comp_len(width: u32, height: u32) -> usize {
    (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_reallocates_and_marks_everything_dirty() {
        let mut c = CompCache::new();
        c.ensure(64, 64);
        assert_eq!(c.size(), (64, 64));
        assert_eq!(c.comp().len(), comp_len(64, 64));
        assert_eq!(c.dirty_len(), 4);
        c.expand_dirty_all();
        assert_eq!(c.dirty_len(), 4);
        // Same size again is a no-op: the painted pixels survive.
        c.ensure(64, 64);
        assert_eq!(c.dirty_len(), 4);
        c.dirty.clear();
        assert_eq!(c.dirty_len(), 0);
        c.ensure(64, 64);
        assert_eq!(c.dirty_len(), 0);
    }

    #[test]
    fn ensure_after_a_document_replaced_recuts_the_buffer() {
        let mut c = CompCache::new();
        c.ensure(64, 64);
        c.expand_dirty_all();
        c.request_reensure();
        c.ensure(64, 64);
        assert!(c.dirty_all, "a new document repaints from transparent");
    }

    #[test]
    fn out_of_canvas_chunks_are_ignored() {
        let mut c = CompCache::new();
        c.ensure(64, 64);
        c.expand_dirty_all();
        c.dirty.clear();
        assert_eq!(c.dirty_len(), 0);
        c.invalidate_chunks(&[(0, 0), (2, 0), (0, 2), (9, 9)]);
        assert_eq!(c.dirty_len(), 1);
    }

    #[test]
    fn a_zero_canvas_is_a_dropped_buffer_not_a_panic() {
        let mut c = CompCache::new();
        c.ensure(0, 0);
        assert_eq!(c.size(), (0, 0));
        assert!(c.comp().is_empty());
        assert_eq!(c.dirty_len(), 0);
        c.ensure(32, 32);
        assert_eq!(c.dirty_len(), 1);
    }
}
