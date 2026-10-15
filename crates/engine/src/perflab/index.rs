//! Candidate 2: chunk index structures, head to head.
//!
//! The engine went `BTreeMap<u64, Box<Chunk>>` → `Vec<Option<Box<Chunk>>>`
//! (see `docs/perf_js_vs_rust.md` §11-2-1). This module asks what is *left*:
//! it builds the same sparse map four ways and exposes them behind one trait so
//! the benchmark can run **identical** loops over each.
//!
//! | type | shape | why it might win |
//! |---|---|---|
//! | [`BTreeIdx`] | `BTreeMap<u64, _>` | the previous implementation, as the reference point |
//! | [`DenseIdx`] | `Vec<Option<Box<_>>>` | **what the engine uses now** |
//! | [`ArenaIdx`] | `Vec<u32>` slots → one `Vec<Cell>` arena | one heap block instead of N; no per-chunk malloc; contiguous clone |
//! | [`HashIdx`] | `HashMap<u64, _>` | the closest thing to JS's `Map`, to show what the frozen key order buys |
//!
//! **The honesty trap this module is built around.** An arena's advertised win is
//! locality — "chunks land near each other". That is only true if chunks are
//! *allocated in key order*. They are not: a document is drawn in stroke order,
//! so a pen-64 diagonal creates its chunks in a scattered sequence. Every
//! benchmark here therefore builds its fixture through the **same scattered
//! insertion order** the engine uses, so the arena gets the same scrambled
//! layout it would get in production. [`ArenaIdx::build_in_key_order`] exists
//! only to show the size of the prize if allocation order *were* sorted.

use std::collections::{BTreeMap, HashMap};

use crate::chunkstore::{AlphaMask, EMPTY_MASK};
use crate::constants::{CHUNK_LEN, CHUNK_PX, MAX_CHUNKS_X, chunk_key};

/// A stored chunk plus its coverage map — the payload every index carries, so
/// no variant is accidentally measuring a thinner data structure than another.
#[derive(Clone)]
pub struct Cell {
    pub data: Box<[u8; CHUNK_LEN]>,
    pub mask: AlphaMask,
}

impl Cell {
    pub fn blank() -> Self {
        Cell {
            data: Box::new([0u8; CHUNK_LEN]),
            mask: EMPTY_MASK,
        }
    }

    /// A cell with `len` opaque pixels of row 0 painted, so a benchmark cannot
    /// be optimised away and the coverage map is not trivially zero.
    pub fn with_span(len: u32) -> Self {
        let mut c = Cell::blank();
        for x in 0..len.min(CHUNK_PX) {
            let o = x as usize * 4;
            c.data[o..o + 4].copy_from_slice(&[1, 2, 3, 255]);
            c.mask[x as usize / 64] |= 1u64 << (x as usize % 64);
        }
        c
    }
}

/// The one interface every index implements, so the benchmark loops are shared.
pub trait ChunkIndex {
    /// Populate with `cells`, inserted **in the given order**.
    fn build(cx_total: u32, cy_total: u32, cells: &[(u32, u32, Cell)]) -> Self
    where
        Self: Sized;

    /// The buffer at `(cx, cy)`, or `None` when absent.
    fn chunk(&self, cx: u32, cy: u32) -> Option<&[u8; CHUNK_LEN]>;

    /// The coverage map at `(cx, cy)`, or `None` when absent.
    fn mask(&self, cx: u32, cy: u32) -> Option<&AlphaMask>;

    /// Create-or-return at `(cx, cy)`.
    fn chunk_mut(&mut self, cx: u32, cy: u32) -> &mut [u8; CHUNK_LEN];

    fn remove(&mut self, cx: u32, cy: u32);

    /// Key-ascending walk — the order the serializer and `for_each_chunk` need.
    fn for_each(&self, f: &mut dyn FnMut(u32, u32, &Cell));

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A full independent copy, the shape an undo snapshot takes.
    fn snapshot(&self) -> Self
    where
        Self: Sized;
}

// ---------------------------------------------------------------------------
// BTreeMap — the previous implementation.

pub struct BTreeIdx {
    map: BTreeMap<u64, Cell>,
    cx_total: u32,
    cy_total: u32,
}

impl BTreeIdx {
    #[inline]
    fn in_range(&self, cx: u32, cy: u32) -> bool {
        cx < self.cx_total && cy < self.cy_total
    }
}

