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

/// UI-5 maps each repainted chunk to one device-px box and uploads only that
/// region. The box has to be a *superset* of every pixel `blit_visible` reads
/// from the chunk — one pixel short leaves a stale pixel on screen forever.
#[test]
fn chunk_device_rect_covers_every_pixel_the_blit_reads_from_it() {
    use super::frame::{DisplayParams, canvas_device_rect, chunk_device_rect, draw_display_into};
    use crate::view::View;

    const W: u32 = 128;
    const H: u32 = 96;
    const CSS_W: f64 = 96.0;
    const CSS_H: f64 = 64.0;
    const A: [u8; 4] = [255, 0, 255, 255];

    let cases = [
        (View::new(1.0, 0.0, 0.0).expect("valid view"), 1.0),
        (View::new(2.5, -120.0, -30.0).expect("valid view"), 1.5),
        (View::new(0.5, 3.7, 1.3).expect("valid view"), 2.0),
    ];
    let mut total_painted = 0usize;
    for (view, dpr) in cases {
        let canvas = canvas_device_rect(&view, W, H, dpr);
        for cy in 0..H / 32 {
            for cx in 0..W / 32 {
                // Only this chunk carries a colour the background cannot spell.
                let mut comp = vec![0u8; (W * H * 4) as usize];
                for y in cy * 32..(cy + 1) * 32 {
                    for x in cx * 32..(cx + 1) * 32 {
                        let o = ((y * W + x) * 4) as usize;
                        comp[o..o + 4].copy_from_slice(&A);
                    }
                }
                let params = DisplayParams {
                    view,
                    canvas_w: W,
                    canvas_h: H,
                    css_w: CSS_W,
                    css_h: CSS_H,
                    dpr,
                    background: "#000000",
                    grid: GridMode::Off,
                    comp: Some(&comp),
                };
                let mut fb = Framebuffer::new(1, 1);
                draw_display_into(&params, &mut fb);
                let rect = chunk_device_rect(&view, W, H, dpr, cx, cy).expect("on canvas");
                assert!(
                    rect.0 >= canvas.0
                        && rect.1 >= canvas.1
                        && rect.0 + rect.2 <= canvas.0 + canvas.2
                        && rect.1 + rect.3 <= canvas.1 + canvas.3,
                    "chunk box {rect:?} escapes the canvas rect {canvas:?}"
                );
                for y in 0..fb.height {
                    for x in 0..fb.width {
                        if px(&fb, x, y) == A {
                            total_painted += 1;
                            assert!(
                                (x as i32) >= rect.0
                                    && (x as i32) < rect.0 + rect.2
                                    && (y as i32) >= rect.1
                                    && (y as i32) < rect.1 + rect.3,
                                "({x},{y}) of chunk ({cx},{cy}) lies outside {rect:?}"
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(total_painted > 0, "the fixture must actually paint pixels");
}

/// The run-based `blit_visible` reads one source column per *run* of destination
/// pixels. This pins the run boundaries against the per-pixel mapping the port
/// shipped with — including zoom-out, where a column owns no pixel at all and
/// the walk has to skip it.
#[test]
fn blit_runs_cover_exactly_the_pixels_the_per_pixel_mapping_gives() {
    use super::frame::ceil_div;

    // (sw, ddw) pairs: zoom in, fractional dpr scale, 1:1, and zoom out.
    let cases: [(i64, i64); 12] = [
        (512, 6144),
        (512, 768),
        (512, 512),
        (512, 341),
        (512, 256),
        (48, 61),
        (48, 143),
        (48, 48),
        (48, 33),
        (7, 90),
        (7, 7),
        (7, 3),
    ];
    let x0 = 3i64; // any source window start; the mapping only adds it on top
    let mut runs_seen = 0usize;
    for (sw, ddw) in cases {
        for ox in [-37i64, 0, 5, 61] {
            for rw in [1i64, 2, 40, 200] {
                let row_end = ox + rw;
                // The walk from `blit_visible`, emitting one entry per pixel.
                let mut got: Vec<(i64, i64)> = Vec::new();
                let mut px = ox;
                let mut k = (px - ox) * sw / ddw;
                while px < row_end {
                    let mut end = ox + ceil_div((k + 1) * ddw, sw);
                    while end <= px {
                        k += 1;
                        end = ox + ceil_div((k + 1) * ddw, sw);
                    }
                    let run_end = end.min(row_end);
                    for p in px..run_end {
                        got.push((p, x0 + k));
                    }
                    runs_seen += 1;
                    px = run_end;
                    k += 1;
                }
                // The per-pixel mapping, one division per pixel.
                let want: Vec<(i64, i64)> = (ox..row_end)
                    .map(|p| (p, x0 + (p - ox) * sw / ddw))
                    .collect();
                assert_eq!(
                    got, want,
                    "sw={sw} ddw={ddw} ox={ox} rw={rw}: runs disagree with per-pixel"
                );
            }
        }
    }
    assert!(runs_seen > 0, "the fixture must produce runs");
}

/// `blend_run` hoists the per-run factors out of `over_blend`'s "normal" path.
/// The arithmetic has to stay identical, byte for byte, or the blit stops
/// matching the JS at exactly the semi-transparent pixels it is meant for.
#[test]
fn blend_run_matches_over_blend() {
    use super::frame::blend_run;
    use crate::blend::over_blend;

    let dsts = [
        [0u8, 0, 0, 0],
        [255, 255, 255, 255],
        [12, 200, 77, 255],
        [3, 9, 240, 128],
        [200, 100, 50, 1],
        [1, 2, 3, 254],
        [77, 153, 231, 63],
    ];
    let srcs = [
        [0u8, 0, 0, 0],
        [1, 2, 3, 1],
        [255, 0, 128, 128],
        [10, 90, 200, 254],
        [7, 7, 7, 255],
        [250, 249, 248, 3],
        [0, 255, 0, 129],
    ];
    for dst in dsts {
        for src in srcs {
            let mut buf = [
                dst[0], dst[1], dst[2], dst[3], dst[0], dst[1], dst[2], dst[3],
            ];
            blend_run(&mut buf, src);
            let want = over_blend(dst, src, 1.0, "normal");
            assert_eq!(&buf[0..4], &want, "dst {dst:?} src {src:?}");
            assert_eq!(&buf[4..8], &want, "dst {dst:?} src {src:?}");
            // ... and one alpha sweep, where the f64 factors are least exact.
            for a in 0u8..=255 {
                let src = [src[0], src[1], src[2], a];
                let mut one = [dst[0], dst[1], dst[2], dst[3]];
                blend_run(&mut one, src);
                let want = over_blend(dst, src, 1.0, "normal");
                assert_eq!(one, want, "dst {dst:?} src {src:?}");
            }
        }
    }
}

/// `blend_rect` hoists the blend factors; the per-pixel formula has to stay
/// identical, including the two fast paths (`sa == 0`, fully opaque source).
#[test]
fn blend_rect_matches_the_per_pixel_formula() {
    use crate::blend::over_blend;

    let srcs = [
        [0u8, 0, 0, 0],
        [1, 2, 3, 1],
        [255, 0, 128, 128],
        [10, 90, 200, 254],
        [7, 7, 7, 255],
        [250, 249, 248, 3],
        [255, 255, 255, 89],
    ];
    let alphas = [0.0f64, 0.25, 0.35, 0.5, 0.6, 1.0, 1.5];
    // Every destination alpha class inside one buffer: 0, 128, 255, 1.
    let pattern = |i: usize| -> [u8; 4] {
        let v = (i as u8).wrapping_mul(37);
        [v, v ^ 0x5a, v.wrapping_add(91), [0, 128, 255, 1][i % 4]]
    };
    for src in srcs {
        for a in alphas {
            let mut fb = Framebuffer::new(4, 4);
            for (i, chunk) in fb.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                chunk.copy_from_slice(&pattern(i));
            }
            let before = fb.rgba.clone();
            fb.blend_rect(1, 1, 2, 2, src, a);
            for y in 0..4u32 {
                for x in 0..4u32 {
                    let o = ((y * fb.width + x) * 4) as usize;
                    let dst = [before[o], before[o + 1], before[o + 2], before[o + 3]];
                    let inside = (1..3).contains(&x) && (1..3).contains(&y);
                    let want = if inside {
                        over_blend(dst, src, a, "normal")
                    } else {
                        dst
                    };
                    assert_eq!(
                        px(&fb, x, y),
                        want,
                        "src {src:?} a {a} at ({x},{y}) over {dst:?}"
                    );
                }
            }
        }
    }
}

/// Byte-for-byte check of the run-based `blit_visible` against a per-pixel
/// oracle — the inner loop as it shipped before the 2026-10-18 drag-bottleneck
/// fix. The matrix spans zoom in, zoom out, fractional dpr and off-centre
/// canvases, which is where run boundaries and clipping interact.
#[test]
fn blit_visible_matches_the_per_pixel_oracle() {
    use super::frame::{blit_visible, canvas_device_rect};
    use super::framebuffer::{clip_rect, intersect_rects};
    use crate::blend::over_blend;
    use crate::view::{View, round_half_up};

    /// The pre-fix loop: one `sx` division per destination pixel.
    #[allow(clippy::too_many_arguments)]
    fn oracle(
        fb: &mut Framebuffer,
        comp: &[u8],
        view: &View,
        w: u32,
        h: u32,
        css_w: f64,
        css_h: f64,
        dpr: f64,
        rect: (i32, i32, i32, i32),
    ) {
        let z = view.zoom();
        let x0 = ((0.0 - view.offset_x()) / z).floor().max(0.0) as i64;
        let y0 = ((0.0 - view.offset_y()) / z).floor().max(0.0) as i64;
        let sw = ((w as f64).min(((css_w - view.offset_x()) / z).ceil()) as i64) - x0;
        let sh = ((h as f64).min(((css_h - view.offset_y()) / z).ceil()) as i64) - y0;
        if sw <= 0 || sh <= 0 {
            return;
        }
        let ddw = round_half_up(sw as f64 * z * dpr) as i64;
        let ddh = round_half_up(sh as f64 * z * dpr) as i64;
        if ddw <= 0 || ddh <= 0 {
            return;
        }
        let ox = round_half_up((view.offset_x() + x0 as f64 * z) * dpr) as i64;
        let oy = round_half_up((view.offset_y() + y0 as f64 * z) * dpr) as i64;
        let fb_rect = (0, 0, fb.width as i32, fb.height as i32);
        let Some(bounds) = intersect_rects(rect, fb_rect) else {
            return;
        };
        let Some((rx, ry, rw, rh)) =
            clip_rect(bounds, ox as i32, oy as i32, ddw as i32, ddh as i32)
        else {
            return;
        };
        let (rw, rh) = (rw as i64, rh as i64);
        let fb_w = fb.width as usize;
        for row in 0..rh {
            let py = ry as i64 + row;
            let sy = y0 + (py - oy) * sh / ddh;
            let srow = (sy * w as i64) as usize * 4;
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

    const W: u32 = 17;
    const H: u32 = 11;
    const CSS_W: f64 = 96.0;
    const CSS_H: f64 = 64.0;
    // All three alpha classes: opaque, semi-transparent, untouched.
    let mut comp = vec![0u8; (W * H * 4) as usize];
    for y in 0..H as usize {
        for x in 0..W as usize {
            let o = (y * W as usize + x) * 4;
            let v = (x * 7 + y * 13) as u8;
            let a = match (x + y) % 3 {
                0 => 255,
                1 => 128,
                _ => 0,
            };
            comp[o..o + 4].copy_from_slice(&[v, v.wrapping_mul(3).wrapping_add(11), v ^ 0x5a, a]);
        }
    }
    let views = [
        View::new(1.0, 0.0, 0.0).expect("valid view"),
        View::new(3.0, -5.0, -2.5).expect("valid view"),
        View::new(8.0, -60.0, -40.0).expect("valid view"),
        View::new(1.5, 7.3, 2.1).expect("valid view"),
        View::new(0.5, 3.7, 1.3).expect("valid view"),
        View::new(0.25, -11.0, 9.0).expect("valid view"),
    ];
    let mut compared = 0usize;
    for &view in &views {
        for dpr in [1.0f64, 1.25, 1.5, 2.0] {
            let mut a = Framebuffer::new(96, 64);
            for (i, chunk) in a.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let v = (i % 251) as u8;
                chunk.copy_from_slice(&[v, v ^ 0x33, v.wrapping_add(7), 255]);
            }
            let mut b = Framebuffer::new(96, 64);
            b.rgba.copy_from_slice(&a.rgba);
            let rect = canvas_device_rect(&view, W, H, dpr);
            blit_visible(&mut a, &comp, &view, W, H, CSS_W, CSS_H, dpr, rect);
            oracle(&mut b, &comp, &view, W, H, CSS_W, CSS_H, dpr, rect);
            assert_eq!(a.rgba, b.rgba, "view {view:?} dpr {dpr}");
            compared += 1;
        }
    }
    assert_eq!(compared, views.len() * 4);
}
