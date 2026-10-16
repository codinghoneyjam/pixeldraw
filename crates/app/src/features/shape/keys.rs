//! `features/shape/shape_keys.js` — commit, discard, placing, nudge.

use pixel_engine::raster::shape_raster::ShapeKind;

use crate::input::KeyEvent;
use crate::tools::api::ToolCx;

use super::pending::finish_placing;
use super::state::{Mode, PendingShape, ShapeTool};

/// Returns whether the key was consumed, exactly like the JS.
pub(super) fn handle_key(tool: &mut ShapeTool, ev: &KeyEvent, cx: &mut ToolCx) -> bool {
    tool.refresh_live(cx);
    if ev.key == "Enter" {
        if tool.mode == Mode::Placing {
            return finish_placing(tool, cx);
        }
        if tool.mode == Mode::Drawing || !tool.has_pending() {
            return false;
        }
        tool.op = None;
        tool.mode = Mode::Pending;
        let _ = tool.commit(cx);
        return true;
    }
    if ev.key == "Escape" || ev.key == "Delete" || ev.key == "Backspace" {
        if tool.mode == Mode::Placing && tool.place.is_some() {
            if ev.key == "Escape"
                && let Some(place) = tool.place.as_mut()
                && place.points.len() > 1
            {
                // Esc steps back one vertex; the last click is regretted.
                place.points.pop();
                tool.emit_changed(cx);
                cx.request_render();
                return true;
            }
            tool.place = None;
            tool.hover_pt = None;
            tool.mode = if tool.has_pending() {
                Mode::Pending
            } else {
                Mode::Idle
            };
            tool.emit_changed(cx);
            cx.request_render();
            return true;
        }
        if matches!(tool.mode, Mode::Drawing | Mode::Resizing | Mode::Moving) && ev.key == "Escape"
        {
            tool.cancel(cx);
            return true;
        }
        if tool.has_pending() || tool.draw.is_some() || tool.op.is_some() {
            tool.discard_pending(cx);
            return true;
        }
        return false;
    }
    let step: i32 = if ev.shift { 32 } else { 1 };
    let d = match ev.key.as_str() {
        "ArrowLeft" => (-step, 0),
        "ArrowRight" => (step, 0),
        "ArrowUp" => (0, -step),
        "ArrowDown" => (0, step),
        _ => return false,
    };
    if !tool.has_pending() || tool.mode != Mode::Pending {
        return false;
    }
    let kind = tool.kind();
    if let Some(p) = tool.pending.clone() {
        let mut geom = p.geom.clone();
        match kind {
            ShapeKind::Polygon => {
                geom.points = geom.points.iter().map(|q| (q.0 + d.0, q.1 + d.1)).collect();
            }
            ShapeKind::Line => {
                geom.p0 = geom.p0.map(|q| (q.0 + d.0, q.1 + d.1));
                geom.p1 = geom.p1.map(|q| (q.0 + d.0, q.1 + d.1));
            }
            _ => {
                if let Some(mut b) = geom.bbox {
                    b.x += d.0;
                    b.y += d.1;
                    geom.bbox = Some(b);
                }
            }
        }
        tool.pending = Some(PendingShape {
            layer_id: p.layer_id,
            color: p.color,
            geom,
        });
    }
    tool.emit_changed(cx);
    cx.request_render();
    true
}
