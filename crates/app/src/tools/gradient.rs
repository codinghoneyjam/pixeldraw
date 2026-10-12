//! `features/gradient/gradient_tool.js` — the radial gradient tool.
//!
//! Facade: tool state in `tool`, packed-mask preview blit in `preview`.

mod preview;
mod tool;

#[cfg(test)]
mod tests;

pub use tool::{EdgeMode, GradientTool};
