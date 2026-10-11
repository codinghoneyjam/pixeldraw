//! PIL polygon scanline engine, ported from `src/core/raster/polygon.js` — facade.
//!
//! A port of Pillow's `Draw.c` (`ImagingDrawPolygon`, `ImagingDrawWideLine`,
//! `ImagingDrawLine`), not a generic rasteriser: the golden fixtures were baked
//! from real PIL output, so `f32` coordinates and the four rounding rules are
//! load-bearing. Rationale and traps: `docs/raster_internals.md`.
//!
//! `docs/native_plan.md` §9 reserves this module from the line rule, so the
//! split below is deliberately minimal — engine internals, mask builders, the
//! fillet geometry, tests.

mod fillet;
mod masks;
mod scanline;

#[cfg(test)]
mod tests;

pub use fillet::fillet_polygon;
pub use masks::{
    line8, points_from_json, polygon_bbox, polygon_inset_ring, polygon_mask, polygon_outline_mask,
    wide_line_quad_edges,
};
pub use scanline::{
    Edge, fround, inter_x, poly_add_edge, poly_build_edges, polygon_generic, py_round, round_c,
    round_down, round_up,
};
