//! The sparse layer bitmap, plus the changeset shape the undo budget speaks in.

use std::fmt;

use crate::chunkstore::alpha::{self, AlphaMask, EMPTY_MASK};
use crate::constants::{CHUNK_LEN, CHUNK_PX, MAX_CHUNKS_X, chunk_coords, chunk_key};
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

/// A stored chunk and its coverage map.
///
/// **Why the mask lives beside the bytes and not in a lookup.** It is written on
/// exactly the three mutations that can change a pixel (`set_pixel`,
/// `set_run`, `put_chunk`), so it cannot drift from the data; and it is read on
/// the *composite* path, where asking "is anything here at all?" must not cost a
/// page of pixel loads. Keeping it in the same map entry makes that impossible
/// to forget: there is no second structure to keep in sync.
#[derive(Clone)]
struct ChunkCell {
    data: Box<Chunk>,
    mask: AlphaMask,
}

/// The sparse layer bitmap.
///
/// **The index is a dense array, not a map.** A canvas holds at most
/// 64x34 = 2176 chunks and `chunk_key = cy * 64 + cx`, so the key *is* the array
/// index and there is nothing left to search for.
///
/// That matters because the hot paths probe far more often than the chunk count
/// suggests. `PixelWriter::set_run` resolves a chunk once per 32-pixel piece of
/// a run, and a pen-64 stamp is ~3,228 pixels wide at every Bresenham step — so
/// one 20-segment stroke is ~5.2M probes. The flood fill's row scan is one probe
/// per 32 pixels of a row. Each of those used to pay a `BTreeMap` descent: 4-5
/// *dependent* loads, each a likely cache miss. A dense index makes it one
/// bounds check and one load from a table that stays in L1/L2.
///
/// The 4 KB chunk bodies still live behind a `Box`, so an absent chunk still
/// allocates nothing.
///
/// **What that costs.** One pointer slot per *possible* chunk whether or not it
/// exists: 2176 x 8 bytes = **17 KB per layer** at the maximum canvas, and
/// nothing at all for a small one (the table is sized `chunks_x * chunks_y`, not
/// the 2176 maximum). At the 64-layer worst case that is 1.1 MB of index
/// against 570 MB of chunk data, so "a mostly-empty layer costs almost nothing"
/// still holds — but "nothing" is now "17 KB". That is the trade.
///
/// **Order is structural.** Slot order *is* key order *is* `(cy, cx)` order, so
/// `for_each_chunk` and serialization stay deterministic without a sort
/// (contract §2 rule 8) — for the same reason the `BTreeMap` gave it, except the
/// guarantee is now the array's own layout rather than a container property.
#[derive(Clone)]
pub struct ChunkStore {
    pub width_px: u32,
    pub height_px: u32,
    pub chunks_x: u32,
    pub chunks_y: u32,
    /// `MAX_CHUNKS_X * chunks_y` slots, indexed by [`chunk_key`]. `None` = absent.
    ///
    /// The stride is `MAX_CHUNKS_X` (64) and **not** `chunks_x`, because that is
    /// the stride `chunk_key` defines. Keeping the array stride identical to the
    /// key stride is what makes the key *be* the index, and it means a resize
    /// never has to move a chunk: only a height change touches the length.
    /// The cost is that a canvas narrower than 64 chunks pays for the unused
    /// tail of each row — 8 bytes per slot, 512 bytes per chunk row, which is
    /// noise next to the 4 KB a real chunk costs.
    slots: Vec<Option<ChunkCell>>,
    /// How many slots are occupied, so `chunk_count` is O(1) not O(slots).
    live: usize,
}

/// A dense table of 4096-byte chunks prints megabytes. The count and the geometry
/// say everything a failing assertion needs, and nothing chokes on it.
impl fmt::Debug for ChunkStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChunkStore")
            .field("width_px", &self.width_px)
            .field("height_px", &self.height_px)
            .field("chunks_x", &self.chunks_x)
            .field("chunks_y", &self.chunks_y)
            .field("slots", &self.slots.len())
            .field("chunk_count", &self.live)
            .finish()
    }
}

