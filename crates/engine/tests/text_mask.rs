//! W12 parity: `tools/text_mask_cases.json` (JS oracle) vs `raster::text_mask`.

use std::collections::HashMap;

use pixel_engine::raster::text_mask::{
    Glyph, GlyphShape, TextParams, decode_glyph_sets, text_mask,
};
use serde_json::Value;

fn cases() -> Value {
    serde_json::from_str(include_str!("../../../tools/text_mask_cases.json"))
        .expect("oracle JSON parses")
}

fn glyph_pack() -> Value {
    serde_json::from_str(include_str!("../../../tools/recipe/text_glyphs.json"))
        .expect("glyph pack parses")
}

fn shape_from(v: &Value) -> GlyphShape {
    let bytes = |a: &Value| {
        a.as_array()
            .expect("alpha array")
            .iter()
            .map(|n| n.as_u64().expect("byte") as u8)
            .collect()
    };
    GlyphShape {
        dx: v["dx"].as_i64().expect("dx") as i32,
        dy: v["dy"].as_i64().expect("dy") as i32,
        w: v["w"].as_u64().expect("w") as usize,
        h: v["h"].as_u64().expect("h") as usize,
        alpha: bytes(&v["alpha"]),
    }
}

fn glyphs_from(defs: &Value) -> HashMap<char, Glyph> {
    defs.as_object()
        .expect("glyph defs object")
        .iter()
        .map(|(k, v)| {
            let ch = k.chars().next().expect("single-char key");
            let g = Glyph {
                advance: v["advance"].as_f64().expect("advance"),
                dx: v["dx"].as_i64().expect("dx") as i32,
                dy: v["dy"].as_i64().expect("dy") as i32,
                w: v["w"].as_u64().expect("w") as usize,
                h: v["h"].as_u64().expect("h") as usize,
                alpha: v["alpha"]
                    .as_array()
                    .expect("alpha")
                    .iter()
                    .map(|n| n.as_u64().expect("byte") as u8)
                    .collect(),
                shaped: v["shaped"]
                    .as_object()
                    .expect("shaped")
                    .iter()
                    .map(|(sw, s)| (sw.parse::<u32>().expect("width key"), shape_from(s)))
                    .collect(),
            };
            (ch, g)
        })
        .collect()
}

fn params_of<'a>(c: &'a Value, glyphs: &'a HashMap<char, Glyph>) -> TextParams<'a> {
    TextParams {
        text: c["text"].as_str().expect("text"),
        glyphs,
        x: c["x"].as_i64().expect("x") as i32,
        y: c["y"].as_i64().expect("y") as i32,
        fill: c["fill"].as_u64().expect("fill") as u32,
        stroke: c["stroke"].as_u64().expect("stroke") as u32,
        stroke_width: c["strokeWidth"].as_i64().expect("sw") as i32,
        shape_stroke: c["shapeStroke"].as_bool().expect("shapeStroke"),
        w: c["w"].as_u64().expect("w") as usize,
        h: c["h"].as_u64().expect("h") as usize,
    }
}

fn want_data(c: &Value) -> Vec<u32> {
    c["data"]
        .as_array()
        .expect("data")
        .iter()
        .map(|n| n.as_u64().expect("u32") as u32)
        .collect()
}

fn assert_data(name: &str, got: &[u32], want: &[u32]) {
    assert_eq!(
        got.len(),
        want.len(),
        "{name}: length {} != {}",
        got.len(),
        want.len()
    );
    if got != want {
        let at = got.iter().zip(want).position(|(a, b)| a != b).unwrap_or(0);
        panic!(
            "{name}: first diff at {at} ({}x{}): got {:#010x}, want {:#010x}",
            0, 0, got[at], want[at]
        );
    }
}

