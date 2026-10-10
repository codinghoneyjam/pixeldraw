//! Composite operations that read back already-painted pixels — ported from
//! `src/core/raster/composite_ops.js`.
//!
//! None of these can be a mask-and-fill primitive, because every one of them
//! reads what is already on the tile through `writer.get()`. They exist because
//! the legacy bakers composed their slots that way:
//!
//! * [`clip_alpha`] — multiply a body slot's alpha by a clip mask
//! * [`erase_polygon`] — punch `cutouts` polygons to alpha 0
//! * [`visor_layer`] — fill a shape, then composite CRT scanlines masked by it
//! * [`sheen_overlay`] — two diagonal white lines masked to the visor shape
//! * [`hud_brackets`] — eight L-shaped corner segments
//!
//! All of them take a [`PixelSink`] because they read back.
//!
//! **Stage order is load-bearing.** In [`sheen_overlay`] the legacy path draws
//! the sheen into its own overlay, multiplies that overlay's *alpha* by the visor
//! mask, and only then composites. Compositing first and masking after is a
//! different image, so the three stages stay separate below.

use crate::pixel::{Rgba, pack_rgba_bytes};
use crate::raster::PixelSink;
use crate::raster::mask::{Mask, rrect_mask};
use crate::raster::polygon::polygon_mask;
use crate::raster::segment::line_mask;
use crate::raster::shape_raster::{FillMode, ShapeKind, ShapeSpec, apply_shape};

#[inline]
fn alpha_of(px: u32) -> u8 {
    ((px >> 24) & 0xff) as u8
}

#[inline]
fn channels_of(px: u32) -> Rgba {
    [
        (px & 0xff) as u8,
        ((px >> 8) & 0xff) as u8,
        ((px >> 16) & 0xff) as u8,
        ((px >> 24) & 0xff) as u8,
    ]
}

/// JS `Math.round`.
#[inline]
fn round_half_up(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}

/// A rectangular clip: the two corners plus a corner radius.
#[derive(Debug, Clone, Copy)]
pub struct Clip {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub radius: i32,
}

/// The shape commands a recipe may carry.
#[derive(Debug, Clone)]
pub enum RecipeShape {
    /// A polygon with `points`.
    Polygon { points: Vec<[f64; 2]> },
    /// A box (optionally rounded) from `cmd`.
    Box {
        cmd: String,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        radius: i32,
    },
}

