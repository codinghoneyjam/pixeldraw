//! The CPU framebuffer the display pipeline paints into (roadmap §3 W2).
//!
//! Stands in for the canvas 2D context: one flat RGBA byte buffer plus the
//! composite ops the JS got from `globalCompositeOperation`. There is **no clip
//! state** — every op clips itself to the buffer, and the rect clips the JS
//! encoded as `ctx.rect(...); ctx.clip()` become explicit intersections the
//! caller passes to [`clip_rect`].

use crate::blend::{over_blend, rnd};
use crate::pixel::Rgba;

/// Device-pixel RGBA, row-major, `width * 4` stride — the layout of the JS
/// `Uint8ClampedArray` a 2D context was reading from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// `clip ∩ (x, y, w, h)`, all device px. What `ctx.rect(...) + clip()` did.
///
/// Returns `None` when the intersection is empty, so callers can skip the work
/// the canvas would have clipped away instead of walking it.
pub(crate) fn clip_rect(
    clip: (i32, i32, i32, i32),
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Option<(i32, i32, i32, i32)> {
    if w <= 0 || h <= 0 {
        return None;
    }
    let x0 = x.max(clip.0);
    let y0 = y.max(clip.1);
    let x1 = (x + w).min(clip.0 + clip.2);
    let y1 = (y + h).min(clip.1 + clip.3);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some((x0, y0, x1 - x0, y1 - y0))
}

impl Framebuffer {
    /// Transparent (all zero) — `new Uint8ClampedArray(w*h*4)`.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            rgba: vec![0; (width as usize) * (height as usize) * 4],
        }
    }

    /// JS `clearRect(0, 0, w, h)`: the whole buffer back to transparent.
    pub fn clear(&mut self) {
        self.rgba.fill(0);
    }

    /// Size the buffer and clear it. The clear is explicit because `Vec::resize`
    /// to the same length leaves the old bytes — and assigning `canvas.width`
    /// in the JS always blanked the bitmap.
    pub fn resize_clear(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.rgba
            .resize((width as usize) * (height as usize) * 4, 0);
        self.clear();
    }

    /// JS `fillStyle = <colour>` + `fillRect` for an **opaque** colour:
    /// destination replaced, alpha included.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, rgba: Rgba) {
        let Some((x, y, w, h)) = self.clamp_rect(x, y, w, h) else {
            return;
        };
        let stride = self.width as usize * 4;
        let mut base = (y * self.width as usize + x) * 4;
        for _ in 0..h {
            for px in self.rgba[base..base + w * 4].as_chunks_mut::<4>().0 {
                px.copy_from_slice(&rgba);
            }
            base += stride;
        }
    }

    /// JS `fillStyle = rgba(...,a)` + `fillRect` — source-over.
    ///
    /// Effective source alpha is `a * rgba[3] / 255`, so `BORDER_RGBA` carries
    /// its 0.6 in the byte and a grid pass carries white with the ink in `a`.
    /// Math is `over_blend` in f64, the same rounding as layer compositing.
    pub fn blend_rect(&mut self, x: i32, y: i32, w: i32, h: i32, rgba: Rgba, a: f64) {
        let Some((x, y, w, h)) = self.clamp_rect(x, y, w, h) else {
            return;
        };
        let stride = self.width as usize * 4;
        let mut base = (y * self.width as usize + x) * 4;
        for _ in 0..h {
            for px in self.rgba[base..base + w * 4].as_chunks_mut::<4>().0 {
                let out = over_blend([px[0], px[1], px[2], px[3]], rgba, a, "normal");
                px.copy_from_slice(&out);
            }
            base += stride;
        }
    }

    /// `difference` against a **white** source with ink `a`.
    ///
    /// RGB: `rnd(dst*(1-a) + a*(255-dst))` — the canvas `difference` factor
    /// `|Cs-Cb|` with `Cs = 255` inked source-over. Alpha: plain source-over
    /// with an opaque white source so the hairline still reads where the canvas
    /// version would have left a faint white trace.
    pub fn difference_rect(&mut self, x: i32, y: i32, w: i32, h: i32, a: f64) {
        let white: Rgba = [255, 255, 255, 255];
        let inv = 1.0 - a;
        let Some((x, y, w, h)) = self.clamp_rect(x, y, w, h) else {
            return;
        };
        let stride = self.width as usize * 4;
        let mut base = (y * self.width as usize + x) * 4;
        for _ in 0..h {
            for px in self.rgba[base..base + w * 4].as_chunks_mut::<4>().0 {
                let dst = [px[0], px[1], px[2], px[3]];
                let out = [
                    rnd(dst[0] as f64 * inv + a * (255.0 - dst[0] as f64)) as u8,
                    rnd(dst[1] as f64 * inv + a * (255.0 - dst[1] as f64)) as u8,
                    rnd(dst[2] as f64 * inv + a * (255.0 - dst[2] as f64)) as u8,
                    over_blend(dst, white, a, "normal")[3],
                ];
                px.copy_from_slice(&out);
            }
            base += stride;
        }
    }

    /// 1-device-px Bresenham line, source-over. For the W8 overlay ops; the
    /// grid never needs it (its runs are rects).
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, rgba: Rgba, a: f64) {
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        let mut x = x0;
        let mut y = y0;
        loop {
            // The 1px rect clips, so off-canvas endpoints need no guard here.
            self.blend_rect(x, y, 1, 1, rgba, a);
            if x == x1 && y == y1 {
                return;
            }
            let e2 = 2 * err;
            // `>=` / `<=`, exactly like `forEachBresenham` in
            // `src/features/pen/brush.js` (equality takes both steps).
            if e2 >= -dy {
                err -= dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    #[inline]
    fn clamp_rect(&self, x: i32, y: i32, w: i32, h: i32) -> Option<(usize, usize, usize, usize)> {
        let clip = (0, 0, self.width as i32, self.height as i32);
        clip_rect(clip, x, y, w, h)
            .map(|(x, y, w, h)| (x as usize, y as usize, w as usize, h as usize))
    }
}