impl ChunkIndex for BTreeIdx {
    fn build(cx_total: u32, cy_total: u32, cells: &[(u32, u32, Cell)]) -> Self {
        let mut map = BTreeMap::new();
        for &(cx, cy, ref c) in cells {
            map.insert(chunk_key(cx, cy), c.clone());
        }
        BTreeIdx {
            map,
            cx_total,
            cy_total,
        }
    }
    fn chunk(&self, cx: u32, cy: u32) -> Option<&[u8; CHUNK_LEN]> {
        if !self.in_range(cx, cy) {
            return None;
        }
        self.map.get(&chunk_key(cx, cy)).map(|c| &*c.data)
    }
    fn mask(&self, cx: u32, cy: u32) -> Option<&AlphaMask> {
        if !self.in_range(cx, cy) {
            return None;
        }
        self.map.get(&chunk_key(cx, cy)).map(|c| &c.mask)
    }
    fn chunk_mut(&mut self, cx: u32, cy: u32) -> &mut [u8; CHUNK_LEN] {
        &mut self
            .map
            .entry(chunk_key(cx, cy))
            .or_insert_with(Cell::blank)
            .data
    }
    fn remove(&mut self, cx: u32, cy: u32) {
        self.map.remove(&chunk_key(cx, cy));
    }
    fn for_each(&self, f: &mut dyn FnMut(u32, u32, &Cell)) {
        for (&k, c) in &self.map {
            f(
                (k % MAX_CHUNKS_X as u64) as u32,
                (k / MAX_CHUNKS_X as u64) as u32,
                c,
            );
        }
    }
    fn len(&self) -> usize {
        self.map.len()
    }
    fn snapshot(&self) -> Self {
        BTreeIdx {
            map: self.map.clone(),
            cx_total: self.cx_total,
            cy_total: self.cy_total,
        }
    }
}

// ---------------------------------------------------------------------------
// Dense — what the engine uses now.

pub struct DenseIdx {
    slots: Vec<Option<Cell>>,
    cx_total: u32,
    cy_total: u32,
}

impl DenseIdx {
    #[inline]
    fn in_range(&self, cx: u32, cy: u32) -> bool {
        cx < self.cx_total && cy < self.cy_total
    }
}

impl ChunkIndex for DenseIdx {
    fn build(cx_total: u32, cy_total: u32, cells: &[(u32, u32, Cell)]) -> Self {
        let mut s = DenseIdx {
            slots: vec![None; MAX_CHUNKS_X as usize * cy_total as usize],
            cx_total,
            cy_total,
        };
        for &(cx, cy, ref c) in cells {
            s.slots[chunk_key(cx, cy) as usize] = Some(c.clone());
        }
        s
    }
    fn chunk(&self, cx: u32, cy: u32) -> Option<&[u8; CHUNK_LEN]> {
        if !self.in_range(cx, cy) {
            return None;
        }
        self.slots[chunk_key(cx, cy) as usize]
            .as_ref()
            .map(|c| &*c.data)
    }
    fn mask(&self, cx: u32, cy: u32) -> Option<&AlphaMask> {
        if !self.in_range(cx, cy) {
            return None;
        }
        self.slots[chunk_key(cx, cy) as usize]
            .as_ref()
            .map(|c| &c.mask)
    }
    fn chunk_mut(&mut self, cx: u32, cy: u32) -> &mut [u8; CHUNK_LEN] {
        let s = chunk_key(cx, cy) as usize;
        &mut self.slots[s].get_or_insert_with(Cell::blank).data
    }
    fn remove(&mut self, cx: u32, cy: u32) {
        self.slots[chunk_key(cx, cy) as usize] = None;
    }
    fn for_each(&self, f: &mut dyn FnMut(u32, u32, &Cell)) {
        for (i, c) in self.slots.iter().enumerate() {
            if let Some(c) = c {
                f(
                    (i as u64 % MAX_CHUNKS_X as u64) as u32,
                    (i as u64 / MAX_CHUNKS_X as u64) as u32,
                    c,
                );
            }
        }
    }
    fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }
    fn snapshot(&self) -> Self {
        DenseIdx {
            slots: self.slots.clone(),
            cx_total: self.cx_total,
            cy_total: self.cy_total,
        }
    }
}

// ---------------------------------------------------------------------------
// Arena — `u32` slot table into one contiguous `Vec<Cell>`.

pub struct ArenaIdx {
    /// slot -> arena index + 1. `0` = absent, which is why `u32` works here and
    /// `Box` does not get a free null niche in the arena.
    slots: Vec<u32>,
    arena: Vec<Cell>,
    free: Vec<u32>,
    cx_total: u32,
    cy_total: u32,
}

impl ArenaIdx {
    #[inline]
    fn in_range(&self, cx: u32, cy: u32) -> bool {
        cx < self.cx_total && cy < self.cy_total
    }
}

impl ArenaIdx {
    fn blank(cx_total: u32, cy_total: u32) -> Self {
        ArenaIdx {
            slots: vec![0; MAX_CHUNKS_X as usize * cy_total as usize],
            arena: Vec::new(),
            free: Vec::new(),
            cx_total,
            cy_total,
        }
    }

