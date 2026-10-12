//! `features/shape/shape_pending.js` — placing flow, numeric accessors, text.
//!
//! The emission text is the contract: `JSON.stringify({pending, vertices})`
//! byte-identical to the JS, key order included, so the option bar and the
//! `TOOL_STATE` listeners cannot tell the port apart.

use pixel_engine::errors::EngineResult;
use pixel_engine::raster::shape_raster::ShapeKind;

use crate::input::ToolEvent;
use crate::tools::api::ToolCx;

use super::geom::{
    Bbox, PendingGeom, Pt, parse_box_geom, parse_line_geom, parse_polygon_geom, snap_place_point,
};
use super::state::{Mode, PendingShape, ShapeTool};

/// Option-bar pending value. `None` (as `Option::None`) discards.
#[derive(Debug, Clone, PartialEq)]
pub enum PendingValue {
    LineEnds {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
    },
    LineBox {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    Polygon {
        points: Vec<Pt>,
    },
    Box {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        radius: Option<i32>,
    },
}

pub(super) fn parse_pending_value(
    kind: ShapeKind,
    v: PendingValue,
    shape_radius: u32,
) -> EngineResult<PendingGeom> {
    match (kind, v) {
        (ShapeKind::Line, PendingValue::LineEnds { x0, y0, x1, y1 }) => {
            Ok(parse_line_geom(x0, y0, x1, y1))
        }
        (ShapeKind::Line, PendingValue::LineBox { x, y, w, h }) => {
            if w < 1 || h < 1 {
                return Err(pixel_engine::errors::DrawToolError::out_of_range(
                    "line extents must be >= 1",
                ));
            }
            Ok(parse_line_geom(x, y, x + w - 1, y + h - 1))
        }
        (ShapeKind::Polygon, PendingValue::Polygon { points }) => parse_polygon_geom(points),
        (_, PendingValue::Box { x, y, w, h, radius }) => {
            parse_box_geom(x, y, w, h, radius, shape_radius as i32)
        }
        _ => Err(pixel_engine::errors::DrawToolError::out_of_range(
            "pending shape mismatch",
        )),
    }
}

/// Polygon vertex append; clicking the first vertex closes the shape.
pub(super) fn place_point(tool: &mut ShapeTool, ev: &ToolEvent, p: Pt, zoom: f64, cx: &mut ToolCx) {
    let (first, len) = match tool.place.as_ref() {
        Some(place) if !place.points.is_empty() => (place.points[0], place.points.len()),
        _ => return,
    };
    let tol = 6.0 / zoom;
    let (fx, fy) = (ev.fx, ev.fy);
    if len >= 3 && (fx - first.0 as f64).abs() <= tol && (fy - first.1 as f64).abs() <= tol {
        finish_placing(tool, cx);
        return;
    }
    if let Some(place) = tool.place.as_mut() {
        let snap = cx.session.settings().snap_unit;
        place.points.push(snap_place_point(p, snap));
    }
    tool.emit_changed(cx);
    cx.request_render();
}

/// Placing with 3+ vertices becomes pending.
pub(super) fn finish_placing(tool: &mut ShapeTool, cx: &mut ToolCx) -> bool {
    let Some(place) = tool.place.clone() else {
        return false;
    };
    if place.points.len() < 3 {
        return false;
    }
    tool.pending = Some(PendingShape {
        layer_id: place.layer_id,
        color: cx.session.settings().primary_color,
        geom: PendingGeom {
            bbox: None,
            p0: None,
            p1: None,
            radius: 0,
            points: place.points,
        },
    });
    tool.place = None;
    tool.hover_pt = None;
    // The JS set `_mode = "pending"` inline.
    tool.mode = Mode::Pending;
    tool.emit_changed(cx);
    cx.request_render();
    true
}

/// Numeric pending for the option bar.
pub(super) fn read_value(kind: ShapeKind, pending: Option<&PendingShape>) -> Option<PendingValue> {
    let p = pending?;
    match kind {
        ShapeKind::Line => {
            let (p0, p1) = (p.geom.p0?, p.geom.p1?);
            Some(PendingValue::LineEnds {
                x0: p0.0,
                y0: p0.1,
                x1: p1.0,
                y1: p1.1,
            })
        }
        ShapeKind::Polygon => Some(PendingValue::Polygon {
            points: p.geom.points.clone(),
        }),
        _ => {
            let b = p.geom.bbox?;
            Some(PendingValue::Box {
                x: b.x,
                y: b.y,
                w: b.w,
                h: b.h,
                radius: Some(p.geom.radius),
            })
        }
    }
}

fn points_json(points: &[Pt]) -> String {
    let mut s = String::from("[");
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{{\"x\":{},\"y\":{}}}", p.0, p.1));
    }
    s.push(']');
    s
}

/// `JSON.stringify(getPending())` — key order is the JS literal order.
pub(super) fn pending_json(pending: Option<&PendingShape>) -> String {
    match pending {
        None => "null".to_string(),
        Some(p) => {
            let g = &p.geom;
            if let (Some(p0), Some(p1)) = (g.p0, g.p1) {
                format!(
                    "{{\"x0\":{},\"y0\":{},\"x1\":{},\"y1\":{}}}",
                    p0.0, p0.1, p1.0, p1.1
                )
            } else if !g.points.is_empty() {
                format!("{{\"points\":{}}}", points_json(&g.points))
            } else {
                let b = g.bbox.unwrap_or(Bbox {
                    x: 0,
                    y: 0,
                    w: 1,
                    h: 1,
                });
                format!(
                    "{{\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"radius\":{}}}",
                    b.x, b.y, b.w, b.h, g.radius
                )
            }
        }
    }
}

/// `JSON.stringify({pending, vertices})` — the `_emitChanged` comparison text.
pub(super) fn emit_text(
    kind: ShapeKind,
    pending: Option<&PendingShape>,
    vertices: Option<Vec<Pt>>,
) -> String {
    let v = match (kind, vertices) {
        (ShapeKind::Polygon, Some(pts)) => points_json(&pts),
        _ => "null".to_string(),
    };
    format!(
        "{{\"pending\":{},\"vertices\":{}}}",
        pending_json(pending),
        v
    )
}
