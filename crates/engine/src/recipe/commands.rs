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
        "radial_gradient" => {
            let center = c.get("center").and_then(Value::as_array);
            let stops = c["stops"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| {
                    let a = s.as_array()?;
                    if a.len() < 5 {
                        return None;
                    }
                    let b = |i: usize| num(&a[i], 0.).clamp(0., 255.);
                    Some(crate::raster::Stop::new(
                        num(&a[0], 0.),
                        b(1),
                        b(2),
                        b(3),
                        b(4),
                    ))
                })
                .collect();
            let mask = crate::raster::radial_gradient_mask(&crate::raster::RadialGradient {
                cx: center.map(|a| num(&a[0], 64.)).unwrap_or(64.),
                cy: center.map(|a| num(&a[1], 64.)).unwrap_or(64.),
                r0: num(&c["r0"], 0.),
                r1: num(&c["r1"], 16.),
                stops,
                w,
                h,
            })?;
            for y in 0..h {
                for x in 0..w {
                    let p = mask.data[y * w + x];
                    if p != 0 {
                        sink.set(x as i32, y as i32, p)
                    }
                }
            }
            Ok(())
        }
        "erase" => {
            let pts = points(
                c.get("points")
                    .or_else(|| c.get("pts"))
                    .unwrap_or(&Value::Null),
            );
            crate::raster::erase_polygon(sink, &pts, w, h);
            Ok(())
        }
        "clip_alpha" => {
            let sh = c.get("shape").unwrap_or(c);
            let b = sh.get("box").or_else(|| sh.get("bbox")).and_then(|v| {
                let a = v[0].as_array()?;
                let z = v[1].as_array()?;
                Some((
                    num(&a[0], 0.),
                    num(&a[1], 0.),
                    num(&z[0], 0.),
                    num(&z[1], 0.),
                ))
            });
            if let Some((x0, y0, x1, y1)) = b {
                crate::raster::composite_ops::clip_alpha(
                    sink,
                    &crate::raster::composite_ops::Clip {
                        x0,
                        y0,
                        x1,
                        y1,
                        radius: num(&sh["radius"], 0.) as i32,
                    },
                    w,
                    h,
                )
            }
            Ok(())
        }
        "spokes" => {
            let center = c.get("center");
            let cx = center.map(|v| num(&v[0], 64.)).unwrap_or(64.);
            let cy = center.map(|v| num(&v[1], 64.)).unwrap_or(64.);
            let count = num(&c["count"], 8.) as usize;
            let length = num(&c["length"], 20.);
            let inner = num(&c["inner_radius"], 0.);
            let offset = num(&c["angle_offset"], 0.);
            let width = num(&c["width"], 1.) as i32;
            let col = if fill != 0 { fill } else { outline };
            for i in 0..count {
                let r = (offset + 360.0 * i as f64 / count as f64).to_radians();
                let a = (jsround(cx + inner * r.cos()), jsround(cy + inner * r.sin()));
                let b = (
                    jsround(cx + (inner + length) * r.cos()),
                    jsround(cy + (inner + length) * r.sin()),
                );
                apply_shape(
                    sink,
                    &ShapeSpec {
                        kind: ShapeKind::Line,
                        stroke_width: width,
                        fill_mode: FillMode::Outline,
                        bbox: None,
                        p0: Some(a),
                        p1: Some(b),
                        radius: 0,
                        points: vec![],
                    },
                    col,
                    0,
                )?;
            }
            Ok(())
        }
        "visor" => {
            let shape_value = c.get("shape").unwrap_or(&Value::Null);
            let shape = visor_shape(shape_value, palette);
            let crt = c.get("crt_lines").filter(|v| !v.is_null()).map(|v| {
                let yr = v.get("y_range").and_then(Value::as_array);
                let xr = v.get("x_range").and_then(Value::as_array);
                crate::raster::composite_ops::CrtLines {
                    y0: yr.map(|a| num(&a[0], 45.) as i32).unwrap_or(45),
                    y1: yr.map(|a| num(&a[1], 80.) as i32).unwrap_or(80),
                    step: yr.map(|a| num(&a[2], 3.) as i32).unwrap_or(3),
                    x0: xr.map(|a| num(&a[0], 20.) as i32).unwrap_or(20),
                    x1: xr.map(|a| num(&a[1], 108.) as i32).unwrap_or(108),
                    alpha: num(&v["alpha"], 45.) as i32,
                }
            });
            let fill = color(
                c.get("fill")
                    .or_else(|| shape_value.get("fill"))
                    .unwrap_or(&Value::Null),
                palette,
                0,
            );
            let outline = color(
                c.get("outline")
                    .or_else(|| shape_value.get("outline"))
                    .unwrap_or(&Value::Null),
                palette,
                0,
            );
            crate::raster::composite_ops::visor_layer(
                sink,
                &shape,
                crt.as_ref(),
                w,
                h,
                fill,
                outline,
            );
            Ok(())
        }
        "sheen" => {
            let shape = visor_shape(c.get("shape").unwrap_or(&Value::Null), palette);
            let sheen = crate::raster::composite_ops::Sheen {
                p1: [num(&c["p1"][0], 0.), num(&c["p1"][1], 0.)],
                p2: [num(&c["p2"][0], 0.), num(&c["p2"][1], 0.)],
                dx: num(&c["dx"], 0.),
            };
            crate::raster::composite_ops::sheen_overlay(sink, &shape, Some(&sheen), w, h);
            Ok(())
        }
        "hud_brackets" => {
            let b = c.get("box").or_else(|| c.get("bbox"));
            let a = b.map(|v| &v[0]);
            let z = b.map(|v| &v[1]);
            let rgba = unpack_rgba(color(c.get("color").unwrap_or(&Value::Null), palette, 0));
            let hud = crate::raster::composite_ops::Hud {
                x0: a.map(|v| num(&v[0], 36.) as i32).unwrap_or(36),
                y0: a.map(|v| num(&v[1], 60.) as i32).unwrap_or(60),
                x1: z.map(|v| num(&v[0], 92.) as i32).unwrap_or(92),
                y1: z.map(|v| num(&v[1], 82.) as i32).unwrap_or(82),
                arm: num(&c["arm"], 5.) as i32,
                color: rgba,
                width: num(&c["width"], 1.) as i32,
            };
            crate::raster::composite_ops::hud_brackets(sink, &hud);
            Ok(())
        }
        "text" => super::text::render(sink, c, palette, w, h),
        "arc" | "chord" => {
            if let Some(b) = raw {
                let p0 = &b[0];
                let p1 = &b[1];
                let (x0, y0, x1, y1) = (
                    num(&p0[0], 0.),
                    num(&p0[1], 0.),
                    num(&p1[0], 0.),
                    num(&p1[1], 0.),
                );
                let bb = crate::raster::Box2::from_corners([x0, y0], [x1, y1]);
                let (start, end) = (num(&c["start"], 0.), num(&c["end"], 360.));
                if typ == "arc" {
                    paint(
                        sink,
                        &arc_mask(bb, start, end, sw, w, h)?,
                        if fill != 0 { fill } else { outline },
                    );
                } else {
                    if fill != 0 {
                        paint(sink, &chord_fill_mask(bb, start, end, w, h)?, fill);
                    }
                    if outline != 0 {
                        paint(
                            sink,
                            &chord_outline_mask(bb, start, end, sw, w, h)?,
                            outline,
                        );
                    }
                }
            }
            Ok(())
        }
        _ => Err(DrawToolError::invalid_state(format!(
            "unsupported recipe command `{typ}`"
        ))),
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
