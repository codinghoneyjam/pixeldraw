//! Painting — `paintDirty` and the `_update` composite half.
//!
//! The two passes and where the frame budget is measured are the whole trap here:
//! the JS painted every visible chunk with **no** budget and only started the
//! clock afterwards, so the budget never throttles what the user is looking at.

use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::composite::{LayerSource, composite_chunk};
use crate::constants::{CHUNK_LEN, CHUNK_PX, chunk_coords};
use crate::model::Document;
use crate::view::VisibleChunkRange;

use super::cache::{CHUNK_ROW, CompCache};

/// Pass 1 goes parallel once the work clears this many `chunk × layer` units.
///
/// A composite unit is cheap (~1 µs for a 1-layer chunk, ~50 µs at 64 layers)
/// while rayon's fork/join plus the batch staging cost a few hundred µs
/// regardless, so the product is a proxy for "at least half a millisecond of
/// serial work". `framebench`'s four A/B rows bracket it
/// (`docs/perf_js_vs_rust.md` §14): 2176 units *lost* 17 % with rayon (the
/// 1-layer cold grid) while 3840 units won 5.8–6.4× (a 64-layer drag).
pub(crate) const PARALLEL_MIN_UNITS: usize = 3072;

/// Visible chunks per parallel batch — bounds the staging buffer (one
/// `[u8; CHUNK_LEN]` image per chunk in the batch) while keeping the pool fed.
const PARALLEL_BATCH: usize = 64;

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
        self.paint_dirty_impl(layers, visible, budget, true)
    }

    /// [`CompCache::paint_dirty_budget`] with pass 1 on the calling thread —
    /// the pre-PERF-1 twin the bench A/Bs against and the tests pin the
    /// parallel path to (byte-identical by design).
    pub fn paint_dirty_budget_serial(
        &mut self,
        layers: &[LayerSource<'_>],
        visible: Option<VisibleChunkRange>,
        budget: Budget,
    ) -> bool {
        self.paint_dirty_impl(layers, visible, budget, false)
    }

    /// `parallel` is the PERF-1 switch: pass 1's *composite* half fans out to
    /// rayon; every blit stays on this thread, in JS key order. The comp bytes,
    /// the dirty set and `last_painted_keys` therefore do not depend on
    /// scheduling — `the_parallel_visible_pass_matches_the_serial_twin` proves
    /// it on a real document.
    fn paint_dirty_impl(
        &mut self,
        layers: &[LayerSource<'_>],
        visible: Option<VisibleChunkRange>,
        budget: Budget,
        parallel: bool,
    ) -> bool {
        self.last_painted_keys.clear();
        if self.dirty.is_empty() {
            return self.dirty_all;
        }
        // JS `const keys = [...r._dirty]`: a snapshot, so a key that leaves the
        // set while the pass runs is skipped rather than painted twice.
        let keys: Vec<u64> = self.dirty.iter().copied().collect();
        // Pass 1 — visible chunks, no budget, JS order.
        self.paint_visible_pass(layers, &keys, visible, parallel);
        // Pass 2 — everything else, and only now the clock starts. It stays
        // sequential: the budget is a wall-clock promise measured before each
        // chunk, and a parallel batch could neither check nor keep it.
        let mut painted_off_screen = 0usize;
        let t0 = Instant::now();
        for &key in &keys {
            if !self.dirty.contains(&key) {
                continue;
            }
            // The JS checked the budget *before* painting a chunk, so a frame can
            // still paint one chunk after the budget ran out (the check that
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

    /// Pass 1 of `paint_dirty`: `keys ∩ visible`, no budget.
    ///
    /// The sequential shape is one `paint_chunk` per key; the parallel shape
    /// composites a batch of them on the pool and blits the results in the same
    /// order afterwards. Out-of-canvas keys are dropped here (the sequential
    /// path did it inside `paint_chunk`) so both shapes retire the same set.
    fn paint_visible_pass(
        &mut self,
        layers: &[LayerSource<'_>],
        keys: &[u64],
        visible: Option<VisibleChunkRange>,
        parallel: bool,
    ) {
        let mut targets: Vec<u64> = Vec::new();
        for &key in keys {
            let (cx, cy) = chunk_coords(key);
            if !Self::in_visible(cx, cy, visible) {
                continue;
            }
            if self.contains_chunk(cx, cy) {
                targets.push(key);
            } else {
                self.dirty.remove(&key);
            }
        }
        if !parallel || targets.len() * layers.len() < PARALLEL_MIN_UNITS {
            for &key in &targets {
                self.paint_chunk(layers, key);
            }
            return;
        }
        for batch in targets.chunks(PARALLEL_BATCH) {
            let bufs: Vec<[u8; CHUNK_LEN]> = batch
                .par_iter()
                .map(|&key| {
                    let (cx, cy) = chunk_coords(key);
                    let mut buf = [0u8; CHUNK_LEN];
                    // The only error is a wrong buffer length; a stack array
                    // cannot trip it and the chunk stays transparent.
                    let _ = composite_chunk(layers, cx, cy, &mut buf);
                    buf
                })
                .collect();
            for (&key, buf) in batch.iter().zip(&bufs) {
                self.blit_painted(key, buf);
            }
        }
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
        // `last_painted` must read 0 when nothing was composited, so the list is
        // reset outside the `paintDirty` call rather than inside it.
        self.last_painted_keys.clear();
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
        self.blit_painted(key, &buf);
    }

    /// `putImageData` half of `paint_chunk`, split out so the parallel path can
    /// composite first and blit later without duplicating this: one row copy
    /// per chunk row, then the dirty-set retirement and the UI-5 key log.
    fn blit_painted(&mut self, key: u64, buf: &[u8; CHUNK_LEN]) {
        let (cx, cy) = chunk_coords(key);
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
        self.last_painted_keys.push((cx, cy));
    }
}
