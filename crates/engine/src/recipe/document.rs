use super::render_commands;
use crate::errors::{DrawToolError, EngineResult};
use crate::{
    chunkstore::{ChunkStore, PixelWriter},
    model::{CanvasInfo, Document, DocumentSpec, IdGen, Layer, Viewport},
};
use serde_json::{Value, json};
fn bad(s: &str) -> DrawToolError {
    DrawToolError::invalid_state(s)
}
fn u(v: &Value, d: u32) -> u32 {
    v.as_u64().map(|x| x as u32).unwrap_or(d)
}
fn shifted(command: &Value, dx: u32) -> Value {
    let mut c = command.clone();
    if let Some(o) = c.as_object_mut() {
        for key in ["box", "bbox"] {
            if let Some(a) = o.get_mut(key).and_then(Value::as_array_mut) {
                if let Some(pair) = a.first_mut().and_then(Value::as_array_mut)
                    && let Some(x) = pair.first_mut()
                {
                    *x = json!(x.as_f64().unwrap_or(0.) + dx as f64)
                }
                if let Some(pair) = a.get_mut(1).and_then(Value::as_array_mut)
                    && let Some(x) = pair.first_mut()
                {
                    *x = json!(x.as_f64().unwrap_or(0.) + dx as f64)
                }
            }
        }
        for key in ["center", "pos", "xy", "pen"] {
            if let Some(a) = o.get_mut(key).and_then(Value::as_array_mut)
                && let Some(x) = a.first_mut()
            {
                *x = json!(x.as_f64().unwrap_or(0.) + dx as f64)
            }
        }
        for key in ["points", "pts"] {
            if let Some(a) = o.get_mut(key).and_then(Value::as_array_mut) {
                for p in a {
                    if let Some(v) = p.as_array_mut() {
                        if let Some(x) = v.first_mut() {
                            *x = json!(x.as_f64().unwrap_or(0.) + dx as f64)
                        }
                    } else if let Some(x) = p.get_mut("x") {
                        *x = json!(x.as_f64().unwrap_or(0.) + dx as f64)
                    }
                }
            }
        }
    }
    c
}
fn canvas_size(obj: &Value) -> (u32, u32) {
    let e = &obj["export"]["canvas"];
    let c = &obj["canvas"];
    if !e.is_null() {
        return (u(&e["width"], 128), u(&e["height"], 128));
    }
    if let Some(a) = c.as_array() {
        return (
            u(a.first().unwrap_or(&Value::Null), 128),
            u(a.get(1).unwrap_or(&Value::Null), 128),
        );
    }
    (u(&c["width"], 128), u(&c["height"], 128))
}
fn viewport(obj: &Value) -> Option<Viewport> {
    let v = obj
        .get("export")
        .and_then(|e| e.get("viewport"))
        .or_else(|| obj.get("crop"))
        .filter(|v| !v.is_null())?;
    if v.is_array() {
        Some(Viewport {
            x: 0,
            y: 0,
            w: u(&v[0], 0),
            h: u(&v[1], 0),
        })
    } else {
        Some(Viewport {
            x: u(&v["x"], 0),
            y: u(&v["y"], 0),
            w: u(&v["w"], 0),
            h: u(&v["h"], 0),
        })
    }
}
fn palette(obj: &Value, extra: &Value) -> Value {
    let mut p = obj
        .get("palette")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(o) = obj.get("dynamic_colors").and_then(Value::as_object) {
        p.extend(o.clone())
    }
    if let Some(o) = extra.as_object() {
        p.extend(o.clone())
    }
    Value::Object(p)
}
fn bake(
    id: &str,
    name: &str,
    commands: &[Value],
    pal: &Value,
    w: u32,
    h: u32,
    visible: bool,
) -> EngineResult<Layer> {
    let mut store = ChunkStore::new(w, h)?;
    {
        let mut writer = PixelWriter::new(&mut store, id);
        render_commands(&mut writer, commands, pal, w as usize, h as usize)?;
        writer.finish();
    }
    Layer::new(id, name, visible, false, 1., "normal", store)
}
fn std_defs(obj: &Value, key: Option<&str>) -> Vec<(String, Vec<Value>)> {
    let mut defs = Vec::new();
    if let Some(a) = obj["layers"].as_array() {
        for l in a {
            let id = l["id"].as_str().unwrap_or("layer");
            if key.is_none_or(|k| k == id) {
                defs.push((
                    id.into(),
                    l["commands"].as_array().cloned().unwrap_or_default(),
                ))
            }
        }
        return defs;
    }
    if let Some(m) = obj["layers"].as_object() {
        for (id, v) in m {
            if key.is_none_or(|k| k == id) {
                defs.push((
                    id.clone(),
                    v.as_array()
                        .cloned()
                        .or_else(|| v["commands"].as_array().cloned())
                        .unwrap_or_default(),
                ))
            }
        }
        return defs;
    }
    if obj.get("albedo_layers").is_some() || obj.get("mask_layers").is_some() {
        if key.is_none() || key == Some("albedo") {
            defs.push((
                "albedo".into(),
                obj["albedo_layers"].as_array().cloned().unwrap_or_default(),
            ))
        }
        if key == Some("mask") || key == Some("mask_layers") {
            defs = vec![(
                "mask".into(),
                obj["mask_layers"].as_array().cloned().unwrap_or_default(),
            )]
        } else if key.is_none() && obj.get("mask_layers").is_some() {
            defs.push((
                "mask".into(),
                obj["mask_layers"].as_array().cloned().unwrap_or_default(),
            ))
        }
        return defs;
    }
    for (id, v) in obj.as_object().into_iter().flatten() {
        if key.is_none_or(|k| k == id)
            && v.as_array().is_some_and(|a| {
                !a.is_empty()
                    && a.iter()
                        .all(|x| x["cmd"].is_string() || x["type"].is_string())
            })
        {
            defs.push((id.clone(), v.as_array().unwrap().clone()))
        }
    }
    defs
}
pub fn recipe_json_to_document(
    obj: &Value,
    layer_key: Option<&str>,
    slot_id: Option<&str>,
    overrides: &Value,
) -> EngineResult<Document> {
    if !obj.is_object() {
        return Err(bad("recipe JSON must be an object"));
    }
    let pal = palette(obj, overrides);
    let bg = if obj["canvas"]["background"] == "white" {
        "#ffffff".to_owned()
    } else {
        "transparent".to_owned()
    };
    let ids = IdGen::new();
    // Surface manifest schema.
    if let Some(surfaces) = obj.get("surfaces").and_then(Value::as_array) {
        let id = slot_id
            .or(layer_key)
            .or_else(|| surfaces.first().and_then(|s| s["id"].as_str()))
            .ok_or_else(|| bad("surface id missing"))?;
        let sf = surfaces
            .iter()
            .find(|s| s["id"] == id)
            .ok_or_else(|| bad("unknown surface id"))?;
        let (w, h) = (
            u(&sf["canvas"]["width"], 128),
            u(&sf["canvas"]["height"], 128),
        );
        let cmds = sf["layers"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|l| {
                let mut c = l.clone();
                if c.get("cmd").is_none() {
                    c["cmd"] = c["type"].clone()
                }
                if c.get("box").is_none() && c.get("bbox").is_some() {
                    c["box"] = c["bbox"].clone()
                }
                c
            })
            .collect::<Vec<_>>();
        let layer = bake(id, id, &cmds, &pal, w, h, true)?;
        return Document::new(DocumentSpec {
            id: obj["document_id"].as_str().unwrap_or("doc-recipe").into(),
            name: obj["manifest_id"].as_str().unwrap_or(id).into(),
            canvas: CanvasInfo {
                width_px: w,
                height_px: h,
                background: bg,
                viewport: None,
            },
            layers: vec![layer],
            active_layer_id: id.into(),
            ids,
        });
    }
    // Pantograph geometry profile becomes stable rounded-rectangle commands.
    if obj.get("geometry").is_some() && obj.get("colors").is_some() && obj["slots"].is_array() {
        let key = slot_id.or(layer_key);
        let entry = key.and_then(|k| obj["nine_patch"].get(k));
        let (w, h, cmds, id) = if let Some(e) = entry {
            let id = key.unwrap();
            (
                u(&e["width"], 128),
                u(&e["height"], 128),
                pantograph_cmds(obj, e["layers"].as_array().unwrap_or(&vec![])),
                id.to_owned(),
            )
        } else {
            let cw = u(&obj["canvas"]["slot_width"], 128);
            let w = u(&obj["canvas"]["total_width"], cw);
            let h = u(&obj["canvas"]["total_height"], 128);
            let mut all = Vec::new();
            for slot in obj["slots"].as_array().unwrap() {
                for c in pantograph_cmds(obj, slot["layers"].as_array().unwrap_or(&vec![])) {
                    all.push(shifted(&c, u(&slot["index"], 0) * cw))
                }
            }
            (w, h, all, "sheet".into())
        };
        let layer = bake(&id, &id, &cmds, &pal, w, h, true)?;
        return Document::new(DocumentSpec {
            id: "doc-recipe".into(),
            name: obj["name"].as_str().unwrap_or("pantograph_sheet").into(),
            canvas: CanvasInfo {
                width_px: w,
                height_px: h,
                background: bg,
                viewport: None,
            },
            layers: vec![layer],
            active_layer_id: id,
            ids,
        });
    }
    // Atlas slots render one tile per slot and paste into a horizontal sheet.
    let atlas = obj
        .get("export")
        .and_then(|e| e.get("atlas"))
        .filter(|a| !a.is_null())
        .or(obj.get("atlas"));
    if let Some(slots) = atlas
        .and_then(|a| a["slots"].as_array())
        .filter(|a| !a.is_empty())
    {
        let cw = u(
            &atlas.unwrap()["cell_width"],
            u(
                &atlas.unwrap()["cellWidth"],
                u(&obj["canvas"]["width"], 128),
            ),
        );
        let ch = u(
            &atlas.unwrap()["cell_height"],
            u(&obj["canvas"]["height"], 128),
        );
        let mut sheet = ChunkStore::new(cw * slots.len() as u32, ch)?;
        let mut layers = Vec::new();
        for (i, slot) in slots.iter().enumerate() {
            let mut cmds = Vec::new();
            for l in slot["layers"].as_array().into_iter().flatten() {
                if layer_key.is_none_or(|k| l["id"] == k) {
                    cmds.extend(l["commands"].as_array().cloned().unwrap_or_default())
                }
            }
            let mut sp = ChunkStore::new(cw, ch)?;
            {
                let mut w = PixelWriter::new(&mut sp, "slot");
                render_commands(&mut w, &cmds, &pal, cw as usize, ch as usize)?;
                w.finish();
            }
            for y in 0..ch {
                for x in 0..cw {
                    let px = sp.get_pixel(x, y);
                    if px != 0 {
                        sheet.set_pixel(i as u32 * cw + x, y, px)
                    }
                }
            }
            let sid = slot["id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("slot_{i}"));
            let mut hidden = ChunkStore::new(cw * slots.len() as u32, ch)?;
            for y in 0..ch {
                for x in 0..cw {
                    let px = sp.get_pixel(x, y);
                    if px != 0 {
                        hidden.set_pixel(i as u32 * cw + x, y, px)
                    }
                }
            }
            layers.push(Layer::new(
                sid.clone(),
                sid,
                false,
                false,
                1.,
                "normal",
                hidden,
            )?)
        }
        let visible = Layer::new("sheet", "sheet", true, false, 1., "normal", sheet)?;
        layers.insert(0, visible);
        return Document::new(DocumentSpec {
            id: "doc-recipe".into(),
            name: obj["name"].as_str().unwrap_or("sheet").into(),
            canvas: CanvasInfo {
                width_px: cw * slots.len() as u32,
                height_px: ch,
                background: bg,
                viewport: None,
            },
            layers,
            active_layer_id: "sheet".into(),
            ids,
        });
    }
    let (w, h) = canvas_size(obj);
    let defs = std_defs(obj, layer_key);
    if defs.is_empty() {
        return Err(bad("recipe has no layers to import"));
    }
    let mut layers = Vec::new();
    for (id, cmds) in defs {
        layers.push(bake(&id, &id, &cmds, &pal, w, h, true)?)
    }
    let active = layers[0].id.clone();
    Document::new(DocumentSpec {
        id: obj["document_id"].as_str().unwrap_or("doc-recipe").into(),
        name: obj["name"].as_str().unwrap_or("recipe").into(),
        canvas: CanvasInfo {
            width_px: w,
            height_px: h,
            background: bg,
            viewport: viewport(obj),
        },
        layers,
        active_layer_id: active,
        ids,
    })
}
fn pantograph_cmds(profile: &Value, layers: &[Value]) -> Vec<Value> {
    let g = &profile["geometry"];
    let c = &profile["colors"];
    let mut out = Vec::new();
    let mk = |b: &Value, r: i32, fill: &Value, outline: &Value, width: Value| {
        let mut v = json!({"cmd":"rounded_rect","box":[[b[0],b[1]],[b[2],b[3]]],"radius":r,"fill":fill,"outline":outline});
        if !width.is_null() {
            v["width"] = width
        }
        v
    };
    for l in layers {
        if let Some(k) = l.as_str() {
            let pair = match k {
                "outer_frame" => Some((
                    &g["outer_frame"],
                    "bbox",
                    "outer_frame_bg_hex",
                    "outer_frame_border_hex",
                )),
                "socket_well" => Some((&g["socket_well"], "bbox", "socket_well_bg_hex", "")),
                "inner_keycap_normal" => Some((
                    &g["inner_keycap"],
                    "normal_bbox",
                    "keycap_body_normal_hex",
                    "keycap_border_normal_hex",
                )),
                "inner_keycap_pressed" => Some((
                    &g["inner_keycap"],
                    "pressed_bbox",
                    "keycap_body_pressed_hex",
                    "keycap_border_pressed_hex",
                )),
                "inner_keycap_hover" => Some((
                    &g["inner_keycap"],
                    "normal_bbox",
                    "keycap_body_hover_hex",
                    "keycap_border_hover_hex",
                )),
                "inner_keycap_disabled" => Some((
                    &g["inner_keycap"],
                    "normal_bbox",
                    "keycap_body_disabled_hex",
                    "keycap_border_disabled_hex",
                )),
                "visor_window_normal" | "visor_window_pressed" => Some((
                    &g["visor_window"],
                    if layers.iter().any(|x| x == "visor_window_pressed") {
                        "pressed_bbox"
                    } else {
                        "normal_bbox"
                    },
                    "visor_bg_hex",
                    "visor_border_hex",
                )),
                _ => None,
            };
            if let Some((spec, bn, f, ol)) = pair {
                out.push(mk(
                    &spec[bn],
                    u(&spec["corner_radius"], 0) as i32,
                    &c[f],
                    if ol.is_empty() { &Value::Null } else { &c[ol] },
                    json!(
                        spec.get("border_width")
                            .or(spec.get("borderwidth"))
                            .map(|v| u(v, 1))
                    ),
                ));
            }
        }
    }
    out
}
