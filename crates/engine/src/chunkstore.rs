//! Sparse chunk storage + the stroke change tracker — facade.
//!
//! Ported 1:1 from `src/core/chunkstore.js`. A layer is a map of 32×32 chunks
//! holding `[u8; CHUNK_LEN]` each; empty chunks are never created, so a
//! mostly-empty 2048×1088 layer costs almost nothing.
//!
//! **Mutation rule** (docs/contract.md §2): nobody touches a chunk outside
//! [`PixelWriter`] or a Command's `do`/`undo`. Out-of-canvas `set` is silently
//! ignored, which is why callers never clip.

mod store;
mod writer;

#[cfg(test)]
mod tests;

pub use store::{Changeset, Chunk, ChunkDiff, ChunkStore, is_all_zero};
pub use writer::PixelWriter;
