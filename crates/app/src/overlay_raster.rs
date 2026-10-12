//! `OverlayOp` rasterizer — tool previews painted into the display framebuffer.
//!
//! The W2 framebuffer ops (`fill_rect`, `blend_rect`, `difference_rect`,
//! `draw_line`) are the only paint used here. Mask layers arrive in canvas
//! pixels and scale by `zoom*dpr` with nearest; the rect/line ops arrive in
//! device pixels and round to the pixel grid.

use pixel_engine::display::Framebuffer;
use pixel_engine::view::View;

use crate::tools::overlay::OverlayOp;

/// Device scale for one canvas pixel.
fn scale(view: View, dpr: f64) -> f64 {
    (view.zoom() * dpr).max(1.0)
}

/// Canvas pixel → device pixel origin.
fn dev(view: View, dpr: f64, x: i32, y: i32) -> (i32, i32) {
    let ox = (view.offset_x() + x as f64 * view.zoom()) * dpr;
    let oy = (view.offset_y() + y as f64 * view.zoom()) * dpr;
    (ox.round() as i32, oy.round() as i32)
}

fn unpack(packed: u32) -> [u8; 4] {
    [
        (packed & 0xff) as u8,
        ((packed >> 8) & 0xff) as u8,
        ((packed >> 16) & 0xff) as u8,
        ((packed >> 24) & 0xff) as u8,
    ]
}

