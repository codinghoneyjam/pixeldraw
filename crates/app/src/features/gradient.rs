//! `features/gradient/gradient_tool.js` — the radial gradient tool.
//!
//! Facade: tool state in `tool`, packed-mask preview blit in `preview`, and
//! the option-bar block in `optionbar`.

mod api;
mod preview;
mod state;
mod tool;

pub mod optionbar;

#[cfg(test)]
mod tests;

pub use state::EdgeMode;
pub use state::GradientTool;
