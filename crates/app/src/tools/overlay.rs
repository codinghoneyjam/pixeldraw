//! Shared overlay operation kinds.

use std::sync::Arc;

use pixel_engine::pixel::{pack_rgba_bytes, unpack_rgba};

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

fn round_half_up(v: f64) -> u8 {
    (v + 0.5).floor().clamp(0.0, 255.0) as u8
}

/// 1-bit mask + colour → cropped [`OverlayOp::MaskBlit`]. Empty masks and
/// zero-sized masks yield `None`. `alpha_scale` scales the alpha channel only.
pub fn mask_to_blit(
    mask: &pixel_engine::raster::Mask,
    packed: u32,
    alpha_scale: Option<f64>,
) -> Option<OverlayOp> {
    if mask.w == 0 || mask.h == 0 || mask.data.len() != mask.w * mask.h {
        return None;
    }
    // Bounding box of the set cells.
    let mut x0 = mask.w as i32;
    let mut y0 = mask.h as i32;
    let mut x1 = -1i32;
    let mut y1 = -1i32;
    for y in 0..mask.h {
        for x in 0..mask.w {
            if mask.data[y * mask.w + x] == 1 {
                x0 = x0.min(x as i32);
                y0 = y0.min(y as i32);
                x1 = x1.max(x as i32);
                y1 = y1.max(y as i32);
            }
        }
    }
    if x1 < x0 {
        return None;
    }
    let [r, g, b, a] = unpack_rgba(packed);
    let scale = alpha_scale.unwrap_or(1.0);
    let scaled_a = if scale == 1.0 {
        a
    } else {
        round_half_up(f64::from(a) * scale)
    };
    if scaled_a == 0 {
        return None;
    }
    let scaled = pack_rgba_bytes(r, g, b, scaled_a);
    let bw = (x1 - x0 + 1) as usize;
    let bh = (y1 - y0 + 1) as usize;
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

/// Packed u32 buffer → [`OverlayOp::MaskBlit`]. Empty/zero-sized/all-transparent
/// inputs yield `None`. `alpha_scale` scales the alpha channel only.
pub fn packed_to_blit(
    w: usize,
    h: usize,
    data: &[u32],
    alpha_scale: Option<f64>,
) -> Option<OverlayOp> {
    if w == 0 || h == 0 || data.len() != w * h {
        return None;
    }
    if !data.iter().any(|&p| p != 0) {
        return None;
    }
    let scale = alpha_scale.unwrap_or(1.0);
    let out: Vec<u32> = if scale == 1.0 {
        data.to_vec()
    } else {
        data.iter()
            .map(|&p| {
                if p == 0 {
                    return 0;
                }
                let [r, g, b, a] = unpack_rgba(p);
                let sa = round_half_up(f64::from(a) * scale);
                if sa == 0 {
                    0
                } else {
                    pack_rgba_bytes(r, g, b, sa)
                }
            })
            .collect()
    };
    if !out.iter().any(|&p| p != 0) {
        return None;
    }
    Some(OverlayOp::MaskBlit {
        x: 0,
        y: 0,
        w: w as u32,
        h: h as u32,
        data: Arc::from(out.into_boxed_slice()),
    })
}
