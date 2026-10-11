//! `sheenOverlay` — two diagonal white lines masked to the visor shape.
//!
//! A bright main line (alpha 110, width 2) plus a dimmer offset line (alpha 60,
//! width 1), both alpha-composited onto the tile through the visor mask.
//!
//! **Stage order is load-bearing.** The legacy path draws the sheen into its own
//! overlay, multiplies that overlay's *alpha* by the visor mask, and only then
//! composites. Compositing first and masking after is a different image, so the
//! three stages below stay separate. The mask multiply is `ImageChops.multiply`
//! — a per-channel multiply, NOT a lerp.

use super::common::{channels_of, round_half_up};
use super::visor::{VisorShape, shape_fill_mask, visor_to_recipe};
use crate::pixel::pack_rgba_bytes;
use crate::raster::PixelSink;
use crate::raster::segment::line_mask;

/// A sheen line definition.
#[derive(Debug, Clone, Copy)]
pub struct Sheen {
    pub p1: [f64; 2],
    pub p2: [f64; 2],
    pub dx: f64,
}

/// Overlay two diagonal sheen lines, masked to the visor shape.
pub fn sheen_overlay<S: PixelSink>(
    sink: &mut S,
    visor: &VisorShape,
    sheen: Option<&Sheen>,
    w: usize,
    h: usize,
) {
    let Some(sheen) = sheen else { return };
    let mask = shape_fill_mask(&visor_to_recipe(visor), w, h);
    let lines = [
        (
            [sheen.p1[0], sheen.p1[1]],
            [sheen.p2[0], sheen.p2[1]],
            110i32,
            2i32,
        ),
        (
            [sheen.p1[0] + sheen.dx, sheen.p1[1]],
            [sheen.p2[0] + sheen.dx, sheen.p2[1]],
            60,
            1,
        ),
    ];

    // Stage 1: the sheen lives in its own overlay.
    let mut overlay = vec![0u8; w * h * 4];
    for (a, b, alpha, width) in lines {
        let Ok(m) = line_mask(
            (round_half_up(a[0]), round_half_up(a[1])),
            (round_half_up(b[0]), round_half_up(b[1])),
            width,
        ) else {
            continue;
        };
        for y in 0..m.h {
            for x in 0..m.w {
                if !m.is_set(x, y) {
                    continue;
                }
                let tx = m.x + x as i32;
                let ty = m.y + y as i32;
                if tx < 0 || ty < 0 || tx >= w as i32 || ty >= h as i32 {
                    continue;
                }
                let o = (ty as usize * w + tx as usize) * 4;
                overlay[o] = 255;
                overlay[o + 1] = 255;
                overlay[o + 2] = 255;
                overlay[o + 3] = overlay[o + 3].max(alpha as u8);
            }
        }
    }

    // Stage 2: multiply the overlay alpha by the visor mask.
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            if overlay[o + 3] == 0 {
                continue;
            }
            let m = if mask.is_set(x, y) { 255u32 } else { 0 };
            overlay[o + 3] = ((overlay[o + 3] as u32 * m) / 255) as u8;
        }
    }

    // Stage 3: alpha_composite onto the tile.
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            let s_n = overlay[o + 3] as f64 / 255.0;
            if s_n == 0.0 {
                continue;
            }
            let px = sink.get(x as i32, y as i32);
            let c = channels_of(px);
            let d_n = c[3] as f64 / 255.0;
            let o_n = s_n + d_n * (1.0 - s_n);
            let mix = |sc: u8| {
                round_half_up((overlay[o] as f64 * s_n + sc as f64 * d_n * (1.0 - s_n)) / o_n)
            };
            sink.set(
                x as i32,
                y as i32,
                pack_rgba_bytes(
                    mix(c[0]) as u8,
                    mix(c[1]) as u8,
                    mix(c[2]) as u8,
                    round_half_up(o_n * 255.0) as u8,
                ),
            );
        }
    }
}
