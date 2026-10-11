//! `erasePolygon` — punch `cutouts` polygons to alpha 0.

use crate::raster::PixelSink;
use crate::raster::polygon::polygon_mask;

/// Punch a polygon to fully transparent alpha.
pub fn erase_polygon<S: PixelSink>(sink: &mut S, points: &[[i32; 2]], w: usize, h: usize) {
    if let Ok(mask) = polygon_mask(points, w, h) {
        for y in 0..h {
            for x in 0..w {
                if mask.is_set(x, y) {
                    sink.set(x as i32, y as i32, 0);
                }
            }
        }
    }
}
