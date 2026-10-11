//! `visorLayer` — fill and outline a visor shape, then darken CRT scanlines.
//!
//! The base under the visor is already opaque, so the composite only darkens.
//! `shapeFillMask` is the visor-side gate for both this and `sheen`; the tool
//! spec spells its bbox `[x, y, w, h]` while the mask speaks corners, and the
//! JS `shapeArgs` makes exactly that conversion before drawing.

use super::common::{alpha_of, channels_of, round_half_up};
use crate::pixel::pack_rgba_bytes;
use crate::raster::PixelSink;
use crate::raster::mask::{Mask, rrect_mask};
use crate::raster::polygon::polygon_mask;
use crate::raster::shape_raster::{FillMode, ShapeKind, ShapeSpec, apply_shape};

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

/// The shape fields `visor_layer` and `sheen_overlay` need.
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

/// The tool spec carries `[x, y, w, h]`; `shape_fill_mask` speaks corners.
pub(crate) fn visor_to_recipe(s: &VisorShape) -> RecipeShape {
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

/// Render a visor slot: fill and outline its shape, then darken every scanline
/// row inside that shape.
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
