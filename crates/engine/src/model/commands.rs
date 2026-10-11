//! Structure and property commands — facade.
//!
//! Ported 1:1 from `src/features/layers/commands.js`. The two pixel-rewriting
//! commands (`MergeDownCommand`, `ResizeCanvasCommand`) live in
//! [`crate::model::commands_pixel`], as they did in the JS tree, because they
//! carry chunk-level imports of their own.
//!
//! `docs/model_internals.md` §6 records why `AddLayerCommand::undo` throws the
//! layer away and why only opacity merges.

mod layer_cmd;
mod paint;
mod prop;

#[cfg(test)]
mod tests;

pub use layer_cmd::{AddLayerCommand, MoveLayerCommand, RemoveLayerCommand};
pub use paint::PaintCommand;
pub use prop::SetLayerPropCommand;
