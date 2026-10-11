use crate::{
    blend::over,
    errors::EngineResult,
    io::base64_to_bytes,
    pixel::{pack_rgba_bytes, unpack_rgba},
    raster::PixelSink,
};
use serde_json::Value;

const GLYPH_DATA: &str = include_str!("../../../../tools/recipe/text_glyphs.json");

#[derive(Clone)]
struct Glyph {
    advance: f64,
    dx: f64,
    dy: f64,
    w: usize,
    h: usize,
    alpha: Vec<u8>,
    shaped: serde_json::Map<String, Value>,
}
fn glyph(v: &Value) -> EngineResult<Glyph> {
    Ok(Glyph {
        advance: v["advance"].as_f64().unwrap_or(0.),
        dx: v["dx"].as_f64().unwrap_or(0.),
        dy: v["dy"].as_f64().unwrap_or(0.),
        w: v["w"].as_u64().unwrap_or(0) as usize,
        h: v["h"].as_u64().unwrap_or(0) as usize,
        alpha: base64_to_bytes(v["alpha_b64"].as_str().unwrap_or_default())?,
        shaped: v["shaped"].as_object().cloned().unwrap_or_default(),
    })
}
fn stamp(data: &mut [u32], w: usize, h: usize, g: &Glyph, x: i32, y: i32, rgb: [u8; 3]) {
    for gy in 0..g.h {
        let ty = y + gy as i32;
        if ty < 0 || ty >= h as i32 {
            continue;
        }
        for gx in 0..g.w {
            let tx = x + gx as i32;
            if tx < 0 || tx >= w as i32 {
                continue;
            }
            let cov = g.alpha.get(gy * g.w + gx).copied().unwrap_or(0);
            if cov == 0 {
                continue;
            }
            let i = ty as usize * w + tx as usize;
            let rgba = over(unpack_rgba(data[i]), [rgb[0], rgb[1], rgb[2], cov], 1.);
            data[i] = pack_rgba_bytes(rgba[0], rgba[1], rgba[2], rgba[3]);
        }
    }
}
fn rounded(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}
pub fn render<S: PixelSink>(
    sink: &mut S,
    c: &Value,
    palette: &Value,
    w: usize,
    h: usize,
) -> EngineResult<()> {
    let text = c["text"].as_str().unwrap_or("");
    if text.is_empty() {
        return Ok(());
    }
    let all: Value = serde_json::from_str(GLYPH_DATA).expect("committed glyph pack is valid JSON");
    let font = c["font"].as_str().unwrap_or("monogram-240");
    let Some(glyphs) = all["fonts"][font]["glyphs"].as_object() else {
        return Ok(());
    };
    let mut placed = Vec::new();
    let mut pen = 0.0;
    for ch in text.chars() {
        let Some(v) = glyphs.get(&ch.to_string()) else {
            return Ok(());
        };
        let g = glyph(v)?;
        let x = rounded(pen + g.dx);
        let y = rounded(g.dy);
        placed.push((x, y, g.clone()));
        pen += g.advance;
    }
    let (pen_x, pen_y) = if let Some(pos) = c.get("pos").and_then(Value::as_array) {
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for (x, y, g) in &placed {
            min_x = min_x.min(*x);
            min_y = min_y.min(*y);
            max_x = max_x.max(*x + g.w as i32 - 1);
            max_y = max_y.max(*y + g.h as i32 - 1);
        }
        let (tw, th) = ((max_x - min_x + 1).max(0), (max_y - min_y + 1).max(0));
        (
            rounded(pos[0].as_f64().unwrap_or(0.) - (tw / 2) as f64),
            rounded(pos[1].as_f64().unwrap_or(0.) - (th / 2) as f64 - 2.),
        )
    } else {
        let p = c.get("pen").or_else(|| c.get("xy"));
        (
            rounded(p.map(|v| v[0].as_f64().unwrap_or(0.)).unwrap_or(0.)),
            rounded(p.map(|v| v[1].as_f64().unwrap_or(0.)).unwrap_or(0.)),
        )
    };
    let placed: Vec<_> = placed
        .into_iter()
        .map(|(x, y, g)| (x + pen_x, y + pen_y, g))
        .collect();
    let mut data = vec![0u32; w * h];
    let white = serde_json::json!("#FFFFFF");
    let fill_value = c.get("fill").or_else(|| c.get("color")).unwrap_or(&white);
    let fill = super::commands::color(fill_value, palette, 0);
    let stroke = c
        .get("stroke_fill")
        .or_else(|| c.get("stroke"))
        .map(|v| super::commands::color(v, palette, 0))
        .unwrap_or(fill);
    let fc = unpack_rgba(fill);
    let sc = unpack_rgba(stroke);
    let sw = c
        .get("stroke_width")
        .or_else(|| c.get("strokeWidth"))
        .or_else(|| c.get("width"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .clamp(0, 64) as i32;
    let shape_stroke = c["stroke_mode"].as_str().unwrap_or("shape") == "shape";
    if stroke != 0 && sw > 0 {
        for (x, y, g) in &placed {
            if shape_stroke && let Some(s) = g.shaped.get(&sw.to_string()) {
                let sg = glyph(s)?;
                stamp(
                    &mut data,
                    w,
                    h,
                    &sg,
                    *x - rounded(g.dx) + rounded(sg.dx),
                    *y - rounded(g.dy) + rounded(sg.dy),
                    [sc[0], sc[1], sc[2]],
                );
                continue;
            }
            for ox in -sw..=sw {
                for oy in -sw..=sw {
                    if ox * ox + oy * oy <= sw * sw {
                        stamp(&mut data, w, h, g, x + ox, y + oy, [sc[0], sc[1], sc[2]]);
                    }
                }
            }
        }
    }
    for (x, y, g) in &placed {
        stamp(&mut data, w, h, g, *x, *y, [fc[0], fc[1], fc[2]]);
    }
    for y in 0..h {
        for x in 0..w {
            let p = data[y * w + x];
            if p != 0 {
                sink.set(x as i32, y as i32, p);
            }
        }
    }
    Ok(())
}
