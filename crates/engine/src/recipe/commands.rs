mod advanced;

use crate::{
    errors::{DrawToolError, EngineResult},
    pixel::{pack_rgba_bytes, parse_hex, unpack_rgba},
    raster::{
        FillMode, PixelSink, ShapeKind, ShapeSpec, apply_shape, arc_mask, chord_fill_mask,
        chord_outline_mask,
    },
};
use serde_json::Value;

fn jsround(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}
fn num(v: &Value, fallback: f64) -> f64 {
    v.as_f64().unwrap_or(fallback)
}

pub(crate) fn color(v: &Value, palette: &Value, depth: usize) -> u32 {
    if depth > 8 || v.is_null() {
        return 0;
    }
    if let Some(a) = v.as_array() {
        if a.len() < 3 {
            return 0;
        }
        let byte = |i: usize, fallback: f64| {
            num(a.get(i).unwrap_or(&Value::Null), fallback)
                .trunc()
                .clamp(0., 255.) as u8
        };
        return pack_rgba_bytes(byte(0, 0.), byte(1, 0.), byte(2, 0.), byte(3, 255.));
    }
    let Some(s) = v.as_str() else {
        return 0;
    };
    if let Some(c) = parse_hex(s) {
        return pack_rgba_bytes(c[0], c[1], c[2], c[3]);
    }
    let key = s.strip_prefix('$').unwrap_or(s);
    let dollar_key = format!("${key}");
    // The JS resolver derives these tokens from body_color before looking up
    // similarly named palette entries; the order is observable in layer parity.
    if key == "body_outline" {
        return 0xff000000;
    }
    if ["body_dark", "body_deep", "body_light", "body_bright"].contains(&key) {
        let base = palette
            .get("body_color")
            .or_else(|| palette.get("body_color_hex"))
            .map(|v| color(v, palette, depth + 1))
            .unwrap_or(0xff808080);
        let c = unpack_rgba(base);
        let f = match key {
            "body_dark" => 0.6,
            "body_deep" => 0.22,
            _ => 1.25,
        };
        return pack_rgba_bytes(
            (c[0] as f64 * f).trunc().clamp(0., 255.) as u8,
            (c[1] as f64 * f).trunc().clamp(0., 255.) as u8,
            (c[2] as f64 * f).trunc().clamp(0., 255.) as u8,
            c[3],
        );
    }
    if let Some(mapped) = palette
        .get(key)
        .or_else(|| palette.get(dollar_key.as_str()))
    {
        return color(mapped, palette, depth + 1);
    }
    if s.starts_with('$') { 0xffffffff } else { 0 }
}

fn points(v: &Value) -> Vec<[i32; 2]> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            if let Some(a) = p.as_array() {
                Some([jsround(num(a.first()?, 0.)), jsround(num(a.get(1)?, 0.))])
            } else {
                Some([jsround(num(&p["x"], 0.)), jsround(num(&p["y"], 0.))])
            }
        })
        .collect()
}
fn boxed(v: &Value) -> Option<[i32; 4]> {
    let a = v[0].as_array()?;
    let b = v[1].as_array()?;
    let (x, y) = (num(a.first()?, 0.), num(a.get(1)?, 0.));
    Some([
        jsround(x),
        jsround(y),
        jsround(num(b.first()?, 0.) - x + 1.),
        jsround(num(b.get(1)?, 0.) - y + 1.),
    ])
}
fn apply<S: PixelSink>(sink: &mut S, spec: &ShapeSpec, fg: u32, bg: u32) -> EngineResult<()> {
    apply_shape(sink, spec, fg, bg)?;
    Ok(())
}
fn paint<S: PixelSink>(sink: &mut S, mask: &crate::raster::Mask, color: u32) {
    for y in 0..mask.h {
        for x in 0..mask.w {
            if mask.is_set(x, y) {
                sink.set(mask.x + x as i32, mask.y + y as i32, color);
            }
        }
    }
}

