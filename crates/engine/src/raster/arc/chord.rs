//! The chord family — `draw.chord` fill and outline.
//!
//! A chord is the arc sector plus the closing diameter, so the fill is a
//! polygon over the sampled perimeter and the outline is the clipped arc stroke
//! with the closing line stroked *after* the angle clip (it lies outside the
//! sector and the clip would erase it).

use super::angle::{Box2, arc_ellipse_points, clip_angle_range, round_half_up, stroke_open_path};
use crate::errors::EngineResult;
use crate::raster::mask::{Mask, assert_mask_size};
use crate::raster::polygon::polygon_mask;

/// Chord fill mask matching PIL `draw.chord` fill.
///
/// The samples are integer-snapped and de-duplicated first, because the
/// scanline engine assumes integer vertices: a near-horizontal float edge would
/// iterate non-integer rows and paint nothing.
pub fn chord_fill_mask(
    b: Box2,
    start_deg: f64,
    end_deg: f64,
    w: usize,
    h: usize,
) -> EngineResult<Mask> {
    assert_mask_size(w, h)?;
    let mut pts: Vec<[i32; 2]> = Vec::new();
    for [x, y] in arc_ellipse_points(b, start_deg, end_deg, 0.5) {
        let q = [round_half_up(x), round_half_up(y)];
        if pts.last() == Some(&q) {
            continue;
        }
        pts.push(q);
    }
    polygon_mask(&pts, w, h)
}

/// Chord outline mask: the arc stroke, clipped, plus the closing diameter.
pub fn chord_outline_mask(
    b: Box2,
    start_deg: f64,
    end_deg: f64,
    width: i32,
    w: usize,
    h: usize,
) -> EngineResult<Mask> {
    assert_mask_size(w, h)?;
    let mut m = Mask::new(w, h);
    let pts = arc_ellipse_points(b, start_deg, end_deg, 0.5);
    stroke_open_path(&mut m.data, w, h, &pts, width);
    clip_angle_range(&mut m.data, w, h, b, start_deg, end_deg);
    // The closing line lies outside the sector, so it is stroked unclipped.
    stroke_open_path(&mut m.data, w, h, &[pts[0], pts[pts.len() - 1]], width);
    Ok(m)
}
