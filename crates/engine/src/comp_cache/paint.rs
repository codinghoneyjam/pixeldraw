//! Painting — `paintDirty` and the `_update` composite half.
//!
//! The two passes and where the frame budget is measured are the whole trap here:
//! the JS painted every visible chunk with **no** budget and only started the
//! clock afterwards, so the budget never throttles what the user is looking at.

use std::time::{Duration, Instant};

use crate::composite::{LayerSource, composite_chunk};
use crate::constants::{CHUNK_LEN, CHUNK_PX, chunk_coords};
use crate::model::Document;
use crate::view::VisibleChunkRange;

use super::cache::{CHUNK_ROW, CompCache};

/// How many chunks one frame may composite off screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Budget {
    /// No budget at all — every dirty chunk is painted this frame.
    Unlimited,
    /// Wall clock, checked **before** each chunk like the JS `nowMs() - t0 > 6`.
    Time(Duration),
    /// Deterministic chunk count, for tests only: the roadmap forbids wall-clock
    /// assertions, and a `Duration` budget's cut-off depends on the machine.
    Chunks(usize),
}

impl Budget {
    /// `FRAME_BUDGET_MS` from `docs/contract.md` §1 — what the display frame uses.
    pub fn frame() -> Self {
        Self::Time(Duration::from_millis(crate::constants::FRAME_BUDGET_MS))
    }
}

impl CompCache {
    /// `paintDirty(this, doc, view, w, h)`.
    ///
    /// Returns whether dirty chunks remain for the next frame. `budget` is the
    /// pinned `Duration` signature; [`CompCache::paint_dirty_budget`] takes the
    /// richer [`Budget`] the deterministic tests use.
    pub fn paint_dirty(
        &mut self,
        layers: &[LayerSource<'_>],
        visible: Option<VisibleChunkRange>,
        budget: Duration,
    ) -> bool {
        self.paint_dirty_budget(layers, visible, Budget::Time(budget))
    }

    pub fn paint_dirty_budget(
        &mut self,
        layers: &[LayerSource<'_>],
        visible: Option<VisibleChunkRange>,
        budget: Budget,
    ) -> bool {
        self.last_painted = 0;
        if self.dirty.is_empty() {
            return self.dirty_all;
        }
        // JS `const keys = [...r._dirty]`: a snapshot, so a key that leaves the
        // set while the pass runs is skipped rather than painted twice.
        let keys: Vec<u64> = self.dirty.iter().copied().collect();
        // Pass 1 — visible chunks, no budget, JS order.
        for &key in &keys {
            let (cx, cy) = chunk_coords(key);
            if Self::in_visible(cx, cy, visible) {
                self.paint_chunk(layers, key);
            }
        }
        // Pass 2 — everything else, and only now the clock starts.
        let mut painted_off_screen = 0usize;
        let t0 = Instant::now();
        for &key in &keys {
            if !self.dirty.contains(&key) {
                continue;
            }
            // The JS checked the budget *before* painting a chunk, so a frame can
            // still paint one chunk after the budget ran out (the check that
            // stops it is the next one).
            match budget {
                Budget::Unlimited => {}
                Budget::Time(limit) => {
                    if t0.elapsed() > limit {
                        break;
                    }
                }
                Budget::Chunks(limit) => {
                    if painted_off_screen >= limit {
                        break;
                    }
                }
            }
            self.paint_chunk(layers, key);
            painted_off_screen += 1;
        }
        !self.dirty.is_empty() || self.dirty_all
    }

    /// `_update`'s composite half: `ensure` (the JS re-checked the size every
    /// frame) → expand `dirtyAll` → `paintDirty`.
    pub fn update(
        &mut self,
        doc: &Document,
        visible: Option<VisibleChunkRange>,
        budget: Duration,
    ) -> bool {
        self.update_budget(doc, visible, Budget::Time(budget))
    }

    pub fn update_budget(
        &mut self,
        doc: &Document,
        visible: Option<VisibleChunkRange>,
        budget: Budget,
    ) -> bool {
        self.ensure(doc.canvas.width_px, doc.canvas.height_px);
        self.expand_dirty_all();
        // `last_painted` must read 0 when nothing was composited, so it is reset
        // outside the `paintDirty` call rather than inside it.
        self.last_painted = 0;
        if !self.dirty.is_empty() {
            let sources = doc.layer_sources();
            self.paint_dirty_budget(&sources, visible, budget);
        }
        // JS `if (dirty.size > 0 || dirtyAll) requestRender()` — the caller keeps
        // calling `update` while this is `true`.
        !self.dirty.is_empty() || self.dirty_all
    }

    /// JS `inVis(cx, cy)`; `vis === null` means nothing is on screen.
    fn in_visible(cx: u32, cy: u32, visible: Option<VisibleChunkRange>) -> bool {
        visible.is_some_and(|v| cx >= v.cx0 && cx <= v.cx1 && cy >= v.cy0 && cy <= v.cy1)
    }

    /// `compositeChunk(doc, cx, cy, img.data)` + `putImageData(img, cx*32, cy*32)`
    /// — the local 32×32 buffer becomes one row copy per chunk row.
    fn paint_chunk(&mut self, layers: &[LayerSource<'_>], key: u64) {
        let (cx, cy) = chunk_coords(key);
        if !self.contains_chunk(cx, cy) {
            // Cannot be blitted anywhere; dropping it keeps it from being retried
            // every frame (the JS painted past the edge and left it in the set).
            self.dirty.remove(&key);
            return;
        }
        let mut buf = [0u8; CHUNK_LEN];
        // The only error `composite_chunk` raises is a wrong buffer length, which
        // a stack `[u8; CHUNK_LEN]` cannot trip; the chunk stays transparent.
        let _ = composite_chunk(layers, cx, cy, &mut buf);
        let stride = (self.width * 4) as usize;
        let x0 = (cx * CHUNK_PX * 4) as usize;
        let y0 = (cy * CHUNK_PX) as usize;
        for row in 0..CHUNK_PX as usize {
            let dst = (y0 + row) * stride + x0;
            let src = row * CHUNK_ROW;
            if dst + CHUNK_ROW > self.comp.len() {
                break;
            }
            self.comp[dst..dst + CHUNK_ROW].copy_from_slice(&buf[src..src + CHUNK_ROW]);
        }
        self.dirty.remove(&key);
        self.last_painted += 1;
    }
}
