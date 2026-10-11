//! View / viewport math — facade.
//!
//! Re-exports the whole surface so callers keep one path: `crate::view::…`.
//! The coordinate model, the rounding rules and the parity traps are in
//! `docs/view_math.md`; this file's comments record only what a reader of the
//! signature alone could not guess.

mod chunks;
mod clamp;
mod fit;
mod geom;
mod store;
mod tests;
mod types;
mod zoom;

pub use chunks::{CHUNK_EDGE_EPS, VisibleChunkRange, visible_chunk_range};
pub use clamp::{CLAMP_MARGIN, clamp_offset, clamp_view, clamp_view_with_margin, pan_by};
pub use fit::{FIT_PADDING, actual_size_view, fit_view, fit_view_with_padding};
pub use geom::{
    CanvasPoint, Pixel, ScreenPoint, canvas_to_screen, floor_i64, pixel_at, round_half_up,
    screen_to_canvas,
};
pub use store::{ViewChange, ViewStore};
pub use types::{View, ViewOptions, Viewport, create_view, viewport_new};
pub use zoom::{ZOOM_MAX_INDEX, ZOOM_MIN_INDEX, largest_rung_at_most, step_zoom, zoom_at};

pub use crate::constants::ZOOM_LEVELS;
