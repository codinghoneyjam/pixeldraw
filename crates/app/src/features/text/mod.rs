//! `features/text/text_tool.js` — the bitmap-glyph text stamp.
//!
//! Clicking places a **pending** text box instead of stamping immediately. The
//! box can be dragged or moved with the X/Y fields and is baked as one history
//! entry on confirm. Colours freeze at placement time; the text/font/stroke
//! options refresh the pending box while it is open.
//!
//! `Math.round(ev.x)` is the identity here — `ToolEvent.x/y` are already the
//! floored pixel. `strokeMode` defaults to `"shape"` in the JS (`?? "shape"`),
//! hence `shape_stroke: true` below.

mod api;
mod state;
mod tool;

#[cfg(test)]
mod tests;

pub mod optionbar;

pub use state::load_glyph_sets;
pub use tool::TextTool;
