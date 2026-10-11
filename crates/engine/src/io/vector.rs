use crate::{
    blend::BLEND_MODES,
    chunkstore::{ChunkStore, PixelWriter},
    constants::{CHUNK_PX, SCHEMA_VERSION},
    errors::{CODE_SCHEMA, DrawToolError, EngineResult},
    model::{CanvasInfo, Document, DocumentSpec, IdGen, Layer, Viewport},
    recipe::render_vector_layer,
};
use serde_json::{Value, json};

fn extract_chunk(layer: &Layer, cx: u32, cy: u32) -> Vec<Value> {
    let Some(chunk) = layer.store.get_chunk(cx, cy) else {
        return Vec::new();
    };
    let mut seen = [false; (CHUNK_PX * CHUNK_PX) as usize];
    let mut commands = Vec::new();
    for y in 0..CHUNK_PX as usize {
        for x in 0..CHUNK_PX as usize {
            let idx = y * CHUNK_PX as usize + x;
            if seen[idx] {
                continue;
            }
            let o = idx * 4;
            let (r, g, b, a) = (chunk[o], chunk[o + 1], chunk[o + 2], chunk[o + 3]);
            if a == 0 {
                continue;
            }
            let mut max_x = x;
            while max_x + 1 < CHUNK_PX as usize && !seen[y * CHUNK_PX as usize + max_x + 1] {
                let q = (y * CHUNK_PX as usize + max_x + 1) * 4;
                if chunk[q + 3] == 0 || chunk[q] != r || chunk[q + 1] != g || chunk[q + 2] != b {
                    break;
                }
                max_x += 1;
            }
            let mut max_y = y;
            'grow: while max_y + 1 < CHUNK_PX as usize {
                for xx in x..=max_x {
                    let q = ((max_y + 1) * CHUNK_PX as usize + xx) * 4;
                    let i = (max_y + 1) * CHUNK_PX as usize + xx;
                    if chunk[q + 3] == 0
                        || seen[i]
                        || chunk[q] != r
                        || chunk[q + 1] != g
                        || chunk[q + 2] != b
                    {
                        break 'grow;
                    }
                }
                max_y += 1;
            }
            for yy in y..=max_y {
                for xx in x..=max_x {
                    seen[yy * CHUNK_PX as usize + xx] = true
                }
            }
            let color = format!("#{r:02x}{g:02x}{b:02x}");
            let ox = cx * CHUNK_PX;
            let oy = cy * CHUNK_PX;
            if max_x == x && max_y == y {
                commands.push(json!({"cmd":"pixel","x":ox+x as u32,"y":oy+y as u32,"color":color}));
            } else {
                commands.push(json!({"cmd":"rect","box":[[ox+x as u32,oy+y as u32],[ox+max_x as u32,oy+max_y as u32]],"fill":color}));
            }
        }
    }
    commands
}
pub fn document_to_vector_json(doc: &Document) -> Value {
    let mut layers = Vec::new();
    for layer in &doc.layers {
        let mut commands = Vec::new();
        layer
            .store
            .for_each_chunk(|cx, cy, _| commands.extend(extract_chunk(layer, cx, cy)));
        layers.push(json!({"layer_id":layer.id,"name":layer.name,"visible":layer.visible,"locked":layer.locked,"opacity":layer.opacity,"blend":layer.blend,"commands":commands}));
    }
    let mut canvas = json!({"width_px":doc.canvas.width_px,"height_px":doc.canvas.height_px,"background":doc.canvas.background});
    if let Some(v) = doc.canvas.viewport {
        canvas["viewport"] = json!({"x":v.x,"y":v.y,"w":v.w,"h":v.h})
    }
    json!({"schema_version":SCHEMA_VERSION,"format":"draw_tool.vector","canvas":canvas,"layers":layers})
}
pub fn layer_to_vector_json(doc: &Document, layer_id: &str) -> EngineResult<Value> {
    let layer = doc.get_layer(layer_id)?;
    let mut out = document_to_vector_json(doc);
    out["document_id"] = json!(doc.id);
    out["name"] = json!(layer.name);
    out["layers"] = Value::Array(
        out["layers"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|l| l["layer_id"] == layer_id)
            .cloned()
            .collect(),
    );
    Ok(out)
}
fn bad(message: &str) -> DrawToolError {
    DrawToolError::new(CODE_SCHEMA, message)
}
pub fn vector_json_to_document(obj: &Value) -> EngineResult<Document> {
    if !obj.is_object() {
        return Err(bad("vector JSON must be an object"));
    }
    let c = obj
        .get("canvas")
        .filter(|v| v.is_object())
        .ok_or_else(|| bad("vector JSON is missing canvas"))?;
    let width = c["width_px"]
        .as_u64()
        .ok_or_else(|| bad("invalid canvas width"))? as u32;
    let height = c["height_px"]
        .as_u64()
        .ok_or_else(|| bad("invalid canvas height"))? as u32;
    let src = obj["layers"]
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| bad("vector JSON must have a non-empty layers array"))?;
    let mut layers = Vec::new();
    let mut ids = IdGen::new();
    for (i, entry) in src.iter().enumerate() {
        let id = entry["layer_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("layer_{}", i + 1));
        let name = entry["name"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Layer {}", i + 1));
        let store = ChunkStore::new(width, height)?;
        let blend = entry["blend"]
            .as_str()
            .and_then(|mode| {
                BLEND_MODES
                    .iter()
                    .copied()
                    .find(|candidate| *candidate == mode)
            })
            .unwrap_or("normal");
        let mut layer = Layer::new(
            id.clone(),
            name,
            entry["visible"] != false,
            entry["locked"] == true,
            entry["opacity"].as_f64().unwrap_or(1.),
            blend,
            store,
        )?;
        {
            let mut writer = PixelWriter::new(&mut layer.store, &id);
            let commands = entry["commands"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            render_vector_layer(
                &mut writer,
                commands,
                obj.get("palette").unwrap_or(&Value::Null),
                width as usize,
                height as usize,
            )?;
            writer.finish();
        }
        ids.seed([id]);
        layers.push(layer);
    }
    let active = layers[0].id.clone();
    let viewport = c
        .get("viewport")
        .filter(|v| !v.is_null())
        .map(|v| Viewport {
            x: v["x"].as_u64().unwrap_or(0) as u32,
            y: v["y"].as_u64().unwrap_or(0) as u32,
            w: v["w"].as_u64().unwrap_or(0) as u32,
            h: v["h"].as_u64().unwrap_or(0) as u32,
        });
    Document::new(DocumentSpec {
        id: obj["document_id"].as_str().unwrap_or_default().into(),
        name: obj["name"].as_str().unwrap_or("Untitled").into(),
        canvas: CanvasInfo {
            width_px: width,
            height_px: height,
            background: c["background"].as_str().unwrap_or("transparent").into(),
            viewport,
        },
        layers,
        active_layer_id: active,
        ids,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NewDocumentOptions, new_document};
    #[test]
    fn vector_export_and_import_round_trip_solid_pixels() {
        let mut doc = new_document(NewDocumentOptions::default().size(32, 32)).unwrap();
        {
            let mut w = PixelWriter::new(&mut doc.layers[0].store, "layer-0001");
            w.set(2, 3, 0xff0000ff);
            w.set(3, 3, 0xff0000ff);
            w.finish();
        }
        let v = document_to_vector_json(&doc);
        assert_eq!(v["layers"][0]["commands"][0]["cmd"], "rect");
        let copy = vector_json_to_document(&v).unwrap();
        assert_eq!(copy.layers[0].store.get_pixel(2, 3), 0xff0000ff);
    }
    #[test]
    fn vector_import_rejects_empty_layers() {
        assert!(
            vector_json_to_document(&json!({"canvas":{"width_px":32,"height_px":32},"layers":[]}))
                .is_err()
        );
    }
}
