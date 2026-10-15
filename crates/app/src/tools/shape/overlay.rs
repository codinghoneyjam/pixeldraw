//! Shape overlay — preview spec, dashed frame, handles.
//!
//! Mirrors `shape_render.js` `renderToolOverlay`, but the canvas calls become
//! [`OverlayOp`]s. [`OverlayOp::MaskBlit`] carries **canvas** pixels (W8 scales
//! it by `zoom*dpr` with nearest); rect ops are **device** `f64`.
//!
//! The preview reads the tool's cached live settings (refreshed on every
//! pointer/key event): `overlay()` itself has no session, so an option-bar
//! edit lands via `set_pending`, which refreshes the cache too.

use pixel_engine::raster::polygon::polygon_bbox;
use pixel_engine::raster::shape_raster::{FillMode, ShapeKind};
use pixel_engine::view::canvas_to_screen;

use std::sync::Arc;

use crate::tools::api::OverlayInfo;
use crate::tools::overlay::OverlayOp;

use layers::paint_layers;

use super::geom::{DragFlags, PendingGeom, draw_spec, layout_handles};
use super::state::{Mode, ShapeTool};

mod layers;

/// The preview the overlay draws: draw spec, placing spec, or pending.
/// Returns the spec kind (handles follow it, not the tool kind), geometry,
/// colour, stroke width and fill mode.
fn preview(tool: &ShapeTool) -> Option<(ShapeKind, PendingGeom, String, u32, FillMode)> {
    let live = &tool.live;
    let fill = FillMode::parse(Some(live.shape_fill));
    match tool.mode {
        Mode::Drawing => {
            let draw = tool.draw.as_ref()?;
            let cur = draw.cur.unwrap_or(draw.p0);
            let flags = DragFlags {
                shift: draw.shift,
                lock: draw.shift || live.shape_lock_aspect,
                center: draw.alt,
                snap: live.snap_unit,
            };
            let geom = draw_spec(tool.kind, draw.p0, cur, flags, live.shape_radius as i32);
            Some((
                tool.kind,
                geom,
                live.primary_color.clone(),
                live.pen_size,
                fill,
            ))
        }
        Mode::Placing => {
            let place = tool.place.as_ref()?;
            let placed = &place.points;
            let mut trail = placed.clone();
            if let Some(hov) = tool.hover_pt {
                trail.push(hov);
            }
            if trail.len() >= 3 {
                // `placingSpec`: hover trail first, then the placed polygon.
                let geom = PendingGeom {
                    bbox: None,
                    p0: None,
                    p1: None,
                    radius: 0,
                    points: trail,
                };
                return Some((
                    ShapeKind::Polygon,
                    geom,
                    live.primary_color.clone(),
                    live.pen_size,
                    fill,
                ));
            }
            if let (Some(&last), Some(hov)) = (placed.last(), tool.hover_pt) {
                let geom = PendingGeom {
                    bbox: None,
                    p0: Some(last),
                    p1: Some(hov),
                    radius: 0,
                    points: vec![],
                };
                return Some((
                    ShapeKind::Line,
                    geom,
                    live.primary_color.clone(),
                    live.pen_size,
                    fill,
                ));
            }
            if placed.len() >= 3 {
                let geom = PendingGeom {
                    bbox: None,
                    p0: None,
                    p1: None,
                    radius: 0,
                    points: placed.clone(),
                };
                return Some((
                    ShapeKind::Polygon,
                    geom,
                    live.primary_color.clone(),
                    live.pen_size,
                    fill,
                ));
            }
            None
        }
        _ => {
            let shape = tool.pending.as_ref()?;
            Some((
                tool.kind,
                shape.geom.clone(),
                shape.color.clone(),
                live.pen_size,
                fill,
            ))
        }
    }
}

/// Frame box: line min/max+1, polygon bbox, box bbox — like the JS.
fn frame_box(kind: ShapeKind, geom: &PendingGeom) -> Option<[i32; 4]> {
    match kind {
        ShapeKind::Line => {
            let (p0, p1) = (geom.p0?, geom.p1?);
            Some([
                p0.0.min(p1.0),
                p0.1.min(p1.1),
                (p1.0 - p0.0).abs() + 1,
                (p1.1 - p0.1).abs() + 1,
            ])
        }
        ShapeKind::Polygon => {
            let pts: Vec<[i32; 2]> = geom.points.iter().map(|p| [p.0, p.1]).collect();
            polygon_bbox(&pts).ok()
        }
        _ => {
            let b = geom.bbox?;
            Some([b.x, b.y, b.w, b.h])
        }
    }
}

pub(super) fn shape_overlay(tool: &ShapeTool, info: &OverlayInfo, out: &mut Vec<OverlayOp>) {
    let Some((spec_kind, geom, color, pen_size, fill)) = preview(tool) else {
        return;
    };
    for layer in paint_layers(spec_kind, &geom, &color, pen_size, fill) {
        // Row-major packed canvas pixels; 0 cells stay transparent.
        let data: Vec<u32> = layer
            .data
            .iter()
            .map(|&c| if c == 1 { layer.packed } else { 0 })
            .collect();
        out.push(OverlayOp::MaskBlit {
            x: layer.x,
            y: layer.y,
            w: layer.w as u32,
            h: layer.h as u32,
            data: Arc::from(data.into_boxed_slice()),
        });
    }
    let Some([bx, by, bw, bh]) = frame_box(spec_kind, &geom) else {
        return;
    };
    let dpr = info.dpr;
    let dev = |x: i32, y: i32| {
        let p = canvas_to_screen(info.view, x as f64, y as f64);
        (p.x * dpr, p.y * dpr)
    };
    let devf = |x: f64, y: f64| {
        let p = canvas_to_screen(info.view, x, y);
        (p.x * dpr, p.y * dpr)
    };
    let (ox, oy) = dev(bx, by);
    let k = info.view.zoom() * dpr;
    out.push(OverlayOp::StrokeRect {
        x: ox,
        y: oy,
        w: (bw as f64 * k).max(1.0),
        h: (bh as f64 * k).max(1.0),
        rgba: [0, 0, 0, 255],
        dash: Some((4.0 * dpr, 3.0 * dpr)),
        difference: false,
    });
    // Handles follow the spec kind: a placing line previews legs, not boxes.
    // Mid-edge handles land on x.5 canvas points; the float mapping keeps
    let hs = 8.0 * dpr;
    for h in layout_handles(spec_kind, &geom) {
        let (hx, hy) = devf(h.x, h.y);
        out.push(OverlayOp::FillRect {
            x: hx - hs / 2.0,
            y: hy - hs / 2.0,
            w: hs,
            h: hs,
            rgba: [255, 255, 255, 255],
        });
        out.push(OverlayOp::StrokeRect {
            x: hx - hs / 2.0,
            y: hy - hs / 2.0,
            w: hs,
            h: hs,
            rgba: [0, 0, 0, 255],
            dash: None,
            difference: false,
        });
    }
}
