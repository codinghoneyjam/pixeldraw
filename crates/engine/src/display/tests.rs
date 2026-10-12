//! Unit tests for the framebuffer ops and the background painter.

use super::background::{paint_background, paint_checker};
use super::framebuffer::Framebuffer;
use super::grid::GridMode;
use crate::constants::{CHECKER_RGBA_A, CHECKER_RGBA_B};

fn px(fb: &Framebuffer, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * fb.width + x) * 4) as usize;
    [fb.rgba[o], fb.rgba[o + 1], fb.rgba[o + 2], fb.rgba[o + 3]]
}

fn fill(fb: &mut Framebuffer, rgba: [u8; 4]) {
    for px in fb.rgba.as_chunks_mut::<4>().0 {
        px.copy_from_slice(&rgba);
    }
}

/// The contract's `rnd`, spelled out so the expectations are hand-computed.
fn rnd_u8(v: f64) -> u8 {
    (v + 0.5).floor() as u8
}

#[test]
fn new_is_transparent_and_clear_keeps_size() {
    let mut fb = Framebuffer::new(3, 2);
    assert_eq!(fb.rgba.len(), 3 * 2 * 4);
    assert!(fb.rgba.iter().all(|&b| b == 0));
    fill(&mut fb, [1, 2, 3, 4]);
    fb.clear();
    assert!(fb.rgba.iter().all(|&b| b == 0));
    assert_eq!((fb.width, fb.height), (3, 2));

    // resize must clear by itself, the way assigning canvas.width did
    fill(&mut fb, [9, 9, 9, 9]);
    fb.resize_clear(4, 4);
    assert_eq!(fb.rgba.len(), 4 * 4 * 4);
    assert!(fb.rgba.iter().all(|&b| b == 0));
}

#[test]
fn rect_ops_clip_to_the_buffer_and_skip_empty_rects() {
    let mut fb = Framebuffer::new(4, 3);
    fb.fill_rect(-2, -2, 3, 3, [10, 20, 30, 255]);
    assert_eq!(px(&fb, 0, 0), [10, 20, 30, 255]);
    assert_eq!(px(&fb, 1, 0), [0, 0, 0, 0], "x=2 is past the -2+3 edge");
    fb.fill_rect(3, 2, 4, 4, [40, 50, 60, 255]);
    assert_eq!(px(&fb, 3, 2), [40, 50, 60, 255]);
    fb.fill_rect(4, 0, 2, 2, [255, 0, 0, 255]);
    assert_eq!(
        px(&fb, 3, 2),
        [40, 50, 60, 255],
        "an off-edge rect paints nothing"
    );
    fb.blend_rect(0, 0, 0, 5, [255, 255, 255, 255], 1.0);
    fb.blend_rect(10, 10, 2, 2, [255, 255, 255, 255], 1.0);
    assert_eq!(px(&fb, 0, 0), [10, 20, 30, 255]);
}

#[test]
fn blend_rect_is_the_source_over_formula_in_f64() {
    let mut fb = Framebuffer::new(2, 1);
    fb.fill_rect(0, 0, 1, 1, [0, 0, 0, 255]);
    fb.blend_rect(0, 0, 1, 1, [255, 255, 255, 255], 0.3);
    // rnd(0*(1-0.3) + 255*0.3) = rnd(76.5) = 77
    assert_eq!(px(&fb, 0, 0), [77, 77, 77, 255]);
    fb.fill_rect(1, 0, 1, 1, [0, 0, 0, 0]);
    fb.blend_rect(1, 0, 1, 1, [255, 255, 255, 255], 0.3);
    // transparent dst keeps the source colour, alpha = rnd(0.3*255) = 77
    assert_eq!(px(&fb, 1, 0), [255, 255, 255, 77]);
}

#[test]
fn blend_rect_multiplies_the_byte_alpha_by_the_extra_factor() {
    let mut fb = Framebuffer::new(1, 1);
    fb.fill_rect(0, 0, 1, 1, [0, 0, 0, 255]);
    // BORDER_RGBA: 153/255 == 0.6 exactly, so 0.6 * 1.0 == 1 * 0.6.
    fb.blend_rect(0, 0, 1, 1, [0, 0, 0, 153], 1.0);
    assert_eq!(px(&fb, 0, 0), [0, 0, 0, 255]);
    let mut fb2 = Framebuffer::new(1, 1);
    fb2.fill_rect(0, 0, 1, 1, [255, 255, 255, 255]);
    fb2.blend_rect(0, 0, 1, 1, [0, 0, 0, 153], 1.0);
    assert_eq!(px(&fb2, 0, 0)[0], rnd_u8(255.0 * 0.4));
}