impl ChunkStore {
    /// Validates the canvas size the way the JS constructor did.
    pub fn new(width_px: u32, height_px: u32) -> EngineResult<Self> {
        crate::constants::require_valid_canvas_size(width_px, height_px)?;
        let (chunks_x, chunks_y) = (width_px / CHUNK_PX, height_px / CHUNK_PX);
        Ok(Self {
            width_px,
            height_px,
            chunks_x,
            chunks_y,
            slots: vec![None; MAX_CHUNKS_X as usize * chunks_y as usize],
            live: 0,
        })
    }

    /// Slot index for a chunk, or `None` when it is off the canvas.
    #[inline]
    fn slot_of(&self, cx: u32, cy: u32) -> Option<usize> {
        if cx >= self.chunks_x || cy >= self.chunks_y {
            return None;
        }
        Some(chunk_key(cx, cy) as usize)
    }

    /// Slot index without the canvas bound check, for callers that already
    /// checked. `debug_assert` keeps the bound honest in tests only.
    #[inline]
    fn slot_unchecked(&self, cx: u32, cy: u32) -> usize {
        let s = chunk_key(cx, cy) as usize;
        debug_assert!(s < self.slots.len());
        s
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
        let Some(cell) = self.slots[self.slot_unchecked(x >> 5, y >> 5)].as_ref() else {
            return 0;
        };
        let o = Self::offset(x, y);
        u32::from_le_bytes([
            cell.data[o],
            cell.data[o + 1],
            cell.data[o + 2],
            cell.data[o + 3],
        ])
    }

    /// One chunk row of RGBA pixels, `CHUNK_PX` pixels (128 bytes) starting at
    /// chunk-local column `lx`, or `None` when the chunk is absent.
    ///
    /// Absent reads back as transparent in every engine path, so a scan walk
    /// treats `None` as "32 zero pixels" rather than branching per pixel. This
    /// is what lets flood fill step a whole scanline with one slot read per 32
    /// pixels instead of one per pixel.
    #[inline]
    pub fn get_chunk_row(&self, cx: u32, y: u32, lx: u32) -> Option<&[u8]> {
        let cell = self.slots[self.slot_unchecked(cx, y >> 5)].as_ref()?;
        let ly = y & 31;
        let start = (ly * CHUNK_PX + lx) as usize * 4;
        Some(&cell.data[start..start + (CHUNK_PX - lx) as usize * 4])
    }