#[test]
fn oracle_synthetic_cases_match_exactly() {
    let root = cases();
    let list = root["synthetic"].as_array().expect("synthetic array");
    assert_eq!(list.len(), 8, "oracle drift: synthetic case count");
    for c in list {
        let name = c["name"].as_str().expect("name");
        let glyphs = glyphs_from(&c["glyphs"]);
        let m = text_mask(&params_of(c, &glyphs)).expect("valid case");
        assert_eq!(
            (m.w, m.h),
            (
                c["w"].as_u64().unwrap() as usize,
                c["h"].as_u64().unwrap() as usize
            )
        );
        assert_data(name, &m.data, &want_data(c));
    }
}

#[test]
fn oracle_real_font_cases_match_exactly() {
    let root = cases();
    let fonts = decode_glyph_sets(&glyph_pack()).expect("pack decodes");
    let list = root["real"].as_array().expect("real array");
    assert_eq!(list.len(), 5, "oracle drift: real case count");
    for c in list {
        let name = c["name"].as_str().expect("name");
        let font = c["font"].as_str().expect("font");
        let glyphs = &fonts[font].glyphs;
        let m = text_mask(&params_of(c, glyphs)).expect("valid case");
        assert_data(name, &m.data, &want_data(c));
    }
}

#[test]
fn committed_pack_decodes_both_fonts() {
    let fonts = decode_glyph_sets(&glyph_pack()).expect("pack decodes");
    assert!(fonts.contains_key("default-42") && fonts.contains_key("monogram-240"));
    let a = &fonts["default-42"].glyphs[&'A'];
    assert_eq!((a.advance, a.dx, a.dy, a.w, a.h), (27.0, 1, 12, 26, 29));
    assert!(a.shaped.contains_key(&3));
    assert_eq!(fonts["default-42"].glyphs.len(), 1);
    let d = &fonts["monogram-240"].glyphs[&'d'];
    assert_eq!((d.advance, d.w, d.h), (90.0, 75, 105));
    assert_eq!(fonts["monogram-240"].glyphs.len(), 9);
}

#[test]
fn shaped_and_disk_branches_diverge() {
    let root = cases();
    let c = root["synthetic"]
        .as_array()
        .expect("synthetic")
        .iter()
        .find(|c| c["name"] == "shaped-c")
        .expect("shaped-c case");
    let glyphs = glyphs_from(&c["glyphs"]);
    let shaped = text_mask(&params_of(c, &glyphs)).expect("shaped").data;
    let mut disk_case = c.clone();
    disk_case["shapeStroke"] = Value::Bool(false);
    let disk = text_mask(&params_of(&disk_case, &glyphs))
        .expect("disk")
        .data;
    assert_ne!(
        shaped, disk,
        "baked stroke must differ from the disk spread"
    );
    assert!(shaped.iter().any(|&p| p != 0) && disk.iter().any(|&p| p != 0));
}

#[test]
fn transparent_stroke_skips_the_stroke_pass() {
    let root = cases();
    let c = root["synthetic"]
        .as_array()
        .expect("synthetic")
        .iter()
        .find(|c| c["name"] == "fill-only-zero-stroke")
        .expect("zero-stroke case");
    let glyphs = glyphs_from(&c["glyphs"]);
    let skipped = text_mask(&params_of(c, &glyphs)).expect("skipped").data;
    let mut fill_only = c.clone();
    fill_only["strokeWidth"] = Value::from(0);
    let plain = text_mask(&params_of(&fill_only, &glyphs))
        .expect("plain")
        .data;
    assert_eq!(skipped, plain);
}

#[test]
fn fully_outside_canvas_stays_transparent() {
    let root = cases();
    let c = root["synthetic"]
        .as_array()
        .expect("synthetic")
        .iter()
        .find(|c| c["name"] == "disk-ab")
        .expect("disk-ab case");
    let glyphs = glyphs_from(&c["glyphs"]);
    let mut far = c.clone();
    far["x"] = Value::from(-100);
    far["y"] = Value::from(-100);
    let m = text_mask(&params_of(&far, &glyphs)).expect("clipped");
    assert!(m.data.iter().all(|&p| p == 0));
}
