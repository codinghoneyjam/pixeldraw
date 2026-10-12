//! Grid line computation and painting — `src/render/grid.js`.
//!
//! `grid_lines` is the pure half (Node-importable in JS, and here: it touches no
//! framebuffer), `paint_grid` is the device-pixel painter.

use super::framebuffer::{Framebuffer, clip_rect};
use crate::constants::{
    CHUNK_PX, GRID_ALPHA_PIXEL, GRID_ALPHA_THICK, GRID_ALPHA_THIN, GRID_BEVEL_DEV_PX,
    GRID_PASS_ALPHA_THICK_DARK, GRID_PASS_ALPHA_THICK_LIGHT, GRID_PASS_ALPHA_THIN_DARK,
    GRID_PASS_ALPHA_THIN_LIGHT, GRID_THICK_CSS_PX, GRID_THIN_CSS_PX, TILE_PX,
};
use crate::view::View;
use crate::view::round_half_up;

/// The four mutually exclusive modes, matching the `gridMode` setting strings.
///
/// Unknown strings map to `Off`, which is what the JS renderer's `_gridMode()`
/// fallback did when the setting was missing or unreadable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridMode {
    Off,
    Unit,
    Tile,
    Pixel,
}

impl From<&str> for GridMode {
    fn from(s: &str) -> Self {
        match s {
            "unit" => GridMode::Unit,
            "tile" => GridMode::Tile,
            "pixel" => GridMode::Pixel,
            _ => GridMode::Off,
        }
    }
}

/// A `{ g, a }` JS entry. `a` is the **palette marker** (which weight/contrast
/// profile to use), not a weight — it is the only channel `strokeDevPx` and
/// `passAlpha` select on, so it must be one of the three constants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridStroke {
    pub g: u32,
    pub a: f64,
}

/// `gridLines` output: thinnest rung first, `pixel → unit → tile`.
#[derive(Debug, Clone, PartialEq)]
pub struct GridLines {
    pub xs: Vec<GridStroke>,
    pub ys: Vec<GridStroke>,
}

/// `strokeDevPx` — **ceil**, not round.
///
/// A 1 CSS px line at dpr 1.25 must not round down to 1 device px (0.8 CSS px)
/// or it disappears again; the 0.15 inspect rung is the fixed 1 device px
/// hairline that pins an exception at every dpr.
pub fn grid_stroke_dev_px(a: f64, dpr: f64) -> u32 {
    if a == GRID_ALPHA_PIXEL {
        return 1;
    }
    let css = if a == GRID_ALPHA_THICK {
        GRID_THICK_CSS_PX
    } else {
        GRID_THIN_CSS_PX
    };
    (css * dpr).ceil().max(1.0) as u32
}

/// `passAlpha(a, dark)` — per-pass ink; thick strokes out-ink thin ones.
#[inline]
fn pass_alpha(a: f64, dark: bool) -> f64 {
    if a == GRID_ALPHA_THICK {
        if dark {
            GRID_PASS_ALPHA_THICK_DARK
        } else {
            GRID_PASS_ALPHA_THICK_LIGHT
        }
    } else if dark {
        GRID_PASS_ALPHA_THIN_DARK
    } else {
        GRID_PASS_ALPHA_THIN_LIGHT
    }
}

/// `pushLines` — stamp `k*step` in `[lo,hi]` clipped to `[0,max]`.
///
/// `k1 = min(max/step, floor(min(max,hi)/step))` is what keeps a line from
/// being emitted past the canvas edge, and `k0 = max(0, ceil(max(0,lo)/step))`
/// what keeps a panned-off canvas from starting before pixel 0.
fn push_lines(out: &mut Vec<GridStroke>, lo: f64, hi: f64, max: u32, step: u32, alpha: f64) {
    let step_f = step as f64;
    let k0 = (0.0_f64).max(lo / step_f).ceil();
    let k1 = (max as f64 / step_f).min(((max as f64).min(hi) / step_f).floor());
    if !k0.is_finite() || !k1.is_finite() {
        return;
    }
    let (k0, k1) = (k0.floor() as i64, k1 as i64);
    for k in k0..=k1 {
        // Only reachable with k*step <= max <= 2048, so the u32 cast is exact.
        out.push(GridStroke {
            g: k as u32 * step,
            a: alpha,
        });
    }
}

