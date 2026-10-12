//! Arc/chord masks and the cropped preview blit.
//!
//! The mask is canvas-sized (up to 2048×1088); commit and overlay share
//! [`mask_for`] so the preview is the committed pixels (WYSIWYG).

use std::sync::Arc;

use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::pixel::{pack_rgba, pack_rgba_bytes, parse_hex, unpack_rgba};
use pixel_engine::raster::Mask;
use pixel_engine::raster::arc::{Box2, arc_mask, chord_fill_mask, chord_outline_mask};

use crate::tools::overlay::OverlayOp;

use super::tool::{ArcKind, ChordMode, Draw};

/// `pixel_preview.js`: preview alpha is `Math.round(a * 0.82)`.
const PREVIEW_ALPHA: f64 = 0.82;

/// JS `Math.round` (halves toward +∞), not `f64::round`.
fn round_half_up(v: f64) -> u8 {
    (v + 0.5).floor().clamp(0.0, 255.0) as u8
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PreviewKey {
    pub(super) x0: i32,
    pub(super) y0: i32,
    pub(super) x1: i32,
    pub(super) y1: i32,
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) width: i32,
    pub(super) outline: bool,
    pub(super) packed: u32,
}

pub(super) fn pack_of(css: &str) -> EngineResult<u32> {
    match parse_hex(css) {
        Some(c) => Ok(pack_rgba(
            c[0] as f64,
            c[1] as f64,
            c[2] as f64,
            c[3] as f64,
        )),
        None => Err(DrawToolError::out_of_range(format!("invalid color {css}"))),
    }
}

/// The mask + paint colour for a drag box, shared by commit and overlay.
pub(super) fn mask_for(
    kind: ArcKind,
    draw: &Draw,
    x1: i32,
    y1: i32,
    w: usize,
    h: usize,
) -> EngineResult<(Mask, u32)> {
    let b = Box2 {
        cx: f64::from(draw.x0 + x1) / 2.0,
        cy: f64::from(draw.y0 + y1) / 2.0,
        rx: f64::from((x1 - draw.x0).abs()) / 2.0,
        ry: f64::from((y1 - draw.y0).abs()) / 2.0,
    };
    match kind {
        ArcKind::Arc => {
            let mask = arc_mask(b, draw.start, draw.end, draw.width, w, h)?;
            Ok((mask, pack_of(&draw.fill_color)?))
        }
        ArcKind::Chord if draw.chord_mode == ChordMode::Fill => {
            let mask = chord_fill_mask(b, draw.start, draw.end, w, h)?;
            Ok((mask, pack_of(&draw.fill_color)?))
        }
        ArcKind::Chord => {
            let mask = chord_outline_mask(b, draw.start, draw.end, draw.width, w, h)?;
            // Outline chords paint the secondary colour, like the JS.
            Ok((mask, pack_of(&draw.outline_color)?))
        }
    }
}

/// 1-bit mask + colour → cropped [`OverlayOp::MaskBlit`] with the 0.82
/// preview alpha. `overlay.rs` `mask_to_blit` is still a W0 stub, so the
/// crop lives here until W0 implements it.
pub(super) fn blit_preview(mask: &Mask, packed: u32) -> Option<OverlayOp> {
    let [r, g, b, a] = unpack_rgba(packed);
    if a == 0 {
        return None;
    }
    let scaled = pack_rgba_bytes(r, g, b, round_half_up(f64::from(a) * PREVIEW_ALPHA));
    let (mut x0, mut y0, mut x1, mut y1) = (mask.w as i32, mask.h as i32, -1i32, -1i32);
    for y in 0..mask.h {
        for x in 0..mask.w {
            if mask.data[y * mask.w + x] == 1 {
                let (x, y) = (x as i32, y as i32);
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x1 < x0 {
        return None;
    }
    let (bw, bh) = ((x1 - x0 + 1) as usize, (y1 - y0 + 1) as usize);
    let mut data = vec![0u32; bw * bh];
    for y in y0..=y1 {
        for x in x0..=x1 {
            if mask.data[y as usize * mask.w + x as usize] == 1 {
                data[(y - y0) as usize * bw + (x - x0) as usize] = scaled;
            }
        }
    }
    Some(OverlayOp::MaskBlit {
        x: mask.x + x0,
        y: mask.y + y0,
        w: bw as u32,
        h: bh as u32,
        data: Arc::from(data.into_boxed_slice()),
    })
}
