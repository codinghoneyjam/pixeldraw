//! Recipe commands for gradients, masks, and compositing effects.

use super::*;

#[allow(clippy::too_many_arguments)] // Keep the command's ported drawing context explicit.
pub(super) fn command_advanced<S: PixelSink>(
    sink: &mut S,
    c: &Value,
    palette: &Value,
    w: usize,
    h: usize,
    typ: &str,
    fill: u32,
    outline: u32,
    sw: i32,
    raw: Option<&Value>,
) -> EngineResult<()> {
    match typ {
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
        "text" => super::super::text::render(sink, c, palette, w, h),
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
