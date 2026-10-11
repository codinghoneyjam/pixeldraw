//! Shared overlay operation kinds.

use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum OverlayOp {
    MaskBlit {
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        data: Arc<[u32]>,
    },
    FillRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rgba: [u8; 4],
    },
    StrokeRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rgba: [u8; 4],
        dash: Option<(f64, f64)>,
        difference: bool,
    },
    Line {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        rgba: [u8; 4],
        width: f64,
        difference: bool,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
        rgba: [u8; 4],
        width: f64,
        dash: Option<(f64, f64)>,
    },
}

pub fn mask_to_blit(
    _mask: &pixel_engine::raster::Mask,
    _packed: u32,
    _alpha_scale: Option<f64>,
) -> Option<OverlayOp> {
    None
}

pub fn packed_to_blit(
    _w: usize,
    _h: usize,
    _data: &[u32],
    _alpha_scale: Option<f64>,
) -> Option<OverlayOp> {
    None
}
