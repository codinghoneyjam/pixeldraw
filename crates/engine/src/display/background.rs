//! Screen-fixed checkerboard + background fill — `src/render/background.js`.

use super::framebuffer::{Framebuffer, clip_rect};
use crate::constants::{CHECKER_CSS_PX, CHECKER_RGBA_A, CHECKER_RGBA_B};
use crate::pixel::parse_hex;
use crate::view::round_half_up;

/// `paintChecker` — A over the rect, then B on every other cell.
///
/// Cells are anchored at the **device origin** (phase never follows the canvas
/// or the pan), which is what makes the "paint only the viewport intersection"
/// shortcut below produce byte-identical pixels to a full-rect loop.
///
/// `viewport` is the device viewport in px (`None` = the JS `null`, no clamp).
pub fn paint_checker(
    fb: &mut Framebuffer,
    dx: i32,
    dy: i32,
    dw: i32,
    dh: i32,
    dpr: f64,
    viewport: Option<(f64, f64)>,
) {
    if dw <= 0 || dh <= 0 {
        return;
    }
    // max(1, round(css*dpr)); Rust's f64::max returns the non-NaN operand, so a
    // NaN dpr lands on 1 instead of poisoning every coordinate.
    let cell = round_half_up(CHECKER_CSS_PX * dpr).max(1.0) as i64;
    let (dx, dy, dw, dh) = (dx as i64, dy as i64, dw as i64, dh as i64);

    let (vx0, vy0, vx1, vy1) = match viewport {
        Some((vw, vh)) if vw.is_finite() && vh.is_finite() => {
            // Exclusive limits; `ceil` turns a fractional `min(dx+dw, vw)` into
            // the integer bound the same set of pixels satisfies.
            let vx0 = dx.max(0);
            let vy0 = dy.max(0);
            let vx1 = (dx + dw).min(vw.ceil() as i64);
            let vy1 = (dy + dh).min(vh.ceil() as i64);
            if vx1 <= vx0 || vy1 <= vy0 {
                return;
            }
            (vx0, vy0, vx1, vy1)
        }
        _ => (dx, dy, dx + dw, dy + dh),
    };

    fb.fill_rect(
        vx0 as i32,
        vy0 as i32,
        (vx1 - vx0) as i32,
        (vy1 - vy0) as i32,
        CHECKER_RGBA_A,
    );

    let x0 = vx0.div_euclid(cell) * cell;
    let y0 = vy0.div_euclid(cell) * cell;
    let mut iy = y0.div_euclid(cell);
    let mut y = y0;
    while y < vy1 {
        let mut ix = x0.div_euclid(cell);
        let mut x = x0;
        while x < vx1 {
            // ((ix+iy)%2 + 2)%2 → rem_euclid; B lands where the parity is odd.
            if (ix + iy).rem_euclid(2) != 0
                && let Some((cx, cy, cw, ch)) = clip_rect(
                    (
                        vx0 as i32,
                        vy0 as i32,
                        (vx1 - vx0) as i32,
                        (vy1 - vy0) as i32,
                    ),
                    x as i32,
                    y as i32,
                    cell as i32,
                    cell as i32,
                )
            {
                fb.fill_rect(cx, cy, cw, ch, CHECKER_RGBA_B);
            }
            x += cell;
            ix += 1;
        }
        y += cell;
        iy += 1;
    }
}

/// `paintBackground` — the four documented outcomes, in the JS order:
/// `transparent` / unparseable / `a === 0` → checker only; `a < 255` → checker
/// then source-over; `a === 255` → flat colour.
#[allow(clippy::too_many_arguments)] // JS `paintBackground(ctx, …dx,dy,dw,dh,dpr,viewport)` verbatim
pub fn paint_background(
    fb: &mut Framebuffer,
    background: &str,
    dx: i32,
    dy: i32,
    dw: i32,
    dh: i32,
    dpr: f64,
    viewport: Option<(f64, f64)>,
) {
    // The JS also tested `typeof background !== "string"`; with a `&str` that
    // branch is unreachable.
    let bg = if background == "transparent" {
        None
    } else {
        parse_hex(background)
    };
    let Some(bg) = bg.filter(|c| c[3] != 0) else {
        paint_checker(fb, dx, dy, dw, dh, dpr, viewport);
        return;
    };
    if bg[3] != 255 {
        paint_checker(fb, dx, dy, dw, dh, dpr, viewport);
        // JS passed the alpha through `toFixed(3)`, so a browser and this f64
        // formula can disagree by ≤1 LSB — the formula is the pinned contract.
    }
    if bg[3] == 255 {
        fb.fill_rect(dx, dy, dw, dh, bg);
    } else {
        fb.blend_rect(dx, dy, dw, dh, bg, 1.0);
    }
}