/// 1px Bresenham walk calling `f` per pixel — the `draw_line` twin with a
/// caller-chosen blend, so `difference` lines share the exact walk.
fn walk_line(mut x0: i32, mut y0: i32, x1: i32, y1: i32, mut f: impl FnMut(i32, i32)) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    loop {
        f(x0, y0);
        if x0 == x1 && y0 == y1 {
            return;
        }
        let e2 = 2 * err;
        if e2 >= -dy {
            err -= dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Dashed 1px rect outline: `dash` is `(on, off)` in device px, `None` is solid.
#[allow(clippy::too_many_arguments)] // one rect + paint + dash: a struct buys nothing
fn stroke_rect_dashed(
    fb: &mut Framebuffer,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    rgba: [u8; 4],
    dash: Option<(f64, f64)>,
    difference: bool,
) {
    if w <= 0 || h <= 0 {
        return;
    }
    let paint = |fb: &mut Framebuffer, px: i32, py: i32| {
        if difference {
            let a = rgba[3] as f64 / 255.0;
            fb.difference_rect(px, py, 1, 1, a);
        } else {
            fb.blend_rect(px, py, 1, 1, rgba, 1.0);
        }
    };
    let (on, off) = dash.unwrap_or((f64::INFINITY, 0.0));
    // Edge pixels in JS stroke order: top, right, bottom, left.
    let mut edge: Vec<(i32, i32)> = vec![];
    for dx in 0..w {
        edge.push((x + dx, y));
    }
    for dy in 1..h {
        edge.push((x + w - 1, y + dy));
    }
    if h > 1 {
        for dx in (0..w - 1).rev() {
            edge.push((x + dx, y + h - 1));
        }
    }
    if w > 1 {
        for dy in (1..h - 1).rev() {
            edge.push((x, y + dy));
        }
    }
    if !on.is_finite() {
        for (px, py) in edge {
            paint(fb, px, py);
        }
        return;
    }
    let period = (on + off).max(1.0);
    for (i, (px, py)) in edge.iter().enumerate() {
        if (i as f64 % period) < on {
            paint(fb, *px, *py);
        }
    }
}

fn stroke_circle(
    fb: &mut Framebuffer,
    cx: f64,
    cy: f64,
    r: f64,
    rgba: [u8; 4],
    width: f64,
    dash: Option<(f64, f64)>,
) {
    if r <= 0.0 {
        return;
    }
    // One outline walk; `width > 1` thickens by concentric walks.
    let rings = width.round().max(1.0) as i32;
    let steps = ((r * 6.0).ceil() as i32).max(24);
    let (on, off) = dash.unwrap_or((f64::INFINITY, 0.0));
    let period = (on + off).max(1.0);
    for ring in 0..rings {
        let rr = r - ring as f64;
        if rr <= 0.0 {
            break;
        }
        for i in 0..steps {
            if on.is_finite() && (i as f64 % period) >= on {
                continue;
            }
            let a = (i as f64 / steps as f64) * std::f64::consts::TAU;
            fb.blend_rect(
                (cx + rr * a.cos()).round() as i32,
                (cy + rr * a.sin()).round() as i32,
                1,
                1,
                rgba,
                1.0,
            );
        }
    }
}

/// Paint `ops` over the display framebuffer, in order.
pub fn rasterize_overlay(fb: &mut Framebuffer, ops: &[OverlayOp], view: View, dpr: f64) {
    let k = scale(view, dpr);
    for op in ops {
        match op {
            OverlayOp::MaskBlit {
                x,
                y,
                w,
                h: _,
                data,
            } => {
                let cell = k.max(1.0) as i32;
                for (i, &packed) in data.iter().enumerate() {
                    if packed == 0 {
                        continue;
                    }
                    let dx = *x + (i as i32 % *w as i32);
                    let dy = *y + (i as i32 / *w as i32);
                    let (ox, oy) = dev(view, dpr, dx, dy);
                    fb.blend_rect(ox, oy, cell, cell, unpack(packed), 1.0);
                }
            }
            OverlayOp::FillRect { x, y, w, h, rgba } => {
                fb.blend_rect(
                    x.round() as i32,
                    y.round() as i32,
                    (*w).round().max(1.0) as i32,
                    (*h).round().max(1.0) as i32,
                    *rgba,
                    1.0,
                );
            }
            OverlayOp::StrokeRect {
                x,
                y,
                w,
                h,
                rgba,
                dash,
                difference,
            } => {
                stroke_rect_dashed(
                    fb,
                    x.round() as i32,
                    y.round() as i32,
                    (*w).round().max(1.0) as i32,
                    (*h).round().max(1.0) as i32,
                    *rgba,
                    *dash,
                    *difference,
                );
            }
            OverlayOp::Line {
                x0,
                y0,
                x1,
                y1,
                rgba,
                width: _,
                difference,
            } => {
                let (ax, ay) = (x0.round() as i32, y0.round() as i32);
                let (bx, by) = (x1.round() as i32, y1.round() as i32);
                if *difference {
                    let a = rgba[3] as f64 / 255.0;
                    walk_line(ax, ay, bx, by, |px, py| fb.difference_rect(px, py, 1, 1, a));
                } else {
                    walk_line(ax, ay, bx, by, |px, py| {
                        fb.blend_rect(px, py, 1, 1, *rgba, 1.0)
                    });
                }
            }
            OverlayOp::Circle {
                cx,
                cy,
                r,
                rgba,
                width,
                dash,
            } => {
                stroke_circle(fb, *cx, *cy, *r, *rgba, *width, *dash);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixel_engine::view::View;

    fn view1() -> View {
        View::new(1.0, 0.0, 0.0).expect("valid view")
    }

    fn px(fb: &Framebuffer, x: u32, y: u32) -> [u8; 4] {
        let o = ((y * fb.width + x) * 4) as usize;
        [fb.rgba[o], fb.rgba[o + 1], fb.rgba[o + 2], fb.rgba[o + 3]]
    }

    #[test]
    fn mask_blit_scales_by_zoom() {
        let mut fb = Framebuffer::new(16, 16);
        let ops = vec![OverlayOp::MaskBlit {
            x: 1,
            y: 1,
            w: 2,
            h: 1,
            data: std::sync::Arc::from(vec![0xff0000ff, 0].into_boxed_slice()),
        }];
        rasterize_overlay(&mut fb, &ops, view1(), 1.0);
        assert_eq!(px(&fb, 1, 1), [255, 0, 0, 255]);
        assert_eq!(px(&fb, 2, 1), [0, 0, 0, 0], "zero cells stay transparent");
    }

    #[test]
    fn dashed_frame_skips_the_gaps() {
        let mut fb = Framebuffer::new(16, 4);
        let ops = vec![OverlayOp::StrokeRect {
            x: 0.0,
            y: 0.0,
            w: 12.0,
            h: 2.0,
            rgba: [255, 255, 255, 255],
            dash: Some((2.0, 2.0)),
            difference: false,
        }];
        rasterize_overlay(&mut fb, &ops, view1(), 1.0);
        assert_eq!(px(&fb, 0, 0), [255, 255, 255, 255]);
        assert_eq!(px(&fb, 1, 0), [255, 255, 255, 255]);
        assert_eq!(px(&fb, 2, 0), [0, 0, 0, 0], "dash gap");
        assert_eq!(px(&fb, 3, 0), [0, 0, 0, 0], "dash gap");
        assert_eq!(px(&fb, 4, 0), [255, 255, 255, 255], "dash resumes");
    }

    #[test]
    fn difference_line_inverts() {
        let mut fb = Framebuffer::new(8, 1);
        fb.fill_rect(0, 0, 8, 1, [0, 0, 0, 255]);
        let ops = vec![OverlayOp::Line {
            x0: 0.0,
            y0: 0.0,
            x1: 7.0,
            y1: 0.0,
            rgba: [255, 255, 255, 255],
            width: 1.0,
            difference: true,
        }];
        rasterize_overlay(&mut fb, &ops, view1(), 1.0);
        assert_eq!(px(&fb, 3, 0)[0], 255, "black inverts to white");
    }
}
