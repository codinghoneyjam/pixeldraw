//! Display pipeline — the CPU port of `src/render/{background,grid,renderer_display}.js`.
//!
//! DOM-free, GPU-free: [`draw_display`] takes a view, a canvas size, the comp
//! buffer and the two settings (background, grid) and returns an RGBA
//! framebuffer in **device** pixels, exactly the pixels the JS canvas had.
//! No overlays (W8 rasterises `OverlayOp`s on top of the result).

mod background;
mod frame;
mod framebuffer;
mod grid;

#[cfg(test)]
mod tests;

pub use background::{paint_background, paint_checker};
pub use frame::{DisplayParams, blit_visible, draw_display, draw_display_into};
pub use framebuffer::Framebuffer;
pub use grid::{GridLines, GridMode, GridStroke, grid_lines, grid_stroke_dev_px, paint_grid};
