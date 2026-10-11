//! `raster/` — shape mask rasterisers, ported from `src/core/raster/`.
//!
//! Submodules:
//!
//! * [`mask`] — ellipse / rounded-rect / outline-ring masks and the size guard.
//! * [`brush`] — the round pen footprint, Bresenham enumeration and stamping.
//! * [`polygon`] — the PIL `Draw.c` scanline engine (fills, closed-path
//!   outlines, wide lines, fillets).
//! * [`snap`] — drag/resize/unit snap geometry, the tool-side helpers.
//!
//! A mask is `{ w, h, data }` with `data` row-major and 1 = painted, exactly the
//! shape the JS tree used, so the golden fixtures read the same way on both sides.

pub mod arc;
pub mod atlas;
pub mod brush;
pub mod composite_ops;
pub mod gradient;
pub mod mask;
pub mod polygon;
pub mod segment;
pub mod shape_raster;
pub mod snap;

pub use arc::{Box2, arc_mask, chord_fill_mask, chord_outline_mask, near_parity};
pub use atlas::{Sheet, Tile, assemble_sheet, paste_channel, paste_pixel};
pub use brush::{
    BrushFootprint, PixelSink, brush_footprint, for_each_bresenham, stamp_brush, stroke_point,
    stroke_segment,
};
pub use composite_ops::{clip_alpha, erase_polygon, hud_brackets, sheen_overlay, visor_layer};
pub use gradient::{GradientMask, RadialGradient, Stop, radial_gradient_mask};
pub use mask::{Mask, assert_mask_size, ellipse_mask, outline_ring, rrect_mask};
pub use polygon::{
    Edge, fillet_polygon, line8, polygon_bbox, polygon_generic, polygon_inset_ring, polygon_mask,
    polygon_outline_mask, wide_line_quad_edges,
};
pub use segment::{Pt, line_mask};
pub use shape_raster::{FillMode, ShapeKind, ShapeSpec, apply_shape};
pub use snap::{DEFAULT_ANGLE_STEP, angle_snap, angle_snap_deg, drag_bbox, resize_bbox, unit_snap};