    /// Command/`PixelWriter`-only write. Out of canvas is ignored; `packed == 0`
    /// clears the pixel but leaves the chunk in place (JS parity).
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, packed: u32) {
        if !self.in_canvas(x, y) {
            return;
        }
        let slot = self.slot_unchecked(x >> 5, y >> 5);
        let (lx, ly) = (x & 31, y & 31);
        if packed == 0 {
            if let Some(cell) = self.slots[slot].as_mut() {
                let o = Self::offset(x, y);
                cell.data[o..o + 4].fill(0);
                alpha::unmark(&mut cell.mask, lx, ly);
            }
            return;
        }
        let cell = self.cell_mut(slot);
        let o = Self::offset(x, y);
        cell.data[o..o + 4].copy_from_slice(&packed.to_le_bytes());
        alpha::mark(&mut cell.mask, lx, ly);
    }

    /// The cell at `slot`, created if the chunk does not exist yet.
    ///
    /// `live` is maintained here and nowhere else, so it cannot drift from
    /// `slots.len() - slots.iter().filter(|s| s.is_none()).count()`.
    #[inline]
    fn cell_mut(&mut self, slot: usize) -> &mut ChunkCell {
        if self.slots[slot].is_none() {
            self.live += 1;
        }
        self.slots[slot].get_or_insert_with(|| ChunkCell {
            data: Box::new([0u8; CHUNK_LEN]),
            mask: EMPTY_MASK,
        })
    }

    /// Write a horizontal run of pixels with the same colour.
    ///
    /// The span form of [`Self::set_pixel`]: one chunk resolution and one
    /// coverage-map update for the whole run instead of one per pixel. A brush
    /// row, a scanline fill and a polygon edge are all runs, so this is the
    /// write shape most of the engine actually produces.
    #[inline]
    pub fn set_run(&mut self, cx: u32, y: u32, lx: u32, len: usize, color: &[u8; 4]) {
        if len == 0 || y >= self.height_px || cx >= self.chunks_x || (y >> 5) >= self.chunks_y {
            return;
        }
        let ly = y & 31;
        // A run never crosses a chunk: the caller splits it (PixelWriter::set_run
        // walks the run chunk by chunk). Clipping here keeps that assumption
        // honest if a future caller forgets.
        let len = len.min((CHUNK_PX - lx) as usize);
        if len == 0 {
            return;
        }
        let slot = self.slot_unchecked(cx, y >> 5);
        // A transparent run is exactly `set_pixel(.., 0)` repeated: it clears the
        // bytes of a chunk that is already there and never brings one into
        // existence. Branching here is what keeps `set_run` and `set_pixel`
        // interchangeable — including on `chunk_count`, which the file format
        // and the undo budget both read.
        if color[3] == 0 {
            if let Some(cell) = self.slots[slot].as_mut() {
                let base = (ly * CHUNK_PX + lx) as usize * 4;
                cell.data[base..base + len * 4].fill(0);
                for i in 0..len as u32 {
                    alpha::unmark(&mut cell.mask, lx + i, ly);
                }
            }
            return;
        }
        let cell = self.cell_mut(slot);
        let base = (ly * CHUNK_PX + lx) as usize * 4;
        let end = base + len * 4;
        // One colour, many pixels — a 4-byte splat. `chunks_exact_mut` writes it
        // with no allocation, which matters: the obvious `color.repeat(len)`
        // heap-allocates once per run, and a pen-64 stroke issues millions of
        // runs, which measured as a 2.6x *regression* on the pen=1 item.
        for slot in cell.data[base..end].as_chunks_mut::<4>().0.iter_mut() {
            slot.copy_from_slice(color);
        }
        alpha::mark_range(&mut cell.mask, ly, lx, lx + len as u32);
    }

    /// The stored buffer, or `None`.
    #[inline]
    pub fn get_chunk(&self, cx: u32, cy: u32) -> Option<&Chunk> {
        let slot = self.slot_of(cx, cy)?;
        self.slots[slot].as_ref().map(|c| &*c.data)
    }

    /// The stored buffer **and** its coverage map — what the composite and
    /// flatten paths ask for, so they pay one slot read per layer per chunk
    /// instead of one per pixel.
    #[inline]
    pub fn get_chunk_with_mask(&self, cx: u32, cy: u32) -> Option<(&Chunk, &AlphaMask)> {
        let slot = self.slot_of(cx, cy)?;
        self.slots[slot].as_ref().map(|c| (&*c.data, &c.mask))
    }

    /// Recompute a chunk's coverage map from its bytes.
    ///
    /// Only for callers that reach into a chunk through a different route than
    /// the three mutators above. The golden/parity tests use it to assert the
    /// mask never drifts rather than to fix it.
    pub fn refresh_chunk_mask(&mut self, cx: u32, cy: u32) {
        if let Some(slot) = self.slot_of(cx, cy)
            && let Some(cell) = self.slots[slot].as_mut()
        {
            cell.mask = alpha::from_chunk(&cell.data[..]);
        }
    }

    /// A copy, for snapshots and undo material.
    pub fn copy_chunk(&self, cx: u32, cy: u32) -> Option<Box<Chunk>> {
        let slot = self.slot_of(cx, cy)?;
        self.slots[slot].as_ref().map(|c| c.data.clone())
    }

    /// `None` deletes, out-of-range or wrong length raises `OUT_OF_RANGE`.
    /// The input is defensively copied, as the JS `data.slice()` was.
    pub fn put_chunk(&mut self, cx: u32, cy: u32, data: Option<&[u8]>) -> EngineResult<()> {
        let Some(slot) = self.slot_of(cx, cy) else {
            return Err(DrawToolError::chunk_out_of_canvas(cx, cy));
        };
        match data {
            None => {
                if self.slots[slot].take().is_some() {
                    self.live -= 1;
                }
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
                if self.slots[slot].is_none() {
                    self.live += 1;
                }
                self.slots[slot] = Some(ChunkCell {
                    mask: alpha::from_chunk(&chunk[..]),
                    data: chunk,
                });
                Ok(())
            }
        }
    }

    /// Key-ascending walk, so serialization is deterministic.
    ///
    /// The slot table *is* in key order, so this is a linear scan with no sort
    /// and no key decoding beyond splitting the index.
    pub fn for_each_chunk(&self, mut f: impl FnMut(u32, u32, &Chunk)) {
        for (i, cell) in self.slots.iter().enumerate() {
            if let Some(cell) = cell {
                let (cx, cy) = chunk_coords(i as u64);
                f(cx, cy, &cell.data);
            }
        }
    }

    /// Number of stored chunks; empty ones are not counted.
    #[inline]
    pub fn chunk_count(&self) -> usize {
        self.live
    }

    /// Delete all-zero chunks, returning how many went.
    ///
    /// One pass over the slots rather than a collect-then-remove: the old
    /// version needed the key list because `BTreeMap::remove` is O(log n) and
    /// borrowing while mutating was awkward. A slot is just `take()`-able.
    pub fn prune_empty(&mut self) -> usize {
        let mut dead = 0;
        for slot in &mut self.slots {
            if slot.as_ref().is_some_and(|c| is_all_zero(&c.data)) {
                slot.take();
                dead += 1;
            }
        }
        self.live -= dead;
        dead
    }

    /// Deep clone sharing no buffers.
    pub fn clone_deep(&self) -> Self {
        self.clone()
    }

    /// Resize, returning the chunks that fell off the edge (redo material).
    ///
    /// Growing keeps every chunk; shrinking returns what no longer has a slot,
    /// in `(cy, cx)` order so the redo material is deterministic too.
    pub fn resize_to(
        &mut self,
        new_w: u32,
        new_h: u32,
    ) -> EngineResult<Vec<(u32, u32, Box<Chunk>)>> {
        crate::constants::require_valid_canvas_size(new_w, new_h)?;
        let nx = new_w / CHUNK_PX;
        let ny = new_h / CHUNK_PX;
        let mut removed = Vec::new();
        for cy in 0..self.chunks_y {
            for cx in 0..self.chunks_x {
                if (cx >= nx || cy >= ny)
                    && let Some(cell) = self.slots[chunk_key(cx, cy) as usize].take()
                {
                    self.live -= 1;
                    removed.push((cx, cy, cell.data));
                }
            }
        }
        self.width_px = new_w;
        self.height_px = new_h;
        self.chunks_x = nx;
        self.chunks_y = ny;
        // Only the height changes the table's length, and every surviving key
        // `cy * 64 + cx` with `cy < ny` is already below the new length — so a
        // plain resize is correct in both directions and nothing has to move.
        self.slots.resize(MAX_CHUNKS_X as usize * ny as usize, None);
        debug_assert_eq!(self.live, self.slots.iter().filter(|s| s.is_some()).count());
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

/// `Option<&Chunk>` → `Option<&[u8]>`: `Box<[u8; N]>::as_deref` keeps the array
/// type and will not coerce to a slice on its own.
pub(crate) fn as_bytes(c: Option<&Chunk>) -> Option<&[u8]> {
    c.map(|b| &b[..])
}

/// Byte comparison of two optional chunks.
pub(crate) fn chunk_eq(a: Option<&[u8]>, b: Option<&[u8]>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}