    fn put(&mut self, cx: u32, cy: u32, cell: Cell) {
        let s = chunk_key(cx, cy) as usize;
        match self.free.pop() {
            // Reuse in place: no 4 KB move, unlike `Vec::swap_remove`.
            Some(j) => {
                self.arena[j as usize] = cell;
                self.slots[s] = j + 1;
            }
            None => {
                self.arena.push(cell);
                self.slots[s] = self.arena.len() as u32;
            }
        }
    }

    /// **Not** what the engine does. Allocates in key order, which is the only
    /// way the arena's locality claim can show up at all.
    pub fn build_in_key_order(cx_total: u32, cy_total: u32, cells: &[(u32, u32, Cell)]) -> Self {
        let mut idx = ArenaIdx::blank(cx_total, cy_total);
        let mut sorted = cells.to_vec();
        sorted.sort_by_key(|&(cx, cy, _)| chunk_key(cx, cy));
        for (cx, cy, c) in sorted {
            idx.put(cx, cy, c.clone());
        }
        idx
    }
}

impl ChunkIndex for ArenaIdx {
    fn build(cx_total: u32, cy_total: u32, cells: &[(u32, u32, Cell)]) -> Self {
        let mut idx = ArenaIdx::blank(cx_total, cy_total);
        for &(cx, cy, ref c) in cells {
            idx.put(cx, cy, c.clone());
        }
        idx
    }
    fn chunk(&self, cx: u32, cy: u32) -> Option<&[u8; CHUNK_LEN]> {
        if !self.in_range(cx, cy) {
            return None;
        }
        let j = self.slots[chunk_key(cx, cy) as usize];
        if j == 0 {
            return None;
        }
        Some(&self.arena[(j - 1) as usize].data)
    }
    fn mask(&self, cx: u32, cy: u32) -> Option<&AlphaMask> {
        if !self.in_range(cx, cy) {
            return None;
        }
        let j = self.slots[chunk_key(cx, cy) as usize];
        if j == 0 {
            return None;
        }
        Some(&self.arena[(j - 1) as usize].mask)
    }
    fn chunk_mut(&mut self, cx: u32, cy: u32) -> &mut [u8; CHUNK_LEN] {
        let s = chunk_key(cx, cy) as usize;
        if self.slots[s] == 0 {
            self.put(cx, cy, Cell::blank());
        }
        let j = (self.slots[s] - 1) as usize;
        &mut self.arena[j].data
    }
    fn remove(&mut self, cx: u32, cy: u32) {
        let s = chunk_key(cx, cy) as usize;
        let j = self.slots[s];
        if j != 0 {
            self.slots[s] = 0;
            self.free.push(j - 1);
        }
    }
    fn for_each(&self, f: &mut dyn FnMut(u32, u32, &Cell)) {
        for (i, &j) in self.slots.iter().enumerate() {
            if j != 0 {
                let i = i as u64;
                let k = i;
                f(
                    (k % MAX_CHUNKS_X as u64) as u32,
                    (k / MAX_CHUNKS_X as u64) as u32,
                    &self.arena[(j - 1) as usize],
                );
            }
        }
    }
    fn len(&self) -> usize {
        self.slots.iter().filter(|&&s| s != 0).count()
    }
    fn snapshot(&self) -> Self {
        ArenaIdx {
            slots: self.slots.clone(),
            arena: self.arena.clone(),
            free: self.free.clone(),
            cx_total: self.cx_total,
            cy_total: self.cy_total,
        }
    }
}

// ---------------------------------------------------------------------------
// HashMap — closest to the JS side's `Map`.

pub struct HashIdx {
    map: HashMap<u64, Cell>,
    cx_total: u32,
    cy_total: u32,
}
impl HashIdx {
    #[inline]
    fn in_range(&self, cx: u32, cy: u32) -> bool {
        cx < self.cx_total && cy < self.cy_total
    }
}

