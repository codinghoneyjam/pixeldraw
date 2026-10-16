//! `features/pen/pen.js` — the pencil and the eraser.
//!
//! Facade: [`PenTool`] state and stroke handling live in `core`, the brush
//! preview in `preview`, and the option-bar block in `optionbar`.

mod core;
mod preview;

pub mod optionbar;

pub use core::{PenMode, PenTool};
