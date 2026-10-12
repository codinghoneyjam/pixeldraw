//! Pen brush preview — footprint wash plus outline at `zoom >= 2`, crosshair
//! below. Device pixels throughout; the `+0.5` canvas-AA nudges from the JS
//! are deliberately absent (the W8 rasterizer draws aliased 1px lines, where
//! the nudge would shift a whole pixel).

use pixel_engine::raster::brush::brush_footprint;
use pixel_engine::view::canvas_to_screen;

use crate::tools::api::OverlayInfo;
use crate::tools::overlay::OverlayOp;

use super::core::{PenMode, PenTool};

pub(super) fn overlay_preview(tool: &PenTool, info: &OverlayInfo, out: &mut Vec<OverlayOp>) {
    // While a stroke is live the last committed point stands in; idle, a
    // stale point must NOT linger after the pointer left (JS trap).
    let pos = if tool.drawing {
        tool.hover.or(tool.last_point)
    } else {
        tool.hover
    };
    let Some((px, py)) = pos else { return };
    let zoom = info.view.zoom();
    let dpr = info.dpr;
    let dev = |cx: i32, cy: i32| {
        let p = canvas_to_screen(info.view, cx as f64, cy as f64);
        (p.x * dpr, p.y * dpr)
    };
    if zoom >= 2.0 {
        let n = if tool.drawing {
            tool.size
        } else {
            tool.preview_size
        };
        let Ok(fp) = brush_footprint(n) else { return };
        let step = (zoom * dpr).max(1.0);
        for by in 0..n {
            for bx in 0..n {
                if !fp.is_set(bx, by) {
                    continue;
                }
                let (dx, dy) = dev(
                    px - fp.offset as i32 + bx as i32,
                    py - fp.offset as i32 + by as i32,
                );
                out.push(OverlayOp::FillRect {
                    x: dx,
                    y: dy,
                    w: step,
                    h: step,
                    // `rgba(255,255,255,0.35)` — the byte carries the alpha.
                    rgba: [255, 255, 255, 89],
                });
            }
        }
        let (ox, oy) = dev(px - fp.offset as i32, py - fp.offset as i32);
        out.push(OverlayOp::StrokeRect {
            x: ox,
            y: oy,
            w: n as f64 * step,
            h: n as f64 * step,
            rgba: [255, 255, 255, 255],
            dash: match tool.mode {
                PenMode::Erase => Some((4.0 * dpr, 2.0 * dpr)),
                PenMode::Draw => None,
            },
            difference: true,
        });
        return;
    }
    let c = canvas_to_screen(info.view, px as f64 + 0.5, py as f64 + 0.5);
    let (cx, cy) = (c.x * dpr, c.y * dpr);
    let r = 5.0 * dpr;
    for ((x0, y0), (x1, y1)) in [((cx - r, cy), (cx + r, cy)), ((cx, cy - r), (cx, cy + r))] {
        out.push(OverlayOp::Line {
            x0,
            y0,
            x1,
            y1,
            rgba: [255, 255, 255, 255],
            width: 1.0,
            difference: true,
        });
    }
}
