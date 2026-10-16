//! `features/gradient/gradient_tool.js` — the radial gradient tool.
//!
//! Facade: tool state in `tool`, packed-mask preview blit in `preview`, and
//! the option-bar block in `optionbar`.

mod preview;
mod tool;

pub mod optionbar;

#[cfg(test)]
mod tests;

pub use tool::{EdgeMode, GradientTool};