impl ChunkIndex for HashIdx {
    fn build(cx_total: u32, cy_total: u32, cells: &[(u32, u32, Cell)]) -> Self {
        HashIdx {
            map: cells
                .iter()
                .map(|&(cx, cy, ref c)| (chunk_key(cx, cy), c.clone()))
                .collect(),
            cx_total,
            cy_total,
        }
    }
    fn chunk(&self, cx: u32, cy: u32) -> Option<&[u8; CHUNK_LEN]> {
        if !self.in_range(cx, cy) {
            return None;
        }
        self.map.get(&chunk_key(cx, cy)).map(|c| &*c.data)
    }
    fn mask(&self, cx: u32, cy: u32) -> Option<&AlphaMask> {
        if !self.in_range(cx, cy) {
            return None;
        }
        self.map.get(&chunk_key(cx, cy)).map(|c| &c.mask)
    }
    fn chunk_mut(&mut self, cx: u32, cy: u32) -> &mut [u8; CHUNK_LEN] {
        &mut self
            .map
            .entry(chunk_key(cx, cy))
            .or_insert_with(Cell::blank)
            .data
    }
    fn remove(&mut self, cx: u32, cy: u32) {
        self.map.remove(&chunk_key(cx, cy));
    }
    /// **Has to sort**, and that is the point: `HashMap` iteration order is
    /// unspecified, so contract §2 rule 8 (deterministic serialization) is not
    /// free with this index — it costs a full sort on every save.
    fn for_each(&self, f: &mut dyn FnMut(u32, u32, &Cell)) {
        let mut keys: Vec<u64> = self.map.keys().copied().collect();
        keys.sort_unstable();
        for k in keys {
            f(
                (k % MAX_CHUNKS_X as u64) as u32,
                (k / MAX_CHUNKS_X as u64) as u32,
                &self.map[&k],
            );
        }
    }
    fn len(&self) -> usize {
        self.map.len()
    }
    fn snapshot(&self) -> Self {
        HashIdx {
            map: self.map.clone(),
            cx_total: self.cx_total,
            cy_total: self.cy_total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(n: usize) -> Vec<(u32, u32, Cell)> {
        (0..n)
            .map(|i| {
                (
                    i as u32 % 40,
                    (i / 40) as u32,
                    Cell::with_span((i % 32) as u32),
                )
            })
            .collect()
    }

    fn check<I: ChunkIndex>(idx: &I, cells: &[(u32, u32, Cell)]) {
        assert_eq!(idx.len(), cells.len(), "count");
        for &(cx, cy, _) in cells {
            assert!(idx.chunk(cx, cy).is_some(), "missing ({cx},{cy})");
            assert!(idx.mask(cx, cy).is_some(), "missing mask ({cx},{cy})");
        }
        assert!(
            idx.chunk(64, 0).is_none(),
            "cx past the canvas must be absent"
        );
        assert!(
            idx.chunk(0, 34).is_none(),
            "cy past the canvas must be absent"
        );
        // Key order, which the serializer depends on.
        let mut seen = Vec::new();
        idx.for_each(&mut |cx, cy, _| seen.push(chunk_key(cx, cy)));
        let mut sorted = seen.clone();
        sorted.sort_unstable();
        assert_eq!(seen, sorted, "iteration must be key-ascending");
    }

    #[test]
    fn every_index_holds_the_same_map() {
        let cells = fixture(300);
        check(&BTreeIdx::build(64, 34, &cells), &cells);
        check(&DenseIdx::build(64, 34, &cells), &cells);
        check(&ArenaIdx::build(64, 34, &cells), &cells);
        check(&ArenaIdx::build_in_key_order(64, 34, &cells), &cells);
        check(&HashIdx::build(64, 34, &cells), &cells);
    }

    #[test]
    fn remove_and_reinsert_reuses_without_growing_the_arena() {
        let cells = fixture(100);
        let mut idx = ArenaIdx::build(64, 34, &cells);
        let before = idx.arena.len();
        for &(cx, cy, _) in &cells {
            idx.remove(cx, cy);
        }
        assert_eq!(idx.len(), 0);
        for &(cx, cy, _) in &cells {
            idx.chunk_mut(cx, cy)[0] = 9;
        }
        assert_eq!(idx.len(), cells.len());
        assert_eq!(idx.arena.len(), before, "free list must be reused in place");
        assert_eq!(idx.chunk(cells[0].0, cells[0].1).expect("present")[0], 9);
    }

    #[test]
    fn snapshots_are_independent() {
        let cells = fixture(50);
        let mut idx = DenseIdx::build(64, 34, &cells);
        let snap = idx.snapshot();
        *idx.chunk_mut(0, 0) = [7; CHUNK_LEN];
        assert_ne!(snap.chunk(0, 0).expect("present")[0], 7);
    }

    #[test]
    fn the_arena_and_the_dense_index_return_identical_bytes() {
        let cells = fixture(200);
        let dense = DenseIdx::build(64, 34, &cells);
        let arena = ArenaIdx::build(64, 34, &cells);
        for &(cx, cy, ref c) in &cells {
            assert_eq!(dense.chunk(cx, cy), Some(&*c.data), "dense ({cx},{cy})");
            assert_eq!(arena.chunk(cx, cy), Some(&*c.data), "arena ({cx},{cy})");
            assert_eq!(dense.mask(cx, cy), Some(&c.mask));
            assert_eq!(arena.mask(cx, cy), Some(&c.mask));
        }
    }
}