#[test]
fn difference_rect_pulls_the_backdrop_toward_its_inverse() {
    let mut fb = Framebuffer::new(3, 1);
    fb.fill_rect(0, 0, 1, 1, [255, 255, 255, 255]);
    fb.fill_rect(1, 0, 1, 1, [0, 0, 0, 255]);
    fb.fill_rect(2, 0, 1, 1, [200, 200, 200, 255]);
    fb.difference_rect(0, 0, 3, 1, 0.15);
    // rnd(dst*(1-a) + a*(255-dst))
    assert_eq!(px(&fb, 0, 0)[0], rnd_u8(255.0 * 0.85));
    assert_eq!(px(&fb, 1, 0)[0], rnd_u8(0.15 * 255.0));
    assert_eq!(px(&fb, 2, 0)[0], rnd_u8(200.0 * 0.85 + 0.15 * 55.0));
    // A mid-tone nets out to nothing, exactly why the visible rungs are a bevel.
    let mid = 255 - rnd_u8(255.0 * 0.15);
    assert_eq!(px(&fb, 0, 0)[0], mid);
}

#[test]
fn draw_line_walks_bresenham_and_clips() {
    let mut fb = Framebuffer::new(6, 4);
    fb.draw_line(0, 0, 5, 3, [255, 255, 255, 255], 1.0);
    // Same walk as `forEachBresenham` in `src/features/pen/brush.js`: (4,2),
    // not (4,3) — hand-traced against the JS error terms, not assumed.
    let lit = |x: u32, y: u32| px(&fb, x, y)[0] == 255;
    assert!(lit(0, 0) && lit(1, 1) && lit(2, 1) && lit(3, 2) && lit(4, 2) && lit(5, 3));
    assert!(!lit(4, 3) && !lit(5, 0) && !lit(0, 3));
    // A line starting off-canvas only lights the visible part.
    fb.draw_line(-3, 0, 3, 0, [255, 255, 255, 255], 1.0);
    assert_eq!(px(&fb, 0, 0)[0], 255);
    assert_eq!(px(&fb, 3, 0)[0], 255);
}

#[test]
fn checker_cell_and_phase_follow_the_device_origin() {
    for (dpr, cell) in [(1.0, 8u32), (1.25, 10), (1.5, 12), (2.0, 16), (3.0, 24)] {
        let mut fb = Framebuffer::new(48, 32);
        paint_checker(&mut fb, 0, 0, 48, 32, dpr, None);
        // Cell size comes from the device origin: B starts at the second cell.
        assert_eq!(px(&fb, 0, 0), CHECKER_RGBA_A);
        assert_eq!(px(&fb, cell, 0), CHECKER_RGBA_B, "dpr {dpr}");
        assert_eq!(px(&fb, 0, cell), CHECKER_RGBA_B, "dpr {dpr}");
        assert_eq!(px(&fb, cell, cell), CHECKER_RGBA_A, "dpr {dpr}");
        assert_eq!(px(&fb, 2 * cell - 1, 0), CHECKER_RGBA_B, "dpr {dpr}");
        assert_eq!(px(&fb, 2 * cell, 0), CHECKER_RGBA_A, "dpr {dpr}");
    }
}

#[test]
fn checker_ignores_the_rect_offset_and_keeps_device_phase() {
    // The cells are screen-fixed: a rect at a non-multiple of the cell size
    // still lands on the global lattice.
    let mut fb = Framebuffer::new(40, 40);
    paint_checker(&mut fb, 0, 0, 40, 40, 1.0, None);
    let mut fb2 = Framebuffer::new(40, 40);
    paint_checker(&mut fb2, 8, 8, 40, 40, 1.0, Some((40.0, 40.0)));
    let a = px(&fb, 10, 10);
    let b = px(&fb2, 10, 10);
    assert_eq!((a, b), (CHECKER_RGBA_A, CHECKER_RGBA_A));
    // (9,9) sits in cell (1,1): parity even, so A on both — not B.
    assert_eq!(px(&fb, 9, 9), CHECKER_RGBA_A);
    assert_eq!(px(&fb2, 9, 9), CHECKER_RGBA_A);
    // An odd-parity cell agrees across the offset too: (16,9) is cell (2,1).
    assert_eq!(px(&fb, 16, 9), CHECKER_RGBA_B);
    assert_eq!(px(&fb2, 16, 9), CHECKER_RGBA_B);
}

