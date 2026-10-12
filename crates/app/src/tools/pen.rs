//! `features/pen/pen.js` — the pencil and the eraser.
//!
//! Facade: [`PenTool`] state and stroke handling live in `core`, the brush
//! preview in `preview`.

mod core;
mod preview;

pub use core::{PenMode, PenTool};
