//! Chunk-level change records shared by undo and stroke tracking.

use super::Chunk;
use crate::constants::CHUNK_LEN;

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
