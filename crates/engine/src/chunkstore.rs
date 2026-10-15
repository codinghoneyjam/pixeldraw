//! Sparse chunk storage + the stroke change tracker — facade.
//!
//! Ported 1:1 from `src/core/chunkstore.js`. A layer is a map of 32×32 chunks
//! holding `[u8; CHUNK_LEN]` each; empty chunks are never created, so a
//! mostly-empty 2048×1088 layer costs almost nothing.
//!
//! **Mutation rule** (docs/contract.md §2): nobody touches a chunk outside
//! [`PixelWriter`] or a Command's `do`/`undo`. Out-of-canvas `set` is silently
//! ignored, which is why callers never clip.

mod alpha;
mod store;
mod writer;

#[cfg(test)]
mod tests;

// The coverage-map primitives are re-exported because `perflab` measures the
// mask-constant idea against them, and because anyone building their own index
pub use alpha::{AlphaMask, EMPTY_MASK, MASK_WORDS, from_chunk, mark_range};
pub use store::{Changeset, Chunk, ChunkDiff, ChunkStore, is_all_zero};
pub use writer::{PixelWriter, StrokeState};
