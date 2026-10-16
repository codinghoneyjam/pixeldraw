//! `features/arc/arc_tool.js` — arc + chord tools over one struct.
//!
//! Facade: tool state in `tool`, mask + preview-blit helpers in `preview`,
//! and the option-bar block in `optionbar`.

mod preview;
mod tool;

pub mod optionbar;

#[cfg(test)]
mod tests;

pub use tool::{ArcKind, ArcTool, ChordMode};
