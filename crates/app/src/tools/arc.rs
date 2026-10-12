//! `features/arc/arc_tool.js` — arc + chord tools over one struct.
//!
//! Facade: tool state in `tool`, mask + preview-blit helpers in `preview`.

mod preview;
mod tool;

#[cfg(test)]
mod tests;

pub use tool::{ArcKind, ArcTool, ChordMode};
