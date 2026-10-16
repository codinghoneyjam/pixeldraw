//! Option-bar parsing and commit/preview spec building.

use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::raster::shape_raster::{FillMode, ShapeKind, ShapeSpec};

use super::ops::drag_geom;
use super::types::{Bbox, DragFlags, PendingGeom, Pt};

/// Option-bar geometry for a line pending.
pub fn parse_line_geom(x0: i32, y0: i32, x1: i32, y1: i32) -> PendingGeom {
    PendingGeom {
        bbox: None,
        p0: Some((x0, y0)),
        p1: Some((x1, y1)),
        radius: 0,
        points: vec![],
    }
}

/// Option-bar geometry for a polygon pending.
pub fn parse_polygon_geom(points: Vec<Pt>) -> EngineResult<PendingGeom> {
    if points.len() < 3 {
        return Err(DrawToolError::out_of_range("polygon needs >= 3 points"));
    }
    Ok(PendingGeom {
        bbox: None,
        p0: None,
        p1: None,
        radius: 0,
        points,
    })
}

/// Option-bar geometry for a box pending.
pub fn parse_box_geom(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: Option<i32>,
    shape_radius: i32,
) -> EngineResult<PendingGeom> {
    if w < 1 || h < 1 {
        return Err(DrawToolError::out_of_range("shape w,h must be >= 1"));
    }
    let r = radius.unwrap_or(shape_radius);
    if r < 0 {
        return Err(DrawToolError::out_of_range("radius must be >= 0"));
    }
    Ok(PendingGeom {
        bbox: Some(Bbox { x, y, w, h }),
        p0: None,
        p1: None,
        radius: r,
        points: vec![],
    })
}

/// Commit-time spec: stroke width and fill mode are live settings, geometry is
/// the pending snapshot.
pub fn build_spec(
    kind: ShapeKind,
    geom: &PendingGeom,
    pen_size: u32,
    shape_fill: &str,
) -> ShapeSpec {
    let sw = pen_size as i32;
    match kind {
        ShapeKind::Line => ShapeSpec {
            kind,
            stroke_width: sw,
            fill_mode: FillMode::Outline,
            bbox: None,
            p0: geom.p0,
            p1: geom.p1,
            radius: 0,
            points: vec![],
        },
        ShapeKind::Polygon => ShapeSpec {
            kind,
            stroke_width: sw,
            fill_mode: FillMode::parse(Some(shape_fill)),
            bbox: None,
            p0: None,
            p1: None,
            radius: 0,
            points: geom.points.iter().map(|p| [p.0, p.1]).collect(),
        },
        _ => {
            let b = geom.bbox.unwrap_or(Bbox {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
            });
            ShapeSpec {
                kind,
                stroke_width: sw,
                fill_mode: FillMode::parse(Some(shape_fill)),
                bbox: Some([b.x, b.y, b.w, b.h]),
                p0: None,
                p1: None,
                radius: if kind == ShapeKind::Rrect {
                    geom.radius
                } else {
                    0
                },
                points: vec![],
            }
        }
    }
}

/// Preview spec for an in-progress draw — same shape, live flags.
pub fn draw_spec(
    kind: ShapeKind,
    p0: Pt,
    cur: Pt,
    flags: DragFlags,
    shape_radius: i32,
) -> PendingGeom {
    drag_geom(kind, p0, cur, flags, shape_radius)
}
