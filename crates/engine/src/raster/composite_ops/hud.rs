//! `hudBrackets` — the eight L-shaped corner segments of the HUD frame.
//!
//! PIL's width-1 Bresenham path is used for every segment (not the rotated-quad
//! wide-line path), which is what the legacy `_draw_hud_brackets` did.

use crate::pixel::{Rgba, pack_rgba_bytes};
use crate::raster::PixelSink;
use crate::raster::shape_raster::{FillMode, ShapeKind, ShapeSpec, apply_shape};

/// Corner bracket parameters.
#[derive(Debug, Clone, Copy)]
pub struct Hud {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub arm: i32,
    pub color: Rgba,
    pub width: i32,
}

impl Default for Hud {
    fn default() -> Self {
        Self {
            x0: 36,
            y0: 60,
            x1: 92,
            y1: 82,
            arm: 5,
            color: [255, 255, 255, 235],
            width: 1,
        }
    }
}

/// Stroke the eight L-shaped HUD corner brackets.
pub fn hud_brackets<S: PixelSink>(sink: &mut S, hud: &Hud) {
    let packed = pack_rgba_bytes(hud.color[0], hud.color[1], hud.color[2], hud.color[3]);
    let segs = [
        ([hud.x0, hud.y0], [hud.x0 + hud.arm, hud.y0]),
        ([hud.x0, hud.y0], [hud.x0, hud.y0 + hud.arm]),
        ([hud.x1, hud.y0], [hud.x1 - hud.arm, hud.y0]),
        ([hud.x1, hud.y0], [hud.x1, hud.y0 + hud.arm]),
        ([hud.x0, hud.y1], [hud.x0 + hud.arm, hud.y1]),
        ([hud.x0, hud.y1], [hud.x0, hud.y1 - hud.arm]),
        ([hud.x1, hud.y1], [hud.x1 - hud.arm, hud.y1]),
        ([hud.x1, hud.y1], [hud.x1, hud.y1 - hud.arm]),
    ];
    for (p, q) in segs {
        let spec = ShapeSpec {
            kind: ShapeKind::Line,
            stroke_width: hud.width.max(1),
            fill_mode: FillMode::Outline,
            bbox: None,
            p0: Some((p[0], p[1])),
            p1: Some((q[0], q[1])),
            radius: 0,
            points: vec![],
        };
        let _ = apply_shape(sink, &spec, packed, packed);
    }
}
