//! `drawDisplay` order + `blitVisible` — `src/render/renderer_display.js`.

use super::background::paint_background;
use super::framebuffer::{Framebuffer, clip_rect};
use super::grid::{GridLines, GridMode, grid_lines, paint_grid};
use crate::blend::over_blend;
use crate::constants::BORDER_RGBA;
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

/// Paint one frame into a caller-owned buffer, resizing it when the device size
/// changed (and clearing it either way — the JS `clearRect` ran every frame).
pub fn draw_display_into(p: &DisplayParams, fb: &mut Framebuffer) {
    let dpr = effective_dpr(p.dpr);
    let (dev_w, dev_h) = device_size(p.css_w, p.css_h, dpr);
    fb.resize_clear(dev_w, dev_h);

    let view = p.view;
    let dx = round_half_up(view.offset_x() * dpr) as i32;
    let dy = round_half_up(view.offset_y() * dpr) as i32;
    // Each side rounds on its own: the far edge is never dx + dw.
    let dw = round_half_up(p.canvas_w as f64 * view.zoom() * dpr) as i32;
    let dh = round_half_up(p.canvas_h as f64 * view.zoom() * dpr) as i32;

    // Clip: background + blit only (the border is drawn outside it in the JS).
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
    let Some((rx, ry, rw, rh)) = clip_rect(rect, ox as i32, oy as i32, ddw as i32, ddh as i32)
    else {
        return;
    };
    let (rw, rh) = (rw as i64, rh as i64);
    let fb_w = fb.width as usize;
    for row in 0..rh {
        let py = ry as i64 + row;
        // src = floor((dest_px - dest_origin) * sw / ddw): exact at integer zoom
        // and the contract for the mapping otherwise (the JS delegated it to the
        // browser's nearest-neighbour sampling).
        let sy = y0 + (py - oy) * sh / ddh;
        let srow = (sy * canvas_w as i64) as usize * 4;
        let mut d = (py as usize * fb_w + rx as usize) * 4;
        for col in 0..rw {
            let px = rx as i64 + col;
            let sx = x0 + (px - ox) * sw / ddw;
            let so = srow + sx as usize * 4;
            let src = [comp[so], comp[so + 1], comp[so + 2], comp[so + 3]];
            let dst = &mut fb.rgba[d..d + 4];
            if src[3] == 255 {
                dst.copy_from_slice(&src);
            } else if src[3] != 0 {
                dst.copy_from_slice(&over_blend(
                    [dst[0], dst[1], dst[2], dst[3]],
                    src,
                    1.0,
                    "normal",
                ));
            }
            d += 4;
        }
    }
}
