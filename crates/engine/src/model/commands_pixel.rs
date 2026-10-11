//! The pixel-rewriting commands — facade.
//!
//! Ported 1:1 from `src/features/layers/commands_pixel.js`. These two rewrite
//! chunk contents rather than just layer structure, which is why the JS tree
//! split them out of `commands.js` with their own chunk/blend imports; the same
//! line is drawn here.
//!
//! `docs/model_internals.md` §4 records what the merge bakes in permanently and
//! what a resize snapshot is *not* allowed to do.

mod merge_down;
mod resize;

#[cfg(test)]
mod tests;

pub use merge_down::MergeDownCommand;
pub use resize::{CutChunk, CutLayer, ResizeCanvasCommand};