fn visor_shape(shape: &Value, palette: &Value) -> crate::raster::composite_ops::VisorShape {
    let kind = match shape
        .get("cmd")
        .or_else(|| shape.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("")
    {
        "polygon" => ShapeKind::Polygon,
        "ellipse" | "circle" => ShapeKind::Ellipse,
        _ => ShapeKind::Rrect,
    };
    crate::raster::composite_ops::VisorShape {
        kind,
        bbox: shape
            .get("box")
            .or_else(|| shape.get("bbox"))
            .and_then(boxed),
        points: points(
            shape
                .get("points")
                .or_else(|| shape.get("pts"))
                .unwrap_or(&Value::Null),
        )
        .into_iter()
        .map(|p| [p[0] as f64, p[1] as f64])
        .collect(),
        radius: num(&shape["radius"], 8.) as i32,
        width: num(&shape["width"], 1.) as i32,
        has_fill: color(&shape["fill"], palette, 0) != 0,
        has_outline: color(&shape["outline"], palette, 0) != 0,
    }
}

fn command<S: PixelSink>(
    sink: &mut S,
    c: &Value,
    palette: &Value,
    w: usize,
    h: usize,
) -> EngineResult<()> {
    let typ = c
        .get("type")
        .or_else(|| c.get("cmd"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let fill = color(
        c.get("fill")
            .or_else(|| c.get("color"))
            .unwrap_or(&Value::Null),
        palette,
        0,
    );
    let outline = color(&c["outline"], palette, 0);
    let sw = num(&c["width"], num(&c["strokeWidth"], 1.)) as i32;
    let raw = c.get("box").or_else(|| c.get("bbox"));
    let bbox = raw.and_then(boxed);
    let box_kind = match typ {
        "rect" | "rectangle" => Some((ShapeKind::Rrect, 0)),
        "rounded_rect" => Some((ShapeKind::Rrect, num(&c["radius"], 8.) as i32)),
        "circle" | "ellipse" => Some((ShapeKind::Ellipse, 0)),
        _ => None,
    };
    if let Some((kind, radius)) = box_kind {
        let mode = if fill != 0 && outline != 0 {
            FillMode::Both
        } else if fill != 0 {
            FillMode::Fill
        } else {
            FillMode::Outline
        };
        let spec = ShapeSpec {
            kind,
            stroke_width: sw,
            fill_mode: mode,
            bbox,
            p0: None,
            p1: None,
            radius,
            points: vec![],
        };
        return apply(sink, &spec, if outline != 0 { outline } else { fill }, fill);
    }
    match typ {
        "line" => {
            let pts = points(
                c.get("points")
                    .or_else(|| c.get("pts"))
                    .unwrap_or(&Value::Null),
            );
            for p in pts.windows(2) {
                apply_shape(
                    sink,
                    &ShapeSpec {
                        kind: ShapeKind::Line,
                        stroke_width: sw,
                        fill_mode: FillMode::Outline,
                        bbox: None,
                        p0: Some((p[0][0], p[0][1])),
                        p1: Some((p[1][0], p[1][1])),
                        radius: 0,
                        points: vec![],
                    },
                    if fill != 0 { fill } else { outline },
                    0,
                )?;
            }
            Ok(())
        }
        "lines" => {
            if let Some(segs) = c
                .get("segments")
                .or_else(|| c.get("lines"))
                .and_then(Value::as_array)
            {
                for seg in segs {
                    let pts = points(seg.get("points").or_else(|| seg.get("pts")).unwrap_or(seg));
                    if pts.len() >= 2 {
                        let col = color(
                            seg.get("color")
                                .or_else(|| seg.get("fill"))
                                .or_else(|| c.get("color"))
                                .unwrap_or(&Value::Null),
                            palette,
                            0,
                        );
                        apply_shape(
                            sink,
                            &ShapeSpec {
                                kind: ShapeKind::Line,
                                stroke_width: num(&seg["width"], sw as f64) as i32,
                                fill_mode: FillMode::Outline,
                                bbox: None,
                                p0: Some((pts[0][0], pts[0][1])),
                                p1: Some((pts[1][0], pts[1][1])),
                                radius: 0,
                                points: vec![],
                            },
                            col,
                            0,
                        )?;
                    }
                }
            }
            Ok(())
        }
        "polygon" => {
            let pts = points(
                c.get("pts")
                    .or_else(|| c.get("points"))
                    .unwrap_or(&Value::Null),
            );
            let mode = if fill != 0 && outline != 0 {
                FillMode::Both
            } else if fill != 0 {
                FillMode::Fill
            } else {
                FillMode::Outline
            };
            let spec = ShapeSpec {
                kind: ShapeKind::Polygon,
                stroke_width: sw,
                fill_mode: mode,
                bbox: None,
                p0: None,
                p1: None,
                radius: 0,
                points: pts,
            };
            apply(sink, &spec, if outline != 0 { outline } else { fill }, fill)
        }
        "pixel" => {
            let xy = c.get("pos").or_else(|| c.get("xy"));
            let x = num(&c["x"], xy.map(|v| num(&v[0], 0.)).unwrap_or(0.));
            let y = num(&c["y"], xy.map(|v| num(&v[1], 0.)).unwrap_or(0.));
            sink.set(jsround(x), jsround(y), fill);
            Ok(())
        }
        "alpha" => {
            let sh = c.get("shape").unwrap_or(c);
            let col = color(
                sh.get("color")
                    .or_else(|| sh.get("fill"))
                    .or_else(|| c.get("color"))
                    .unwrap_or(&Value::Null),
                palette,
                0,
            );
            let a = num(&c["alpha"], 255.).round().clamp(0., 255.) as u32;
            let col = if a == 0 {
                0
            } else {
                (col & 0xffffff) | (a << 24)
            };
            let b = sh.get("box").or_else(|| sh.get("bbox")).and_then(boxed);
            let pts = points(
                sh.get("points")
                    .or_else(|| sh.get("pts"))
                    .unwrap_or(&Value::Null),
            );
            let k = match sh
                .get("cmd")
                .or_else(|| sh.get("type"))
                .and_then(Value::as_str)
                .unwrap_or("")
            {
                "ellipse" => ShapeKind::Ellipse,
                "polygon" => ShapeKind::Polygon,
                _ => ShapeKind::Rrect,
            };
            let spec = ShapeSpec {
                kind: k,
                stroke_width: sw,
                fill_mode: if c["outline"] == true {
                    FillMode::Outline
                } else {
                    FillMode::Fill
                },
                bbox: b,
                p0: None,
                p1: None,
                radius: num(&sh["radius"], 8.) as i32,
                points: pts,
            };
            apply(sink, &spec, col, col)
        }
        _ => advanced::command_advanced(sink, c, palette, w, h, typ, fill, outline, sw, raw),
    }
}

pub fn render<S: PixelSink>(
    sink: &mut S,
    commands: &[Value],
    palette: &Value,
    width: usize,
    height: usize,
) -> EngineResult<()> {
    for c in commands {
        command(sink, c, palette, width, height)?;
    }
    Ok(())
}

pub fn render_vector_layer<S: PixelSink>(
    sink: &mut S,
    commands: &[Value],
    palette: &Value,
    width: usize,
    height: usize,
) -> EngineResult<()> {
    let resolved: Vec<Value> = commands
        .iter()
        .map(|command| {
            let mut command = command.clone();
            if let Some(object) = command.as_object_mut() {
                for key in ["fill", "color"] {
                    if let Some(token) = object
                        .get(key)
                        .and_then(Value::as_str)
                        .and_then(|s| s.strip_prefix('$'))
                    {
                        object.insert(
                            key.to_owned(),
                            palette
                                .get(token)
                                .cloned()
                                .unwrap_or_else(|| serde_json::json!("#000000")),
                        );
                    }
                }
            }
            command
        })
        .collect();
    render(sink, &resolved, palette, width, height)
}
