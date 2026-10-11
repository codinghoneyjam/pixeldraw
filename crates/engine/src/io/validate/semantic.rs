use super::structural::{
    Collector, LayerInfo, MAX_ERRORS, ValidationReport, check_canvas, check_layer, check_no_extra,
    is_obj,
};
use crate::errors::{
    CODE_ACTIVE_LAYER_MISSING, CODE_CHUNK_BAD_PNG, CODE_CHUNK_DUPLICATE, CODE_CHUNK_OUT_OF_CANVAS,
    CODE_DUP_LAYER_ID, CODE_SCHEMA,
};
use crate::io::{base64_to_bytes, png::decode_png};
use serde_json::Value;
const DOC_KEYS: &[&str] = &[
    "schema_version",
    "format",
    "document_id",
    "name",
    "canvas",
    "active_layer_id",
    "layers",
];
const FILE_KEYS: &[&str] = &["schema_version", "format", "source_canvas", "layer"];
fn is_array(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Array(_)))
}
fn semantic(
    chunks: &Value,
    flags: &[bool],
    dims: Option<(u32, u32)>,
    base: &str,
    c: &mut Collector,
) {
    let Some(items) = chunks.as_array() else {
        return;
    };
    let mut seen = std::collections::HashSet::new();
    for (i, ch) in items.iter().enumerate() {
        if !is_obj(ch) {
            continue;
        }
        let (Some(cx), Some(cy)) = (
            ch.get("cx").and_then(Value::as_i64),
            ch.get("cy").and_then(Value::as_i64),
        ) else {
            continue;
        };
        let key = (cx, cy);
        if !seen.insert(key) {
            c.push(
                CODE_CHUNK_DUPLICATE,
                format!("{base}/{i}"),
                format!("duplicate chunk ({cx},{cy})"),
            );
        }
        if let Some((w, h)) = dims
            && (cx * 32 >= i64::from(w) || cy * 32 >= i64::from(h))
        {
            c.push(
                CODE_CHUNK_OUT_OF_CANVAS,
                format!("{base}/{i}"),
                format!("chunk ({cx},{cy}) outside canvas"),
            );
        }
        if flags.get(i) == Some(&true) {
            let decoded = ch
                .get("png")
                .and_then(Value::as_str)
                .and_then(|s| base64_to_bytes(s).ok())
                .and_then(|b| decode_png(&b).ok());
            if !matches!(decoded,Some(ref image) if image.width==32 && image.height==32) {
                c.push(
                    CODE_CHUNK_BAD_PNG,
                    format!("{base}/{i}/png"),
                    "chunk png must decode to 32x32 RGBA".into(),
                );
            }
        }
        if c.len() >= MAX_ERRORS {
            break;
        }
    }
}
pub fn check_chunks_semantic(
    chunks: &Value,
    flags: &[bool],
    dims: Option<(u32, u32)>,
    base: &str,
    c: &mut Collector,
) {
    semantic(chunks, flags, dims, base, c);
}
#[allow(clippy::collapsible_if)]
pub fn validate_document(obj: &Value) -> ValidationReport {
    let mut c = Collector::new();
    if !is_obj(obj) {
        c.push(CODE_SCHEMA, "".into(), "document must be an object".into());
        return c.report();
    }
    check_no_extra(obj, DOC_KEYS, "", &mut c);
    for (key, want, msg) in [
        ("schema_version", "2.0.0", "schema_version must be 2.0.0"),
        (
            "format",
            "draw_tool.document",
            "format must be draw_tool.document",
        ),
    ] {
        if obj.get(key).and_then(Value::as_str) != Some(want) {
            c.push(CODE_SCHEMA, format!("/{key}"), msg.into());
        }
    }
    if !obj
        .get("document_id")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty())
    {
        c.push(
            CODE_SCHEMA,
            "/document_id".into(),
            "document_id must be a non-empty string".into(),
        );
    }
    if obj
        .get("name")
        .is_some_and(|v| !v.as_str().is_some_and(|s| s.encode_utf16().count() <= 128))
    {
        c.push(
            CODE_SCHEMA,
            "/name".into(),
            "name must be a string of at most 128 chars".into(),
        );
    }
    if obj
        .get("active_layer_id")
        .is_some_and(|v| !v.as_str().is_some_and(|s| !s.is_empty()))
    {
        c.push(
            CODE_SCHEMA,
            "/active_layer_id".into(),
            "active_layer_id must be a non-empty string".into(),
        );
    }
    let dims = if let Some(v) = obj.get("canvas") {
        check_canvas(v, "/canvas", &mut c).map(|d| (d.width_px, d.height_px))
    } else {
        c.push(CODE_SCHEMA, "/canvas".into(), "canvas is required".into());
        None
    };
    let mut infos = Vec::new();
    let mut layers_ok = false;
    if let Some(ls) = obj.get("layers").and_then(Value::as_array) {
        layers_ok = (1..=64).contains(&ls.len());
        if !layers_ok {
            c.push(
                CODE_SCHEMA,
                "/layers".into(),
                "layers must hold 1-64 entries".into(),
            );
        }
        for (i, l) in ls.iter().enumerate() {
            infos.push(check_layer(l, &format!("/layers/{i}"), &mut c));
        }
    } else {
        c.push(
            CODE_SCHEMA,
            "/layers".into(),
            "layers must be an array".into(),
        );
    }
    if layers_ok && c.len() < MAX_ERRORS {
        let ls = obj["layers"].as_array().unwrap();
        let mut ids = std::collections::HashSet::new();
        for (i, l) in ls.iter().enumerate() {
            if let Some(id) = l.get("layer_id").and_then(Value::as_str) {
                if !ids.insert(id) {
                    c.push(
                        CODE_DUP_LAYER_ID,
                        format!("/layers/{i}/layer_id"),
                        format!("duplicate layer id {id}"),
                    );
                }
            }
        }
        if let Some(id) = obj.get("active_layer_id").and_then(Value::as_str)
            && !ids.contains(id)
        {
            c.push(
                CODE_ACTIVE_LAYER_MISSING,
                "/active_layer_id".into(),
                format!("active layer {id} missing"),
            );
        }
        for (i, l) in ls.iter().enumerate() {
            if l.get("type").and_then(Value::as_str) == Some("raster")
                && is_obj(&l["raster"])
                && is_array(l["raster"].get("chunks"))
            {
                semantic(
                    &l["raster"]["chunks"],
                    &infos[i].structural_chunks,
                    dims,
                    &format!("/layers/{i}/raster/chunks"),
                    &mut c,
                );
                if c.len() >= MAX_ERRORS {
                    break;
                }
            }
        }
    }
    c.report()
}
pub fn validate_layer_file(obj: &Value) -> ValidationReport {
    let mut c = Collector::new();
    if !is_obj(obj) {
        c.push(
            CODE_SCHEMA,
            "".into(),
            "layer file must be an object".into(),
        );
        return c.report();
    }
    check_no_extra(obj, FILE_KEYS, "", &mut c);
    for (key, want, msg) in [
        ("schema_version", "2.0.0", "schema_version must be 2.0.0"),
        (
            "format",
            "draw_tool.layer",
            "format must be draw_tool.layer",
        ),
    ] {
        if obj.get(key).and_then(Value::as_str) != Some(want) {
            c.push(CODE_SCHEMA, format!("/{key}"), msg.into());
        }
    }
    let dims = obj
        .get("source_canvas")
        .and_then(|v| check_canvas(v, "/source_canvas", &mut c))
        .map(|d| (d.width_px, d.height_px));
    if obj.get("source_canvas").is_none() {
        c.push(
            CODE_SCHEMA,
            "/source_canvas".into(),
            "source_canvas is required".into(),
        );
    }
    let info: LayerInfo = if let Some(l) = obj.get("layer") {
        check_layer(l, "/layer", &mut c)
    } else {
        c.push(CODE_SCHEMA, "/layer".into(), "layer is required".into());
        LayerInfo {
            structural_chunks: vec![],
        }
    };
    if obj.get("layer").is_some_and(|l| {
        l.get("type").and_then(Value::as_str) == Some("raster")
            && is_obj(&l["raster"])
            && is_array(l["raster"].get("chunks"))
    }) {
        semantic(
            &obj["layer"]["raster"]["chunks"],
            &info.structural_chunks,
            dims,
            "/layer/raster/chunks",
            &mut c,
        );
    }
    c.report()
}
