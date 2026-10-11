//! The sparse layer bitmap, plus the changeset shape the undo budget speaks in.

use std::collections::BTreeMap;

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
