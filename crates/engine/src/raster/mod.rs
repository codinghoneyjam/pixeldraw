//! `raster/` — shape mask rasterisers, ported from `src/core/raster/`.
//!
//! Submodules:
//!
//! * [`mask`] — ellipse / rounded-rect / outline-ring masks and the size guard.
//! * [`brush`] — the round pen footprint, Bresenham enumeration and stamping.
//! * [`polygon`] — the PIL `Draw.c` scanline engine (fills, closed-path
//!   outlines, wide lines, fillets).
//!
//! A mask is `{ w, h, data }` with `data` row-major and 1 = painted, exactly the
//! shape the JS tree used, so the golden fixtures read the same way on both sides.

pub mod brush;
pub mod mask;
pub mod polygon;
pub mod segment;

pub use brush::{
    BrushFootprint, PixelSink, brush_footprint, for_each_bresenham, stamp_brush, stroke_point,
    stroke_segment,
};
pub use mask::{Mask, assert_mask_size, ellipse_mask, outline_ring, rrect_mask};
pub use polygon::{
    Edge, fillet_polygon, line8, polygon_bbox, polygon_generic, polygon_inset_ring, polygon_mask,
    polygon_outline_mask, wide_line_quad_edges,
};
pub use segment::{Pt, line_mask};
