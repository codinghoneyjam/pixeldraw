//! Integer-pen glyph placement, ported from `placeText`.

use std::collections::HashMap;

use super::glyph::Glyph;
use crate::errors::{CODE_UNSUPPORTED_LAYER_TYPE, DrawToolError, EngineResult};

/// One laid-out character: canvas origin plus its baked glyph.
#[derive(Debug, Clone)]
pub struct PlacedGlyph<'a> {
    pub ch: char,
    pub x: i32,
    pub y: i32,
    pub glyph: &'a Glyph,
}

/// Lay `text` out from (`pen_x`, `pen_y`); unknown chars raise like the JS.
///
/// The missing-glyph message quotes with JSON string syntax, matching the JS
/// `` `glyph missing for ${JSON.stringify(ch)}` `` exactly.
pub fn place_text<'a>(
    glyphs: &'a HashMap<char, Glyph>,
    text: &str,
    pen_x: i32,
    pen_y: i32,
) -> EngineResult<Vec<PlacedGlyph<'a>>> {
    let mut out = Vec::new();
    let mut pen = pen_x as f64;
    for ch in text.chars() {
        let Some(g) = glyphs.get(&ch) else {
            let quoted = serde_json::to_string(&ch.to_string()).expect("char serializes");
            return Err(DrawToolError::new(
                CODE_UNSUPPORTED_LAYER_TYPE,
                format!("glyph missing for {quoted}"),
            ));
        };
        // `Math.round` is `(v + 0.5).floor()`; the cast truncates the floor.
        out.push(PlacedGlyph {
            ch,
            x: (pen + g.dx as f64 + 0.5).floor() as i32,
            y: (pen_y as f64 + g.dy as f64 + 0.5).floor() as i32,
            glyph: g,
        });
        pen += g.advance;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn glyph(advance: f64, dx: i32, dy: i32) -> Glyph {
        Glyph {
            advance,
            dx,
            dy,
            w: 0,
            h: 0,
            alpha: vec![],
            shaped: BTreeMap::new(),
        }
    }

    #[test]
    fn halves_round_toward_positive_infinity() {
        let map = HashMap::from([('a', glyph(1.5, 0, 0)), ('n', glyph(1.5, 1, 1))]);
        let ps = place_text(&map, "aan", -2, -1).expect("known chars");
        assert_eq!(ps[0].x, -2); // floor(-2 + 0.5)
        assert_eq!(ps[1].x, 0); // floor(-0.5 + 0.5): Math.round(-0.5) is -0
        assert_eq!(ps[2].x, 2); // floor(1.0 + 1 + 0.5)
        assert_eq!(ps[0].y, -1); // floor(-1 + 0.5)
        assert_eq!(ps[2].y, 0); // floor(-1 + 1 + 0.5)
        assert_eq!(ps[2].ch, 'n');
    }

    #[test]
    fn missing_glyph_quotes_like_json_stringify() {
        let map = HashMap::from([('A', glyph(1.0, 0, 0))]);
        match place_text(&map, "A\"", 0, 0) {
            Err(e) => {
                assert_eq!(e.code, CODE_UNSUPPORTED_LAYER_TYPE);
                assert_eq!(e.message, "glyph missing for \"\\\"\"");
            }
            Ok(_) => panic!("unknown char must fail"),
        }
    }
}
