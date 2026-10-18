//! `drawDisplay` order + `blitVisible` — `src/render/renderer_display.js`.

use super::background::paint_background;
use super::framebuffer::{Framebuffer, clip_rect, intersect_rects};
use super::grid::{GridLines, GridMode, grid_lines, paint_grid};
use crate::blend::rnd_u8;
use crate::constants::{BORDER_RGBA, CHUNK_PX};
use crate::pixel::Rgba;
use crate::view::{View, round_half_up};

/// One frame's worth of display inputs. `comp` is the W3 comp buffer
/// (`canvas_w * canvas_h * 4` RGBA); `None` is the JS `r._comp == null`.
pub struct DisplayParams<'a> {
    pub view: View,
    pub canvas_w: u32,
    pub canvas_h: u32,
    pub css_w: f64,
    pub css_h: f64,
    pub dpr: f64,
    /// `"transparent"` or `#rrggbb[aa]` — `doc.canvas.background`.
    pub background: &'a str,
    pub grid: GridMode,
    pub comp: Option<&'a [u8]>,
}

/// `r._dpr || 1` — 0 and NaN fall back, negative and infinity survive like the JS.
#[inline]
fn effective_dpr(raw: f64) -> f64 {
    if raw == 0.0 || raw.is_nan() { 1.0 } else { raw }
}

/// `Math.max(1, Math.round(css * dpr))` for both axes.
fn device_size(css_w: f64, css_h: f64, dpr: f64) -> (u32, u32) {
    (
        round_half_up(css_w * dpr).max(1.0) as u32,
        round_half_up(css_h * dpr).max(1.0) as u32,
    )
}

/// Allocate one frame and paint it.
pub fn draw_display(p: &DisplayParams) -> Framebuffer {
    let (w, h) = device_size(p.css_w, p.css_h, effective_dpr(p.dpr));
    let mut fb = Framebuffer::new(w, h);
    draw_display_into(p, &mut fb);
    fb
}

/// The canvas rect in device px — the `dx/dy/dw/dh` `draw_display_into` hands
/// to the background painter, the blit and the border.
///
/// Each side rounds on its own: the far edge is never `dx + dw`.
pub fn canvas_device_rect(
    view: &View,
    canvas_w: u32,
    canvas_h: u32,
    dpr: f64,
) -> (i32, i32, i32, i32) {
    let dpr = effective_dpr(dpr);
    let dx = round_half_up(view.offset_x() * dpr) as i32;
    let dy = round_half_up(view.offset_y() * dpr) as i32;
    let dw = round_half_up(canvas_w as f64 * view.zoom() * dpr) as i32;
    let dh = round_half_up(canvas_h as f64 * view.zoom() * dpr) as i32;
    (dx, dy, dw, dh)
}

/// Two roundings — the continuous corner and the blit's floored per-pixel source
/// — can each move a chunk edge by a device pixel; the box carries both.
const RECT_MARGIN: i32 = 2;

/// Conservative device-px box of one comp chunk (`cx*32 .. +32` canvas px),
/// clipped to [`canvas_device_rect`]; `None` when the chunk is off the canvas
/// or the clip leaves nothing.
///
/// UI-5 feeds this the chunks [`crate::comp_cache::CompCache::last_painted_keys`]
/// reported and uploads just that texture region. Over-covering a few pixels is
/// free (they are read from the same framebuffer); under-covering leaves a stale
/// pixel on screen, so the box only has to be a superset.
pub fn chunk_device_rect(
    view: &View,
    canvas_w: u32,
    canvas_h: u32,
    dpr: f64,
    cx: u32,
    cy: u32,
) -> Option<(i32, i32, i32, i32)> {
    let sx0 = cx * CHUNK_PX;
    let sy0 = cy * CHUNK_PX;
    if sx0 >= canvas_w || sy0 >= canvas_h {
        return None;
    }
    let sx1 = (sx0 + CHUNK_PX).min(canvas_w);
    let sy1 = (sy0 + CHUNK_PX).min(canvas_h);
    let dpr = effective_dpr(dpr);
    let z = view.zoom();
    let x0 = round_half_up((view.offset_x() + sx0 as f64 * z) * dpr) as i32;
    let y0 = round_half_up((view.offset_y() + sy0 as f64 * z) * dpr) as i32;
    let x1 = round_half_up((view.offset_x() + sx1 as f64 * z) * dpr) as i32;
    let y1 = round_half_up((view.offset_y() + sy1 as f64 * z) * dpr) as i32;
    let rect = (
        x0 - RECT_MARGIN,
        y0 - RECT_MARGIN,
        (x1 - x0) + 2 * RECT_MARGIN,
        (y1 - y0) + 2 * RECT_MARGIN,
    );
    intersect_rects(canvas_device_rect(view, canvas_w, canvas_h, dpr), rect)
}

