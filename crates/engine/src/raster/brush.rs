//! The round pen: footprint, Bresenham enumeration and stamping.
//!
//! Ported from `src/features/pen/brush.js` (the JS tree's SSOT; the duplicate in
//! `core/raster/raster_brush.js` was removed upstream).
//!
//! The 1·2·3px footprints are the interesting part: the rule
//! `(2x+1-n)² + (2y+1-n)² <= n²` converges to a circle for large `n` and to a
//! sensible square-ish dot for `n <= 3`, which is what the `brush` golden group
//! pins.

mod footprint;
mod sink;
mod stroke;

#[cfg(test)]
mod tests;

pub use footprint::{BrushFootprint, brush_footprint};
pub use sink::PixelSink;
pub use stroke::{for_each_bresenham, stamp_brush, stroke_point, stroke_segment};
