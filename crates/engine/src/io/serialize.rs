use crate::blend::BLEND_MODES;
use crate::chunkstore::{ChunkStore, is_all_zero};
use crate::constants::{
    CHUNK_LEN, CHUNK_PX, READABLE_SCHEMA_VERSIONS, SCHEMA_VERSION, TILE_PX, UNIT_PX,
};
use crate::errors::{CODE_SCHEMA, CODE_UNSUPPORTED_LAYER_TYPE, DrawToolError, EngineResult};
use crate::io::{
    base64_to_bytes, bytes_to_base64,
    png::{decode_png, encode_png_chunk},
    validate::{validate_document, validate_layer_file},
};
use crate::model::{CanvasInfo, Document, DocumentSpec, IdGen, Layer, Viewport};
use rayon::prelude::*;
use serde_json::{Value, json};

fn canvas_json(doc: &Document) -> Value {
    let c = &doc.canvas;
    let mut value = json!({"tile_px":TILE_PX,"unit_px":UNIT_PX,"width_px":c.width_px,"height_px":c.height_px,"background":c.background});
    if let Some(v) = c.viewport {
        value["viewport"] = json!({"x":v.x,"y":v.y,"w":v.w,"h":v.h});
    }
    value
}
fn layer_json(layer: &Layer) -> EngineResult<Value> {
    // Borrow the chunk payloads instead of copying every 4 KB chunk: the store
    // already owns them for the whole call, and `for_each_chunk` walks key
    // ascending, so the sort below is only a guard for a future store change.
    let mut chunks = Vec::new();
    for cy in 0..layer.store.chunks_y {
        for cx in 0..layer.store.chunks_x {
            if let Some(data) = layer.store.get_chunk(cx, cy)
                && !is_all_zero(data)
            {
                chunks.push((cy, cx, data));
            }
        }
    }
    chunks.sort_by_key(|(cy, cx, _)| (*cy, *cx));
    // One PNG encode per chunk, none dependent on another: the rows go to
    // rayon the way composite/flatten already do. `collect` keeps index order,
    // so the bytes are identical to the sequential loop this replaced — the
    // same output-byte rule the Phase 5 experiments followed.
    let encoded: Vec<Value> = chunks
        .par_iter()
        .map(|(cy, cx, data)| -> EngineResult<Value> {
            let png = encode_png_chunk(&data[..])?;
            Ok(json!({"cx":cx,"cy":cy,"png":bytes_to_base64(&png)}))
        })
        .collect::<EngineResult<Vec<Value>>>()?;
    let chunks = encoded;
    Ok(
        json!({"layer_id":layer.id,"name":layer.name,"type":"raster","visible":layer.visible,"locked":layer.locked,"opacity":layer.opacity,"blend":layer.blend,"raster":{"chunk_px":CHUNK_PX,"encoding":"png_base64","chunks":chunks}}),
    )
}
pub fn document_to_json(doc: &Document) -> EngineResult<Value> {
    let mut layers = Vec::with_capacity(doc.layers.len());
    for layer in &doc.layers {
        layers.push(layer_json(layer)?);
    }
    let mut result = json!({"schema_version":SCHEMA_VERSION,"format":"draw_tool.document","document_id":doc.id,"canvas":canvas_json(doc),"active_layer_id":doc.active_layer_id,"layers":layers});
    result["name"] = json!(doc.name);
    if let Some(atlas) = &doc.atlas {
        result["atlas"] = serde_json::to_value(atlas)
            .map_err(|e| DrawToolError::invalid_state(format!("atlas encode failed: {e}")))?;
    }
    Ok(result)
}
pub fn layer_to_json(doc: &Document, layer_id: &str) -> EngineResult<Value> {
    let layer = doc.get_layer(layer_id)?;
    Ok(
        json!({"schema_version":SCHEMA_VERSION,"format":"draw_tool.layer","source_canvas":canvas_json(doc),"layer":layer_json(layer)?}),
    )
}
fn invalid(message: &str) -> DrawToolError {
    DrawToolError::new(CODE_SCHEMA, message)
}
fn first_validation_error(report: crate::io::validate::ValidationReport) -> DrawToolError {
    let e = report
        .errors
        .into_iter()
        .next()
        .unwrap_or(crate::io::validate::ValidationError {
            code: CODE_SCHEMA,
            path: String::new(),
            message: "invalid JSON".into(),
        });
    DrawToolError::with_details(
        e.code,
        e.message,
        serde_json::json!({"path":e.path}).to_string(),
    )
}
fn decode_chunk(png: &str) -> EngineResult<Option<Box<[u8; CHUNK_LEN]>>> {
    let bytes = base64_to_bytes(png)?;
    let image = decode_png(&bytes).map_err(|_| {
        DrawToolError::new(crate::errors::CODE_CHUNK_BAD_PNG, "chunk png decode failed")
    })?;
    if image.width != CHUNK_PX || image.height != CHUNK_PX {
        return Err(DrawToolError::new(
            crate::errors::CODE_CHUNK_BAD_PNG,
            "chunk png must be 32x32",
        ));
    }
    let mut data = Box::new([0u8; CHUNK_LEN]);
    data.copy_from_slice(&image.rgba);
    for i in (0..CHUNK_LEN).step_by(4) {
        if data[i + 3] == 0 {
            data[i..i + 3].fill(0);
        }
    }
    if is_all_zero(&data) {
        Ok(None)
    } else {
        Ok(Some(data))
    }
}
fn from_canvas(c: &Value) -> CanvasInfo {
    let viewport = c
        .get("viewport")
        .filter(|v| !v.is_null())
        .map(|v| Viewport {
            x: v["x"].as_u64().unwrap_or(0) as u32,
            y: v["y"].as_u64().unwrap_or(0) as u32,
            w: v["w"].as_u64().unwrap_or(0) as u32,
            h: v["h"].as_u64().unwrap_or(0) as u32,
        });
    CanvasInfo {
        width_px: c["width_px"].as_u64().unwrap_or(0) as u32,
        height_px: c["height_px"].as_u64().unwrap_or(0) as u32,
        background: c
            .get("background")
            .and_then(Value::as_str)
            .unwrap_or("transparent")
            .into(),
        viewport,
    }
}
/// Rewrite an older readable schema version to the current one. New optional
/// fields are left absent (treated as `None`/empty by `json_to_document`).
pub fn migrate_document(value: &Value) -> EngineResult<Value> {
    let mut v = value.clone();
    let ver = v
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !READABLE_SCHEMA_VERSIONS.contains(&ver) {
        return Err(DrawToolError::new(
            CODE_SCHEMA,
            format!("unsupported schema_version {ver}"),
        ));
    }
    if ver != SCHEMA_VERSION
        && let Some(obj) = v.as_object_mut()
    {
        obj.insert(
            "schema_version".to_string(),
            Value::String(SCHEMA_VERSION.to_string()),
        );
    }
    Ok(v)
}