/// Paint one frame into a caller-owned buffer, resizing it when the device size
/// changed (and clearing it either way — the JS `clearRect` ran every frame).
pub fn draw_display_into(p: &DisplayParams, fb: &mut Framebuffer) {
    let dpr = effective_dpr(p.dpr);
    let (dev_w, dev_h) = device_size(p.css_w, p.css_h, dpr);
    fb.resize_clear(dev_w, dev_h);

    let view = p.view;
    // Clip: background + blit only (the border is drawn outside it in the JS).
    let (dx, dy, dw, dh) = canvas_device_rect(&view, p.canvas_w, p.canvas_h, dpr);
    let clip = (dx, dy, dw, dh);
    paint_background(
        fb,
        p.background,
        dx,
        dy,
        dw,
        dh,
        dpr,
        Some((dev_w as f64, dev_h as f64)),
    );
    if let Some(comp) = p.comp {
        blit_visible(
            fb, comp, &view, p.canvas_w, p.canvas_h, p.css_w, p.css_h, dpr, clip,
        );
    }
    let lines: Option<GridLines> =
        grid_lines(&view, p.canvas_w, p.canvas_h, p.css_w, p.css_h, p.grid);
    paint_grid(fb, lines.as_ref(), &view, dx, dy, dw, dh, dev_w, dev_h, dpr);

    if dw > 0 && dh > 0 {
        // BORDER_RGBA, 1 device px on each of the four sides, unclipped by the
        // canvas rect — only the framebuffer bounds can trim it.
        fb.blend_rect(dx - 1, dy - 1, dw + 2, 1, BORDER_RGBA, 1.0);
        fb.blend_rect(dx - 1, dy + dh, dw + 2, 1, BORDER_RGBA, 1.0);
        fb.blend_rect(dx - 1, dy, 1, dh, BORDER_RGBA, 1.0);
        fb.blend_rect(dx + dw, dy, 1, dh, BORDER_RGBA, 1.0);
    }
}

/// `ceil(a / b)` in integers: `b > 0`, `a` any sign (`div_euclid` floors).
#[inline]
pub(super) fn ceil_div(a: i64, b: i64) -> i64 {
    let q = a.div_euclid(b);
    q + i64::from(a.rem_euclid(b) != 0)
}

/// One source colour across a destination run: an opaque pixel is a plain copy.
#[inline]
pub(super) fn fill_run(dst: &mut [u8], src: Rgba) {
    for chunk in dst.as_chunks_mut::<4>().0 {
        chunk.copy_from_slice(&src);
    }
}

/// `over_blend(dst, src, 1.0, "normal")` across a run of destination pixels that
/// all read the **same** source colour.
///
/// Byte-identical to the general routine — the loop keeps the exact f64
/// arithmetic, it just stops re-deriving `sa`, `1 - sa` and the `src * sa`
/// products, and stops re-testing the mode string, for every pixel of the run
/// (`blend_run_matches_over_blend` pins the equality).
#[inline]
pub(super) fn blend_run(dst: &mut [u8], src: Rgba) {
    let sa = src[3] as f64 / 255.0;
    if sa == 0.0 {
        return;
    }
    let inv = 1.0 - sa;
    let sc = [src[0] as f64 * sa, src[1] as f64 * sa, src[2] as f64 * sa];
    for chunk in dst.as_chunks_mut::<4>().0 {
        let da = chunk[3] as f64 / 255.0;
        let oa = sa + da * inv;
        let out = [
            rnd_u8((sc[0] + chunk[0] as f64 * da * inv) / oa),
            rnd_u8((sc[1] + chunk[1] as f64 * da * inv) / oa),
            rnd_u8((sc[2] + chunk[2] as f64 * da * inv) / oa),
            rnd_u8(oa * 255.0),
        ];
        if out[3] == 0 {
            chunk.copy_from_slice(&[0, 0, 0, 0]);
        } else {
            chunk.copy_from_slice(&out);
        }
    }
}

