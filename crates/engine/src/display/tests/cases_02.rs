use super::*;

/// The run-based `blit_visible` reads one source column per *run* of destination
/// pixels. This pins the run boundaries against the per-pixel mapping the port
/// shipped with — including zoom-out, where a column owns no pixel at all and
/// the walk has to skip it.
#[test]
fn blit_runs_cover_exactly_the_pixels_the_per_pixel_mapping_gives() {
    use crate::display::frame::ceil_div;

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
    use crate::blend::over_blend;
    use crate::display::frame::blend_run;

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
    use crate::blend::over_blend;
    use crate::display::frame::{blit_visible, canvas_device_rect};
    use crate::display::framebuffer::{clip_rect, intersect_rects};
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
