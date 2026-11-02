//! Pending-box state and the packed-mask helpers shared by the tool and API.

use std::collections::BTreeMap;
use std::sync::Arc;

use pixel_engine::pixel::{pack_rgba, parse_hex};
use pixel_engine::raster::gradient::GradientMask;
use pixel_engine::raster::text_mask::{GlyphSet, decode_glyph_sets};

use crate::features::shape::Pt;
use crate::tools::overlay::OverlayOp;

// Same committed pack the recipe renderer reads (`recipe/text.rs:10`);
// that second reader exists, so this file must stay in Phase 6.
const GLYPH_JSON: &str = include_str!("../../../../../web/tools/recipe/text_glyphs.json");

/// Decode the committed pack; a corrupt pack yields an empty map so every
/// font lookup warns `TEXT_FONT_MISSING` instead of panicking.
pub fn load_glyph_sets() -> BTreeMap<String, GlyphSet> {
    let raw: serde_json::Value = serde_json::from_str(GLYPH_JSON).unwrap_or_default();
    decode_glyph_sets(&raw).unwrap_or_default()
}

pub(super) fn pack_of(css: &str) -> u32 {
    match parse_hex(css) {
        Some(c) => pack_rgba(c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64),
        None => pack_rgba(0.0, 0.0, 0.0, 255.0),
    }
}

#[derive(Debug, Clone)]
pub(super) struct PendingText {
    pub(super) layer_id: String,
    pub(super) text: String,
    pub(super) font: String,
    pub(super) stroke_width: i32,
    pub(super) anchor: Pt,
    pub(super) fill: u32,
    pub(super) stroke: u32,
    pub(super) bbox: [i32; 4],
    pub(super) canvas: (u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Idle,
    Pending,
    Moving,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PreviewKey {
    pub(super) text: String,
    pub(super) font: String,
    pub(super) stroke_width: i32,
    pub(super) anchor: Pt,
    pub(super) fill: u32,
    pub(super) stroke: u32,
    pub(super) bbox: [i32; 4],
}

/// First/last painted cell in a packed mask.
pub(super) fn bbox_of_mask(mask: &GradientMask) -> Option<[i32; 4]> {
    let (mut x0, mut y0) = (i32::MAX, i32::MAX);
    let (mut x1, mut y1) = (i32::MIN, i32::MIN);
    for y in 0..mask.h {
        for x in 0..mask.w {
            if mask.data[y * mask.w + x] != 0 {
                x0 = x0.min(x as i32);
                y0 = y0.min(y as i32);
                x1 = x1.max(x as i32);
                y1 = y1.max(y as i32);
            }
        }
    }
    (x0 <= x1).then(|| [x0, y0, x1 - x0 + 1, y1 - y0 + 1])
}

pub(super) fn crop_preview(mask: &GradientMask, bbox: [i32; 4]) -> Option<OverlayOp> {
    let [x, y, w, h] = bbox;
    let mut data = vec![0u32; (w * h) as usize];
    let mut any = false;
    for yy in 0..h {
        let sy = y + yy;
        if sy < 0 || sy >= mask.h as i32 {
            continue;
        }
        for xx in 0..w {
            let sx = x + xx;
            if sx < 0 || sx >= mask.w as i32 {
                continue;
            }
            let packed = mask.data[sy as usize * mask.w + sx as usize];
            if packed != 0 {
                any = true;
                data[yy as usize * w as usize + xx as usize] = packed;
            }
        }
    }
    any.then(|| OverlayOp::MaskBlit {
        x,
        y,
        w: w as u32,
        h: h as u32,
        data: Arc::from(data.into_boxed_slice()),
    })
}