/// `blitVisible` — nearest-neighbour blit of the comp buffer into the canvas
/// rect, source-over (a comp pixel with alpha < 255 blends with what the
/// background painter already left there).
///
/// `rect` is the canvas rect in device px; it doubles as the clip the JS got
/// from `ctx.clip()` around the background+blit pair.
#[allow(clippy::too_many_arguments)] // JS `blitVisible(…)` verbatim; `DisplayParams` covers `draw_display`
pub fn blit_visible(
    fb: &mut Framebuffer,
    comp: &[u8],
    view: &View,
    canvas_w: u32,
    canvas_h: u32,
    css_w: f64,
    css_h: f64,
    dpr: f64,
    rect: (i32, i32, i32, i32),
) {
    if canvas_w == 0 || canvas_h == 0 {
        return;
    }
    let need = canvas_w as usize * canvas_h as usize * 4;
    if comp.len() < need {
        return;
    }
    let z = view.zoom();
    let x0 = ((0.0 - view.offset_x()) / z).floor().max(0.0) as i64;
    let y0 = ((0.0 - view.offset_y()) / z).floor().max(0.0) as i64;
    let sw = ((canvas_w as f64).min(((css_w - view.offset_x()) / z).ceil()) as i64) - x0;
    let sh = ((canvas_h as f64).min(((css_h - view.offset_y()) / z).ceil()) as i64) - y0;
    if sw <= 0 || sh <= 0 {
        return;
    }
    let ddw = round_half_up(sw as f64 * z * dpr) as i64;
    let ddh = round_half_up(sh as f64 * z * dpr) as i64;
    if ddw <= 0 || ddh <= 0 {
        return;
    }
    // Each dest side rounds separately, like dx/dw.
    let ox = round_half_up((view.offset_x() + x0 as f64 * z) * dpr) as i64;
    let oy = round_half_up((view.offset_y() + y0 as f64 * z) * dpr) as i64;
    // **Two** bounds, not one. `rect` is the canvas rect in device px, which is
    // *not* bounded by the framebuffer: zoom in past 1:1 and it is wider than
    let fb_rect = (0, 0, fb.width as i32, fb.height as i32);
    let Some(bounds) = intersect_rects(rect, fb_rect) else {
        return;
    };
    let Some((rx, ry, rw, rh)) = clip_rect(bounds, ox as i32, oy as i32, ddw as i32, ddh as i32)
    else {
        return;
    };
    let (rw, rh) = (rw as i64, rh as i64);
    let fb_w = fb.width as usize;
    let cw = canvas_w as i64;
    for row in 0..rh {
        let py = ry as i64 + row;
        // src = floor((dest_px - dest_origin) * sw / ddw): exact at integer zoom
        // and the contract for the mapping otherwise (the JS delegated it to the
        let sy = y0 + (py - oy) * sh / ddh;
        let srow = (sy * canvas_w as i64) as usize * 4;
        // One *run* per source column, not one mapping per pixel: dest pixel
        // `px` reads column `k = (px - ox) * sw / ddw`, and column `k` owns
        // `px in [ox + ceil(k*ddw/sw), ox + ceil((k+1)*ddw/sw))` — so the run
        // boundary is one division per column. The old per-pixel division cost
        // ~6.6 ms per megapixel of destination at dpr 1.5 (measured on the
        // 1838x1466 drag framebuffer, 2026-10-18, and that frame is redrawn
        // whole on every damage). Zoomed far out a column can own no pixel at
        // all; the inner loop walks on to the next column that does.
        let row_end = rx as i64 + rw;
        let mut px = rx as i64;
        let mut k = (px - ox) * sw / ddw;
        while px < row_end {
            let mut end = ox + ceil_div((k + 1) * ddw, sw);
            while end <= px {
                k += 1;
                end = ox + ceil_div((k + 1) * ddw, sw);
            }
            let run_end = end.min(row_end);
            let sx = x0 + k;
            if (0..cw).contains(&sx) {
                let so = srow + sx as usize * 4;
                let src = [comp[so], comp[so + 1], comp[so + 2], comp[so + 3]];
                let d = (py as usize * fb_w + px as usize) * 4;
                let dst = &mut fb.rgba[d..d + (run_end - px) as usize * 4];
                if src[3] == 255 {
                    fill_run(dst, src);
                } else if src[3] != 0 {
                    blend_run(dst, src);
                }
            }
            px = run_end;
            k += 1;
        }
    }
}
