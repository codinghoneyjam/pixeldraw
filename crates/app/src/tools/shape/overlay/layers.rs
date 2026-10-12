//! Mask paint layers — `computePaintLayers` in Rust.
//!
//! The spec expanded into coloured canvas-space masks, mirroring
//! `shape_overlay.js` (including the polygon both-mode branch) so the preview
//! is WYSIWYG against `apply_shape`.

use pixel_engine::raster::mask::{ellipse_mask, outline_ring, rrect_mask};
use pixel_engine::raster::polygon::{
    polygon_bbox, polygon_inset_ring, polygon_mask, polygon_outline_mask,
};
use pixel_engine::raster::segment::line_mask;
use pixel_engine::raster::shape_raster::{FillMode, ShapeKind};

use super::super::commit::pack_of;
use super::super::geom::PendingGeom;

/// One paint layer: canvas-space mask plus packed colour.
pub(super) struct PaintLayer {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) w: usize,
    pub(super) h: usize,
    pub(super) data: Vec<u8>,
    pub(super) packed: u32,
}

/// `computePaintLayers` — the spec expanded into coloured masks.
pub(super) fn paint_layers(
    kind: ShapeKind,
    geom: &PendingGeom,
    color: &str,
    pen_size: u32,
    fill_mode: FillMode,
) -> Vec<PaintLayer> {
    let Ok(packed) = pack_of(color) else {
        return vec![];
    };
    let sw = pen_size as i32;
    let layer = |x: i32, y: i32, mask: &pixel_engine::raster::Mask| PaintLayer {
        x,
        y,
        w: mask.w,
        h: mask.h,
        data: mask.data.clone(),
        packed,
    };
    match kind {
        ShapeKind::Line => {
            let (Some(p0), Some(p1)) = (geom.p0, geom.p1) else {
                return vec![];
            };
            match line_mask(p0, p1, sw) {
                Ok(m) => vec![layer(m.x, m.y, &m)],
                Err(_) => vec![],
            }
        }
        ShapeKind::Polygon => {
            let pts: Vec<[i32; 2]> = geom.points.iter().map(|p| [p.0, p.1]).collect();
            let Ok(bb) = polygon_bbox(&pts) else {
                return vec![];
            };
            let local: Vec<[i32; 2]> = pts.iter().map(|p| [p[0] - bb[0], p[1] - bb[1]]).collect();
            let Ok(fill) = polygon_mask(&local, bb[2] as usize, bb[3] as usize) else {
                return vec![];
            };
            if fill_mode == FillMode::Fill {
                return vec![layer(bb[0], bb[1], &fill)];
            }
            let ring = if sw >= 2 {
                polygon_inset_ring(&fill, sw)
            } else {
                match polygon_outline_mask(&local, bb[2].max(bb[3]) as usize, sw) {
                    Ok(r) => r,
                    Err(_) => return vec![],
                }
            };
            if fill_mode == FillMode::Both {
                let (rx, ry) = (ring.x, ring.y);
                vec![layer(bb[0], bb[1], &fill), layer(rx, ry, &ring)]
            } else {
                let (rx, ry) = (ring.x, ring.y);
                vec![layer(rx, ry, &ring)]
            }
        }
        _ => {
            let Some(b) = geom.bbox else {
                return vec![];
            };
            let outer = if kind == ShapeKind::Ellipse {
                ellipse_mask(b.w as usize, b.h as usize)
            } else {
                rrect_mask(b.w as usize, b.h as usize, geom.radius as i64)
            };
            let Ok(outer) = outer else {
                return vec![];
            };
            if fill_mode == FillMode::Fill {
                return vec![layer(b.x, b.y, &outer)];
            }
            let rk = if kind == ShapeKind::Ellipse {
                "ellipse"
            } else {
                "rrect"
            };
            match outline_ring(
                rk,
                b.w as usize,
                b.h as usize,
                geom.radius as i64,
                sw as usize,
            ) {
                Ok(ring) => vec![layer(b.x, b.y, &ring)],
                Err(_) => vec![],
            }
        }
    }
}
