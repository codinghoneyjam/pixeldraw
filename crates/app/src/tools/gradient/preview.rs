//! Packed gradient mask → cropped preview blit with the 0.82 alpha.
//!
//! `overlay.rs` `packed_to_blit` is still a W0 stub, so the crop lives here
//! until W0 implements it.

use std::sync::Arc;

use pixel_engine::pixel::{pack_rgba_bytes, unpack_rgba};

use crate::tools::overlay::OverlayOp;

/// `pixel_preview.js`: preview alpha is `Math.round(a * 0.82)`.
const PREVIEW_ALPHA: f64 = 0.82;

/// JS `Math.round` (halves toward +∞), not `f64::round`.
fn round_half_up(v: f64) -> u8 {
    (v + 0.5).floor().clamp(0.0, 255.0) as u8
}

/// Packed mask → cropped [`OverlayOp::MaskBlit`] with the 0.82 preview alpha.
pub(super) fn blit_preview(w: usize, h: usize, data: &[u32]) -> Option<OverlayOp> {
    let (mut x0, mut y0, mut x1, mut y1) = (w as i32, h as i32, -1i32, -1i32);
    for y in 0..h {
        for x in 0..w {
            if data[y * w + x] != 0 {
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
    let mut out = vec![0u32; bw * bh];
    for y in y0..=y1 {
        for x in x0..=x1 {
            let p = data[y as usize * w + x as usize];
            if p != 0 {
                let [r, g, b, a] = unpack_rgba(p);
                out[(y - y0) as usize * bw + (x - x0) as usize] =
                    pack_rgba_bytes(r, g, b, round_half_up(f64::from(a) * PREVIEW_ALPHA));
            }
        }
    }
    Some(OverlayOp::MaskBlit {
        x: x0,
        y: y0,
        w: bw as u32,
        h: bh as u32,
        data: Arc::from(out.into_boxed_slice()),
    })
}