#[test]
fn checker_viewport_intersection_equals_the_full_rect_loop() {
    /// The naive full-rect loop the viewport shortcut must equal: walk the
    /// whole rect, clip each pixel to the buffer, keep the device phase.
    /// Pixels the rect does not cover stay transparent.
    fn brute(fb: &mut Framebuffer, rx: i32, ry: i32, rw: i32, rh: i32, dpr: f64) {
        let cell = (8.0 * dpr).round() as i64;
        let (w, h) = (fb.width as i32, fb.height as i32);
        for y in 0..h {
            for x in 0..w {
                let covered = x >= rx && x < rx + rw && y >= ry && y < ry + rh;
                if !covered {
                    continue;
                }
                let odd =
                    ((x as i64).div_euclid(cell) + (y as i64).div_euclid(cell)).rem_euclid(2) != 0;
                let c = if odd { CHECKER_RGBA_B } else { CHECKER_RGBA_A };
                let o = ((y as u32 * fb.width + x as u32) * 4) as usize;
                fb.rgba[o..o + 4].copy_from_slice(&c);
            }
        }
    }
    // Small viewport: the optimised painter must match the naive full loop,
    // including when the rect hangs off the left/top edge.
    for dpr in [1.0, 1.25, 1.5, 2.0] {
        for (rx, ry) in [(0i32, 0i32), (13, 4), (-8, -8)] {
            let mut a = Framebuffer::new(24, 20);
            brute(&mut a, rx, ry, 24, 20, dpr);
            let mut b = Framebuffer::new(24, 20);
            paint_checker(&mut b, rx, ry, 24, 20, dpr, Some((24.0, 20.0)));
            assert_eq!(
                a.rgba, b.rgba,
                "dpr {dpr} rect ({rx},{ry}) must match the brute-force lattice"
            );
        }
    }
}

#[test]
fn background_kinds() {
    let size = |bg: &str, dpr: f64| {
        let mut fb = Framebuffer::new(16, 16);
        paint_background(&mut fb, bg, 0, 0, 16, 16, dpr, Some((16.0, 16.0)));
        px(&fb, 0, 0)
    };
    // transparent / invalid → checker only
    assert_eq!(size("transparent", 1.0), CHECKER_RGBA_A);
    assert_eq!(size("not-a-colour", 1.0), CHECKER_RGBA_A);
    // a === 0 → checker only, even though the colour bytes parsed
    assert_eq!(size("#ff000000", 1.0), CHECKER_RGBA_A);
    // opaque → flat colour
    assert_eq!(size("#336699", 1.0), [0x33, 0x66, 0x99, 255]);
    // a === 255 explicit in the string behaves the same
    assert_eq!(size("#336699ff", 1.0), [0x33, 0x66, 0x99, 255]);
    // semi → checker behind a source-over wash
    assert_eq!(size("#33669980", 1.0), {
        let mut fb = Framebuffer::new(16, 16);
        paint_checker(&mut fb, 0, 0, 16, 16, 1.0, Some((16.0, 16.0)));
        fb.blend_rect(0, 0, 16, 16, [0x33, 0x66, 0x99, 128], 1.0);
        px(&fb, 0, 0)
    });
}

#[test]
fn background_rect_offscreen_paints_nothing() {
    let mut fb = Framebuffer::new(16, 16);
    paint_background(&mut fb, "#ff0000", 20, 0, 8, 8, 1.0, Some((16.0, 16.0)));
    assert!(fb.rgba.iter().all(|&b| b == 0));
}

#[test]
fn grid_mode_parses_the_setting_strings() {
    assert_eq!(GridMode::from("off"), GridMode::Off);
    assert_eq!(GridMode::from("unit"), GridMode::Unit);
    assert_eq!(GridMode::from("tile"), GridMode::Tile);
    assert_eq!(GridMode::from("pixel"), GridMode::Pixel);
    // The JS `_gridMode()` fallback for a missing/unreadable setting.
    assert_eq!(GridMode::from(""), GridMode::Off);
    assert_eq!(GridMode::from("Pixel"), GridMode::Off);
}
