//! Glyph-set decoding, ported from `src/features/text/glyph_set.js`.
//!
//! Fonts are never parsed here; glyphs arrive as committed bitmap JSON
//! (`tools/recipe/text_glyphs.json`), so decoding is base64 plus shape checks.

use std::collections::{BTreeMap, HashMap};

use crate::errors::{CODE_SCHEMA, DrawToolError, EngineResult};
use crate::io::base64_to_bytes;

/// One baked bitmap: canvas offset plus coverage bytes (`w*h`, 0 = skip).
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphShape {
    pub dx: i32,
    pub dy: i32,
    pub w: usize,
    pub h: usize,
    pub alpha: Vec<u8>,
}

/// A baked glyph plus its pre-stroked variants keyed by stroke width.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    pub advance: f64,
    pub dx: i32,
    pub dy: i32,
    pub w: usize,
    pub h: usize,
    pub alpha: Vec<u8>,
    pub shaped: BTreeMap<u32, GlyphShape>,
}

/// One decoded font: path/size plus glyphs keyed by character.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphSet {
    pub path: String,
    pub size: f64,
    pub glyphs: HashMap<char, Glyph>,
}

/// `Math.round`: halves go toward +inf (`f64::round` goes away from zero).
fn round_half_up(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}

fn i32_field(v: &serde_json::Value, key: &str) -> i32 {
    if let Some(i) = v.get(key).and_then(|n| n.as_i64()) {
        i as i32
    } else {
        round_half_up(v.get(key).and_then(|n| n.as_f64()).unwrap_or(0.0))
    }
}

fn usize_field(v: &serde_json::Value, key: &str) -> usize {
    v.get(key).and_then(|n| n.as_u64()).unwrap_or(0) as usize
}

fn decode_shape(v: &serde_json::Value) -> EngineResult<GlyphShape> {
    let w = usize_field(v, "w");
    let h = usize_field(v, "h");
    let b64 = v
        .get("alpha_b64")
        .and_then(|s| s.as_str())
        .unwrap_or_default();
    let alpha = base64_to_bytes(b64)?;
    if alpha.len() != w * h {
        return Err(DrawToolError::new(CODE_SCHEMA, "invalid glyph alpha size"));
    }
    Ok(GlyphShape {
        dx: i32_field(v, "dx"),
        dy: i32_field(v, "dy"),
        w,
        h,
        alpha,
    })
}

fn decode_glyph(v: &serde_json::Value) -> EngineResult<Glyph> {
    let shape = decode_shape(v)?;
    let mut shaped = BTreeMap::new();
    if let Some(map) = v.get("shaped").and_then(|s| s.as_object()) {
        for (k, s) in map {
            // Non-numeric keys could never match a width lookup; skip them.
            if let Ok(sw) = k.parse::<u32>() {
                shaped.insert(sw, decode_shape(s)?);
            }
        }
    }
    Ok(Glyph {
        advance: v.get("advance").and_then(|n| n.as_f64()).unwrap_or(0.0),
        dx: shape.dx,
        dy: shape.dy,
        w: shape.w,
        h: shape.h,
        alpha: shape.alpha,
        shaped,
    })
}

/// `decodeGlyphSets`: `raw.fonts[name]{path,size,glyphs{...}}` → fonts.
pub fn decode_glyph_sets(raw: &serde_json::Value) -> EngineResult<BTreeMap<String, GlyphSet>> {
    let mut fonts = BTreeMap::new();
    if let Some(map) = raw.get("fonts").and_then(|f| f.as_object()) {
        for (name, font) in map {
            let mut glyphs = HashMap::new();
            if let Some(list) = font.get("glyphs").and_then(|g| g.as_object()) {
                for (key, g) in list {
                    // Multi-char keys are unreachable via `chars()`; skip them.
                    let mut chars = key.chars();
                    if let (Some(c), None) = (chars.next(), chars.next()) {
                        glyphs.insert(c, decode_glyph(g)?);
                    }
                }
            }
            fonts.insert(
                name.clone(),
                GlyphSet {
                    path: font
                        .get("path")
                        .and_then(|p| p.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    size: font.get("size").and_then(|s| s.as_f64()).unwrap_or(0.0),
                    glyphs,
                },
            );
        }
    }
    Ok(fonts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn one_glyph(alpha_b64: &str) -> serde_json::Value {
        json!({"fonts": {"f": {"path": "p", "size": 42.0,
            "glyphs": {"A": {"advance": 27.0, "dx": 1, "dy": 12, "w": 2, "h": 1,
                "alpha_b64": alpha_b64, "shaped": {"3": {"dx": -2, "dy": 9, "w": 1, "h": 1,
                    "alpha_b64": "AA=="}}}}}}})
    }

    #[test]
    fn decodes_fields_and_shaped_keys() {
        // "AP8=" is [0,255]; "AA==" is [0]; sizes chosen to match.
        let fonts = decode_glyph_sets(&one_glyph("AP8=")).expect("valid");
        let g = &fonts["f"].glyphs[&'A'];
        assert_eq!((g.advance, g.dx, g.dy, g.w, g.h), (27.0, 1, 12, 2, 1));
        assert_eq!(g.alpha, vec![0, 255]);
        assert_eq!(fonts["f"].size, 42.0);
        let s = &g.shaped[&3];
        assert_eq!((s.dx, s.dy, s.w, s.h), (-2, 9, 1, 1));
    }

    #[test]
    fn alpha_size_mismatch_is_a_coded_error() {
        match decode_glyph_sets(&one_glyph("AA==")) {
            Err(e) => {
                assert_eq!(e.code, CODE_SCHEMA);
                assert_eq!(e.message, "invalid glyph alpha size");
            }
            Ok(_) => panic!("2x1 glyph with 1 alpha byte must fail"),
        }
    }

    #[test]
    fn missing_fonts_is_an_empty_map() {
        assert!(decode_glyph_sets(&json!({})).expect("valid").is_empty());
        assert!(decode_glyph_sets(&json!(null)).expect("valid").is_empty());
    }
}