/// Check that `value` carries a supported schema version (no migration).
pub fn validate_supported_schema(value: &Value) -> EngineResult<()> {
    let ver = value
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if READABLE_SCHEMA_VERSIONS.contains(&ver) {
        Ok(())
    } else {
        Err(DrawToolError::new(
            CODE_SCHEMA,
            format!("unsupported schema_version {ver}"),
        ))
    }
}

pub fn json_to_document(obj: &Value) -> EngineResult<(Document, Vec<String>)> {
    let obj = migrate_document(obj)?;
    let report = validate_document(&obj);
    if !report.ok {
        return Err(first_validation_error(report));
    }
    let layer_values = obj["layers"]
        .as_array()
        .ok_or_else(|| invalid("layers must be an array"))?;
    if layer_values.iter().any(|l| l["type"] == "vector") {
        return Err(DrawToolError::new(
            CODE_UNSUPPORTED_LAYER_TYPE,
            "vector layers are not supported",
        ));
    }
    let c = &obj["canvas"];
    let mut layers = Vec::with_capacity(layer_values.len());
    for l in layer_values {
        let mut store = ChunkStore::new(
            c["width_px"].as_u64().unwrap_or(0) as u32,
            c["height_px"].as_u64().unwrap_or(0) as u32,
        )?;
        for chunk in l["raster"]["chunks"].as_array().into_iter().flatten() {
            let data = decode_chunk(chunk["png"].as_str().unwrap_or_default())?;
            if let Some(data) = data {
                store.put_chunk(
                    chunk["cx"].as_u64().unwrap_or(0) as u32,
                    chunk["cy"].as_u64().unwrap_or(0) as u32,
                    Some(&data[..]),
                )?;
            }
        }
        let blend = l["blend"].as_str().unwrap_or("normal");
        let blend = BLEND_MODES
            .iter()
            .copied()
            .find(|m| *m == blend)
            .ok_or_else(|| invalid("unknown blend mode"))?;
        layers.push(Layer::new(
            l["layer_id"].as_str().unwrap_or_default(),
            l["name"].as_str().unwrap_or("Layer"),
            l["visible"].as_bool().unwrap_or(true),
            l["locked"].as_bool().unwrap_or(false),
            l["opacity"].as_f64().unwrap_or(1.0),
            blend,
            store,
        )?);
    }
    let ids = layer_values
        .iter()
        .filter_map(|l| l["layer_id"].as_str())
        .collect::<Vec<_>>();
    let mut idgen = IdGen::new();
    idgen.seed(ids);
    let active = obj
        .get("active_layer_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            layer_values
                .last()
                .and_then(|l| l["layer_id"].as_str())
                .unwrap_or_default()
        });
    let atlas = obj
        .get("atlas")
        .filter(|value| !value.is_null())
        .map(|value| {
            serde_json::from_value::<crate::atlas::AtlasLayout>(value.clone())
                .map_err(|e| DrawToolError::new(CODE_SCHEMA, format!("invalid atlas layout: {e}")))
        })
        .transpose()?;
    let mut doc = Document::new(DocumentSpec {
        id: obj["document_id"].as_str().unwrap_or_default().into(),
        name: obj
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Untitled")
            .into(),
        canvas: from_canvas(c),
        layers,
        active_layer_id: active.into(),
        ids: idgen,
    })?;
    // Keep a structurally valid authoring layout even if later layer edits
    // removed one of its source ids. The document must remain openable so the
    // user can repair that slot; export validates references before baking.
    doc.atlas = atlas;
    Ok((doc, Vec::new()))
}
pub fn import_layer_json(
    doc: &mut Document,
    obj: &Value,
) -> EngineResult<(Layer, usize, Vec<String>)> {
    let obj = migrate_document(obj)?;
    let report = validate_layer_file(&obj);
    if !report.ok {
        return Err(first_validation_error(report));
    }
    let v = &obj["layer"];
    if v["type"] == "vector" {
        return Err(DrawToolError::new(
            CODE_UNSUPPORTED_LAYER_TYPE,
            "vector layers are not supported",
        ));
    }
    let mut store = ChunkStore::new(doc.canvas.width_px, doc.canvas.height_px)?;
    let mut dropped = 0;
    for chunk in v["raster"]["chunks"].as_array().into_iter().flatten() {
        let cx = chunk["cx"].as_u64().unwrap_or(0) as u32;
        let cy = chunk["cy"].as_u64().unwrap_or(0) as u32;
        let data = decode_chunk(chunk["png"].as_str().unwrap_or_default())?;
        if (cx as u64) * (CHUNK_PX as u64) >= doc.canvas.width_px as u64
            || (cy as u64) * (CHUNK_PX as u64) >= doc.canvas.height_px as u64
        {
            dropped += 1;
        } else if let Some(data) = data {
            store.put_chunk(cx, cy, Some(&data[..]))?;
        }
    }
    let blend = BLEND_MODES
        .iter()
        .copied()
        .find(|m| *m == v["blend"].as_str().unwrap_or("normal"))
        .ok_or_else(|| invalid("unknown blend mode"))?;
    let layer = Layer::new(
        doc.ids.next(),
        v["name"].as_str().unwrap_or("Layer"),
        v["visible"].as_bool().unwrap_or(true),
        v["locked"].as_bool().unwrap_or(false),
        v["opacity"].as_f64().unwrap_or(1.0),
        blend,
        store,
    )?;
    let src = &obj["source_canvas"];
    let warns =
        if src["width_px"] != doc.canvas.width_px || src["height_px"] != doc.canvas.height_px {
            vec![format!(
                "source canvas {}x{} differs from document {}x{}",
                src["width_px"], src["height_px"], doc.canvas.width_px, doc.canvas.height_px
            )]
        } else {
            vec![]
        };
    Ok((layer, dropped, warns))
}
#[cfg(test)]
mod tests;
