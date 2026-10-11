//! `clipAlpha` — multiply a body slot's alpha by a rounded-rect clip mask.
//!
//! Alpha is cleared *outside* the clip, because the legacy path multiplies the
//! whole alpha channel by the mask — body art that spills past the rounded rect
//! turns transparent instead of staying opaque.

use super::common::round_half_up;
use crate::raster::PixelSink;
use crate::raster::mask::rrect_mask;

/// A rectangular clip: the two corners plus a corner radius.
#[derive(Debug, Clone, Copy)]
pub struct Clip {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub radius: i32,
}

/// Multiply the tile's alpha by a rounded-rect clip mask.
pub fn clip_alpha<S: PixelSink>(sink: &mut S, clip: &Clip, w: usize, h: usize) {
    let bw = round_half_up(clip.x1 - clip.x0 + 1.0) as usize;
    let bh = round_half_up(clip.y1 - clip.y0 + 1.0) as usize;
    let mask = match rrect_mask(bw, bh, clip.radius as i64) {
        Ok(m) => m,
        Err(_) => return,
    };
    let ox = round_half_up(clip.x0);
    let oy = round_half_up(clip.y0);
    for y in 0..h {
        for x in 0..w {
            let mx = x as i32 - ox;
            let my = y as i32 - oy;
            let inside = mx >= 0
                && my >= 0
                && (mx as usize) < mask.w
                && (my as usize) < mask.h
                && mask.is_set(mx as usize, my as usize);
            if inside {
                continue;
            }
            if sink.get(x as i32, y as i32) != 0 {
                sink.set(x as i32, y as i32, 0);
            }
        }
    }
}