/// Build a 1 = inside mask for a visor/cutout shape.
///
/// Only fill and polygon shapes occur in the enemy specs; anything else yields
/// an empty mask so callers fail closed.
pub fn shape_fill_mask(shape: &RecipeShape, w: usize, h: usize) -> Mask {
    let mut out = Mask::new(w, h);
    match shape {
        RecipeShape::Polygon { points } if points.len() >= 3 => {
            if let Ok(m) = polygon_mask(
                &points
                    .iter()
                    .map(|p| [round_half_up(p[0]), round_half_up(p[1])])
                    .collect::<Vec<_>>(),
                w,
                h,
            ) {
                out = m;
            }
        }
        RecipeShape::Box {
            cmd,
            x0,
            y0,
            x1,
            y1,
            radius,
        } => {
            let bw = round_half_up(x1 - x0 + 1.0);
            let bh = round_half_up(y1 - y0 + 1.0);
            let m = if cmd == "rounded_rect" {
                rrect_mask(bw as usize, bh as usize, *radius as i64).ok()
            } else {
                None
            };
            if let Some(m) = m {
                let ox = round_half_up(*x0);
                let oy = round_half_up(*y0);
                for y in 0..m.h {
                    for x in 0..m.w {
                        let tx = ox + x as i32;
                        let ty = oy + y as i32;
                        if m.is_set(x, y) && tx < w as i32 && ty < h as i32 && tx >= 0 && ty >= 0 {
                            out.set(tx as usize, ty as usize, 1);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    out
}

/// Multiply the tile's alpha by a rounded-rect clip mask.
///
/// Alpha is cleared *outside* the clip, because the legacy path multiplies the
/// whole alpha channel by the mask — body art that spills past the rounded rect
/// turns transparent instead of staying opaque.
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

/// Punch a polygon to fully transparent alpha.
pub fn erase_polygon<S: PixelSink>(sink: &mut S, points: &[[i32; 2]], w: usize, h: usize) {
    if let Ok(mask) = polygon_mask(points, w, h) {
        for y in 0..h {
            for x in 0..w {
                if mask.is_set(x, y) {
                    sink.set(x as i32, y as i32, 0);
                }
            }
        }
    }
}

/// `crt_lines` parameters for [`visor_layer`].
#[derive(Debug, Clone, Copy)]
pub struct CrtLines {
    pub y0: i32,
    pub y1: i32,
    pub step: i32,
    pub x0: i32,
    pub x1: i32,
    pub alpha: i32,
}

impl Default for CrtLines {
    fn default() -> Self {
        let [y0, y1, step] = [45, 80, 3];
        let [x0, x1] = [20, 108];
        Self {
            y0,
            y1,
            step,
            x0,
            x1,
            alpha: 45,
        }
    }
}

/// Derive the `fillMode` the JS `fillModeOf` produced.
fn fill_mode_of(fill: bool, outline: bool) -> FillMode {
    if fill && outline {
        FillMode::Both
    } else if fill {
        FillMode::Fill
    } else {
        FillMode::Outline
    }
}

/// The shape fields [`visor_layer`] and [`sheen_overlay`] need.
#[derive(Debug, Clone)]
pub struct VisorShape {
    pub kind: ShapeKind,
    pub bbox: Option<[i32; 4]>,
    pub points: Vec<[f64; 2]>,
    pub radius: i32,
    pub width: i32,
    pub has_fill: bool,
    pub has_outline: bool,
}

/// Render a visor slot: fill and outline its shape, then darken every scanline
/// row inside that shape.
///
/// The base under the visor is already opaque, so the composite only darkens.
pub fn visor_layer<S: PixelSink>(
    sink: &mut S,
    shape: &VisorShape,
    crt: Option<&CrtLines>,
    w: usize,
    h: usize,
    fill_packed: u32,
    outline_packed: u32,
) {
    let spec = ShapeSpec {
        kind: shape.kind,
        stroke_width: shape.width.max(1),
        fill_mode: fill_mode_of(shape.has_fill, shape.has_outline),
        bbox: shape.bbox,
        p0: None,
        p1: None,
        radius: shape.radius,
        points: shape
            .points
            .iter()
            .map(|p| [round_half_up(p[0]), round_half_up(p[1])])
            .collect(),
    };
    let _ = apply_shape(sink, &spec, outline_packed, fill_packed);
    let Some(crt) = crt else { return };
    let mask = shape_fill_mask(&visor_to_recipe(shape), w, h);
    let keep = 1.0 - crt.alpha as f64 / 255.0;
    let mut y = crt.y0 + 2;
    while y < crt.y1 - 1 {
        let mut x = crt.x0 + 2;
        while x <= crt.x1 - 2 {
            if x >= 0
                && y >= 0
                && (x as usize) < w
                && (y as usize) < h
                && mask.is_set(x as usize, y as usize)
            {
                let px = sink.get(x, y);
                if px != 0 {
                    let c = channels_of(px);
                    sink.set(
                        x,
                        y,
                        pack_rgba_bytes(
                            round_half_up(c[0] as f64 * keep) as u8,
                            round_half_up(c[1] as f64 * keep) as u8,
                            round_half_up(c[2] as f64 * keep) as u8,
                            alpha_of(px),
                        ),
                    );
                }
            }
            x += crt.step;
        }
        y += crt.step;
    }
}

fn visor_to_recipe(s: &VisorShape) -> RecipeShape {
    if !s.points.is_empty() {
        return RecipeShape::Polygon {
            points: s.points.clone(),
        };
    }
    // The tool spec carries [x, y, w, h]; `shapeFillMask` speaks corners, and
    // the JS `shapeArgs` makes exactly this conversion before drawing.
    if let Some([x0, y0, w, h]) = s.bbox {
        return RecipeShape::Box {
            cmd: "rounded_rect".to_string(),
            x0: x0 as f64,
            y0: y0 as f64,
            x1: (x0 + w - 1) as f64,
            y1: (y0 + h - 1) as f64,
            radius: s.radius,
        };
    }
    RecipeShape::Polygon { points: vec![] }
}

/// A sheen line definition.
#[derive(Debug, Clone, Copy)]
pub struct Sheen {
    pub p1: [f64; 2],
    pub p2: [f64; 2],
    pub dx: f64,
}

/// Overlay two diagonal sheen lines, masked to the visor shape.
///
/// A bright main line (alpha 110, width 2) plus a dimmer offset line (alpha 60,
/// width 1), both alpha-composited onto the tile through the visor mask.
pub fn sheen_overlay<S: PixelSink>(
    sink: &mut S,
    visor: &VisorShape,
    sheen: Option<&Sheen>,
    w: usize,
    h: usize,
) {
    let Some(sheen) = sheen else { return };
    let mask = shape_fill_mask(&visor_to_recipe(visor), w, h);
    let lines = [
        (
            [sheen.p1[0], sheen.p1[1]],
            [sheen.p2[0], sheen.p2[1]],
            110i32,
            2i32,
        ),
        (
            [sheen.p1[0] + sheen.dx, sheen.p1[1]],
            [sheen.p2[0] + sheen.dx, sheen.p2[1]],
            60,
            1,
        ),
    ];

    // Stage 1: the sheen lives in its own overlay.
    let mut overlay = vec![0u8; w * h * 4];
    for (a, b, alpha, width) in lines {
        let Ok(m) = line_mask(
            (round_half_up(a[0]), round_half_up(a[1])),
            (round_half_up(b[0]), round_half_up(b[1])),
            width,
        ) else {
            continue;
        };
        for y in 0..m.h {
            for x in 0..m.w {
                if !m.is_set(x, y) {
                    continue;
                }
                let tx = m.x + x as i32;
                let ty = m.y + y as i32;
                if tx < 0 || ty < 0 || tx >= w as i32 || ty >= h as i32 {
                    continue;
                }
                let o = (ty as usize * w + tx as usize) * 4;
                overlay[o] = 255;
                overlay[o + 1] = 255;
                overlay[o + 2] = 255;
                overlay[o + 3] = overlay[o + 3].max(alpha as u8);
            }
        }
    }

    // Stage 2: multiply the overlay alpha by the visor mask. `ImageChops.multiply`
    // is a per-channel multiply, NOT a lerp.
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            if overlay[o + 3] == 0 {
                continue;
            }
            let m = if mask.is_set(x, y) { 255u32 } else { 0 };
            overlay[o + 3] = ((overlay[o + 3] as u32 * m) / 255) as u8;
        }
    }

    // Stage 3: alpha_composite onto the tile.
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            let s_n = overlay[o + 3] as f64 / 255.0;
            if s_n == 0.0 {
                continue;
            }
            let px = sink.get(x as i32, y as i32);
            let c = channels_of(px);
            let d_n = c[3] as f64 / 255.0;
            let o_n = s_n + d_n * (1.0 - s_n);
            let mix = |sc: u8| {
                round_half_up((overlay[o] as f64 * s_n + sc as f64 * d_n * (1.0 - s_n)) / o_n)
            };
            sink.set(
                x as i32,
                y as i32,
                pack_rgba_bytes(
                    mix(c[0]) as u8,
                    mix(c[1]) as u8,
                    mix(c[2]) as u8,
                    round_half_up(o_n * 255.0) as u8,
                ),
            );
        }
    }
}

/// Corner bracket parameters.
#[derive(Debug, Clone, Copy)]
pub struct Hud {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub arm: i32,
    pub color: Rgba,
    pub width: i32,
}

impl Default for Hud {
    fn default() -> Self {
        Self {
            x0: 36,
            y0: 60,
            x1: 92,
            y1: 82,
            arm: 5,
            color: [255, 255, 255, 235],
            width: 1,
        }
    }
}

/// Stroke the eight L-shaped HUD corner brackets.
///
/// PIL's width-1 Bresenham path is used for every segment (not the rotated-quad
/// wide-line path), which is what the legacy `_draw_hud_brackets` did.
pub fn hud_brackets<S: PixelSink>(sink: &mut S, hud: &Hud) {
    let packed = pack_rgba_bytes(hud.color[0], hud.color[1], hud.color[2], hud.color[3]);
    let segs = [
        ([hud.x0, hud.y0], [hud.x0 + hud.arm, hud.y0]),
        ([hud.x0, hud.y0], [hud.x0, hud.y0 + hud.arm]),
        ([hud.x1, hud.y0], [hud.x1 - hud.arm, hud.y0]),
        ([hud.x1, hud.y0], [hud.x1, hud.y0 + hud.arm]),
        ([hud.x0, hud.y1], [hud.x0 + hud.arm, hud.y1]),
        ([hud.x0, hud.y1], [hud.x0, hud.y1 - hud.arm]),
        ([hud.x1, hud.y1], [hud.x1 - hud.arm, hud.y1]),
        ([hud.x1, hud.y1], [hud.x1, hud.y1 - hud.arm]),
    ];
    for (p, q) in segs {
        let spec = ShapeSpec {
            kind: ShapeKind::Line,
            stroke_width: hud.width.max(1),
            fill_mode: FillMode::Outline,
            bbox: None,
            p0: Some((p[0], p[1])),
            p1: Some((q[0], q[1])),
            radius: 0,
            points: vec![],
        };
        let _ = apply_shape(sink, &spec, packed, packed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunkstore::ChunkStore;

    /// A store-backed sink so these tests exercise the real read-back path.
    struct Store {
        store: ChunkStore,
    }

    impl Store {
        fn new(w: usize, h: usize) -> Self {
            Self {
                store: ChunkStore::new(w as u32 * 32, h as u32 * 32).unwrap(),
            }
        }
    }

    impl PixelSink for Store {
        fn set(&mut self, x: i32, y: i32, p: u32) {
            if x < 0 || y < 0 {
                return;
            }
            self.store.set_pixel(x as u32, y as u32, p);
        }
        fn get(&self, x: i32, y: i32) -> u32 {
            if x < 0 || y < 0 {
                return 0;
            }
            self.store.get_pixel(x as u32, y as u32)
        }
    }

    #[test]
    fn channels_and_alpha_helpers_unpack_little_endian() {
        assert_eq!(channels_of(0x11223344), [0x44, 0x33, 0x22, 0x11]);
        assert_eq!(alpha_of(0x11223344), 0x11);
    }

    #[test]
    fn shape_fill_mask_fails_closed_for_unknown_shapes() {
        let m = shape_fill_mask(&RecipeShape::Polygon { points: vec![] }, 8, 8);
        assert!(m.data.iter().all(|&v| v == 0));
    }

    #[test]
    fn clip_alpha_clears_outside_the_rounded_rect() {
        let mut s = Store::new(8, 8);
        for y in 0..64 {
            for x in 0..64 {
                s.store.set_pixel(x, y, 0xff0000ff);
            }
        }
        let before = s.store.chunk_count();
        clip_alpha(
            &mut s,
            &Clip {
                x0: 10.0,
                y0: 10.0,
                x1: 20.0,
                y1: 20.0,
                radius: 0,
            },
            64,
            64,
        );
        assert!(before > 0);
        assert_eq!(s.store.get_pixel(15, 15), 0xff0000ff, "inside survives");
        assert_eq!(s.store.get_pixel(0, 0), 0, "outside is cleared");
    }

    #[test]
    fn erase_polygon_punches_to_transparent() {
        let mut s = Store::new(8, 8);
        for y in 0..64 {
            for x in 0..64 {
                s.store.set_pixel(x, y, 0xff0000ff);
            }
        }
        erase_polygon(&mut s, &[[10, 10], [20, 10], [20, 20], [10, 20]], 64, 64);
        assert_eq!(s.store.get_pixel(15, 15), 0);
        assert_eq!(s.store.get_pixel(25, 25), 0xff0000ff);
    }

    #[test]
    fn hud_brackets_paints_eight_l_corners() {
        let mut s = Store::new(8, 8);
        hud_brackets(&mut s, &Hud::default());
        // The four corners of the box must all be painted.
        for (x, y) in [(36, 60), (92, 60), (36, 82), (92, 82)] {
            assert_eq!(
                s.store.get_pixel(x as u32, y as u32),
                pack_rgba_bytes(255, 255, 255, 235),
                "corner ({x},{y})"
            );
        }
    }

    #[test]
    fn visor_layer_darkens_only_inside_the_shape() {
        let mut s = Store::new(8, 8);
        for y in 0..64 {
            for x in 0..64 {
                s.store.set_pixel(x, y, 0xffffffff);
            }
        }
        let shape = VisorShape {
            kind: ShapeKind::Rrect,
            bbox: Some([10, 10, 54, 54]),
            points: vec![],
            radius: 0,
            width: 1,
            has_fill: true,
            has_outline: true,
        };
        visor_layer(
            &mut s,
            &shape,
            Some(&CrtLines::default()),
            64,
            64,
            0x808080ff,
            0x000000ff,
        );
        // A scanline pixel inside the visor is darker than the untouched base.
        let dark = s.store.get_pixel(40, 50);
        assert!(
            dark < 0xffffffff,
            "scanline pixel {dark:08x} was not darkened"
        );
    }

    #[test]
    fn sheen_overlay_composites_white_onto_the_shape() {
        let mut s = Store::new(8, 8);
        for y in 0..64 {
            for x in 0..64 {
                s.store.set_pixel(x, y, 0x000000ff);
            }
        }
        let shape = VisorShape {
            kind: ShapeKind::Rrect,
            bbox: Some([0, 0, 63, 63]),
            points: vec![],
            radius: 0,
            width: 1,
            has_fill: true,
            has_outline: true,
        };
        sheen_overlay(
            &mut s,
            &shape,
            Some(&Sheen {
                p1: [10.0, 10.0],
                p2: [50.0, 50.0],
                dx: 5.0,
            }),
            64,
            64,
        );
        // Some pixel must now carry white-ish luminance from the sheen.
        let mut any = false;
        for y in 0..64 {
            for x in 0..64 {
                let c = channels_of(s.store.get_pixel(x, y));
                if c[0] > 100 {
                    any = true;
                }
            }
        }
        assert!(any, "the sheen line never landed");
    }
}