/// `gridLines` — pure line list for a view/mode, `None` when zoom hides them all.
///
/// Thresholds stay at 6 CSS px of on-screen spacing for both rungs: 6 CSS px is
/// 9 device px at dpr 1.5, far wider than the 3-device-px tile bevel, so the
/// bevel can never close the gap between two neighbouring lines.
pub fn grid_lines(
    view: &View,
    w: u32,
    h: u32,
    vw: f64,
    vh: f64,
    mode: GridMode,
) -> Option<GridLines> {
    let z = view.zoom();
    let show_unit = matches!(mode, GridMode::Unit | GridMode::Pixel) && CHUNK_PX as f64 * z >= 6.0;
    let show_tile = matches!(mode, GridMode::Tile | GridMode::Pixel) && TILE_PX as f64 * z >= 6.0;
    let show_pixel = mode == GridMode::Pixel && z >= 8.0;
    if !show_unit && !show_tile && !show_pixel {
        return None;
    }
    let xa = (0.0 - view.offset_x()) / z;
    let xb = (vw - view.offset_x()) / z;
    let ya = (0.0 - view.offset_y()) / z;
    let yb = (vh - view.offset_y()) / z;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    // Order matters: pixel stays at index 0 (thinnest-first, so tile lines end
    // up drawn over unit lines).
    if show_pixel {
        push_lines(&mut xs, xa, xb, w, 1, GRID_ALPHA_PIXEL);
        push_lines(&mut ys, ya, yb, h, 1, GRID_ALPHA_PIXEL);
    }
    if show_unit {
        push_lines(&mut xs, xa, xb, w, CHUNK_PX, GRID_ALPHA_THIN);
        push_lines(&mut ys, ya, yb, h, CHUNK_PX, GRID_ALPHA_THIN);
    }
    if show_tile {
        push_lines(&mut xs, xa, xb, w, TILE_PX, GRID_ALPHA_THICK);
        push_lines(&mut ys, ya, yb, h, TILE_PX, GRID_ALPHA_THICK);
    }
    Some(GridLines { xs, ys })
}

/// `strokeRun` — one grid run: hairline `difference`, or a two-tone bevel.
///
/// TRAP: the light run's box is `main × main` at the line's origin, not a full
/// length run — that is what `cw`/`ch` in `grid.js` L102-103 compute, so the
/// port keeps it. Only the dark run spans the whole line.
#[allow(clippy::too_many_arguments)] // one grid run = one rect + two scalars; a struct buys nothing
fn stroke_run(
    fb: &mut Framebuffer,
    clip: (i32, i32, i32, i32),
    a: f64,
    x: i32,
    y: i32,
    len: i32,
    horizontal: bool,
    dpr: f64,
) {
    let px = grid_stroke_dev_px(a, dpr) as i32;
    let hair = a == GRID_ALPHA_PIXEL;
    let main = if hair { 1 } else { px };
    let (cw, ch) = if horizontal {
        (if hair { len } else { main }, main)
    } else {
        (main, if hair { len } else { main })
    };
    if hair {
        fb.difference_rect(x, y, cw, ch, a);
        return;
    }
    let Some((rx, ry, rw, rh)) = clip_rect(clip, x, y, cw, ch) else {
        return;
    };
    fb.blend_rect(rx, ry, rw, rh, [255, 255, 255, 255], pass_alpha(a, false));
    let bevel = GRID_BEVEL_DEV_PX as i32;
    let bx = if horizontal { x } else { x + px - bevel };
    let by = if horizontal { y + px - bevel } else { y };
    let (bw, bh) = if horizontal {
        (len, bevel)
    } else {
        (bevel, len)
    };
    if let Some((rx, ry, rw, rh)) = clip_rect(clip, bx, by, bw, bh) {
        fb.blend_rect(rx, ry, rw, rh, [0, 0, 0, 255], pass_alpha(a, true));
    }
}

/// `paintGrid` — device-space painter for a [`grid_lines`] result.
///
/// The grid is clipped to the canvas rect; the caller's rect is `(dx, dy, dw,
/// dh)` in device px.
#[allow(clippy::too_many_arguments)] // JS `paintGrid(…)` verbatim; rect stays split for the clip math
pub fn paint_grid(
    fb: &mut Framebuffer,
    lines: Option<&GridLines>,
    view: &View,
    dx: i32,
    dy: i32,
    dw: i32,
    dh: i32,
    dev_w: u32,
    dev_h: u32,
    dpr: f64,
) {
    let Some(lines) = lines else {
        return;
    };
    let y0 = dy.max(0);
    let y1 = (dy + dh).min(dev_h as i32);
    let gx0 = dx.max(0);
    let gx1 = (dx + dw).min(dev_w as i32);
    if y1 <= y0 || gx1 <= gx0 {
        return;
    }
    let sx = if dpr == 0.0 || dpr.is_nan() {
        1.0
    } else {
        dpr.max(1.0)
    };
    let clip = (dx, dy, dw, dh);
    for stroke in &lines.xs {
        let x = round_half_up((view.offset_x() + stroke.g as f64 * view.zoom()) * sx) as i32;
        stroke_run(fb, clip, stroke.a, x, y0, y1 - y0, false, sx);
    }
    for stroke in &lines.ys {
        let y = round_half_up((view.offset_y() + stroke.g as f64 * view.zoom()) * sx) as i32;
        stroke_run(fb, clip, stroke.a, gx0, y, gx1 - gx0, true, sx);
    }
}
