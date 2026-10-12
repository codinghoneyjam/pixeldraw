//! `textMask`, ported from `src/core/raster/text_mask.js`.
//!
//! The stroke pass is either one baked (`shaped`) stamp or a brute-force disk
//! of glyph stamps; the fill pass always stamps each glyph once at its pen.

use std::collections::HashMap;

use super::glyph::Glyph;
use super::place::place_text;
use crate::blend::over;
use crate::errors::EngineResult;
use crate::pixel::{pack_rgba_bytes, unpack_rgba};
use crate::raster::gradient::GradientMask;
use crate::raster::mask::assert_mask_size;

/// `textMask` inputs; `shape_stroke` is `strokeMode === 'shape'`.
pub struct TextParams<'a> {
    pub text: &'a str,
    pub glyphs: &'a HashMap<char, Glyph>,
    pub x: i32,
    pub y: i32,
    pub fill: u32,
    pub stroke: u32,
    pub stroke_width: i32,
    pub shape_stroke: bool,
    pub w: usize,
    pub h: usize,
}

#[allow(clippy::too_many_arguments)]
fn stamp_glyph(
    data: &mut [u32],
    w: usize,
    h: usize,
    alpha: &[u8],
    gw: usize,
    gh: usize,
    at_x: i32,
    at_y: i32,
    rgb: [u8; 3],
) {
    for gy in 0..gh {
        let y = at_y + gy as i32;
        if y < 0 || y >= h as i32 {
            continue;
        }
        for gx in 0..gw {
            let cov = alpha[gy * gw + gx];
            if cov == 0 {
                continue;
            }
            let x = at_x + gx as i32;
            if x < 0 || x >= w as i32 {
                continue;
            }
            let i = y as usize * w + x as usize;
            let blended = over(unpack_rgba(data[i]), [rgb[0], rgb[1], rgb[2], cov], 1.0);
            data[i] = pack_rgba_bytes(blended[0], blended[1], blended[2], blended[3]);
        }
    }
}

/// Rasterize `text` to packed RGBA (`GradientMask` shape: `w*h` u32s).
pub fn text_mask(p: &TextParams) -> EngineResult<GradientMask> {
    assert_mask_size(p.w, p.h)?;
    let mut out = GradientMask {
        w: p.w,
        h: p.h,
        data: vec![0u32; p.w * p.h],
    };
    let placed = place_text(p.glyphs, p.text, p.x, p.y)?;
    let fill_rgb = {
        let c = unpack_rgba(p.fill);
        [c[0], c[1], c[2]]
    };
    let stroke_rgb = {
        let c = unpack_rgba(p.stroke);
        [c[0], c[1], c[2]]
    };
    // Packed 0 is transparent black; the JS `stroke !== 0` guard skips it.
    // (JS `stroke ?? fill` has no Rust form: `stroke` is not optional.)
    if p.stroke != 0 && p.stroke_width > 0 {
        let sw = p.stroke_width;
        for plc in &placed {
            let g = plc.glyph;
            if p.shape_stroke
                && let Some(shaped) = g.shaped.get(&(sw as u32))
            {
                stamp_glyph(
                    &mut out.data,
                    p.w,
                    p.h,
                    &shaped.alpha,
                    shaped.w,
                    shaped.h,
                    plc.x - g.dx + shaped.dx,
                    plc.y - g.dy + shaped.dy,
                    stroke_rgb,
                );
                continue;
            }
            // i64 compare: the iteration space matches the JS for sane widths.
            let r2 = (sw as i64) * (sw as i64);
            for ox in -sw..=sw {
                for oy in -sw..=sw {
                    if (ox as i64) * (ox as i64) + (oy as i64) * (oy as i64) <= r2 {
                        stamp_glyph(
                            &mut out.data,
                            p.w,
                            p.h,
                            &g.alpha,
                            g.w,
                            g.h,
                            plc.x + ox,
                            plc.y + oy,
                            stroke_rgb,
                        );
                    }
                }
            }
        }
    }
    for plc in &placed {
        let g = plc.glyph;
        stamp_glyph(
            &mut out.data,
            p.w,
            p.h,
            &g.alpha,
            g.w,
            g.h,
            plc.x,
            plc.y,
            fill_rgb,
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_px_map() -> HashMap<char, Glyph> {
        let mut map = HashMap::new();
        map.insert(
            'A',
            Glyph {
                advance: 1.0,
                dx: 0,
                dy: 0,
                w: 1,
                h: 1,
                alpha: vec![255],
                shaped: Default::default(),
            },
        );
        map
    }

    #[test]
    fn single_coverage_pixel_packs_fill() {
        let map = one_px_map();
        let p = TextParams {
            text: "A",
            glyphs: &map,
            x: 0,
            y: 0,
            fill: 0xff0000ff,
            stroke: 0,
            stroke_width: 0,
            shape_stroke: true,
            w: 1,
            h: 1,
        };
        let m = text_mask(&p).expect("valid");
        assert_eq!(m.data, vec![0xff0000ff]);
    }

    #[test]
    fn size_guard_rejects_zero() {
        let map = one_px_map();
        let p = TextParams {
            text: "A",
            glyphs: &map,
            x: 0,
            y: 0,
            fill: 0xff0000ff,
            stroke: 0,
            stroke_width: 0,
            shape_stroke: true,
            w: 0,
            h: 1,
        };
        assert!(text_mask(&p).is_err());
    }
}
