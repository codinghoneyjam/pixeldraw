//! Structured edit operations: apply a JSON patch to a plain document and
//! report the pixels each layer changed.
//!
//! The caller edits a private in-memory document and writes only after every
//! operation succeeds, so malformed late operations never leave a partial
//! output file.

mod json;

use std::collections::BTreeSet;

use pixel_engine::atlas::AtlasLayout;
use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::model::Document;
use pixel_engine::raster::{FillMode, ShapeKind, ShapeSpec, apply_shape};

use json::{
    StoreSink, check_fields, optional_i32, optional_i32_pair, optional_i32_quad, optional_points,
    optional_rgba, required_i32, required_rgba, required_str, required_usize, resolve_layer,
};

#[derive(Debug)]
pub(crate) struct EditSummary {
    pub(crate) operations_applied: usize,
    pub(crate) pixel_changes: Vec<serde_json::Value>,
}

pub(crate) fn apply_edit_patch(
    doc: &mut Document,
    patch: &serde_json::Value,
) -> EngineResult<EditSummary> {
    check_fields(patch, &["operations"])?;
    let operations = patch
        .get("operations")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| DrawToolError::invalid_state("patch.operations must be an array"))?;
    if operations.len() > 10_000 {
        return Err(DrawToolError::out_of_range(
            "patch may contain at most 10000 operations",
        ));
    }
    let mut working = doc.clone();
    for (index, operation) in operations.iter().enumerate() {
        if let Err(error) = apply_edit_operation(&mut working, operation) {
            return Err(DrawToolError::with_details(
                error.code,
                format!("operation {index}: {}", error.message),
                serde_json::json!({"operation_index": index}).to_string(),
            ));
        }
    }
    let pixel_changes = pixel_diff(doc, &working);
    *doc = working;
    Ok(EditSummary {
        operations_applied: operations.len(),
        pixel_changes,
    })
}

fn pixel_diff(before: &Document, after: &Document) -> Vec<serde_json::Value> {
    let mut changes = Vec::new();
    for before_layer in &before.layers {
        let Some(after_layer) = after.find_layer(&before_layer.id) else {
            continue;
        };
        let mut chunks = BTreeSet::new();
        before_layer.store.for_each_chunk(|cx, cy, _| {
            chunks.insert((cx, cy));
        });
        after_layer.store.for_each_chunk(|cx, cy, _| {
            chunks.insert((cx, cy));
        });

        let mut pixel_count = 0u64;
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0, 0);
        for (cx, cy) in chunks {
            let (a, b) = (
                before_layer.store.get_chunk(cx, cy),
                after_layer.store.get_chunk(cx, cy),
            );
            for local in 0..(pixel_engine::constants::CHUNK_PX as usize).pow(2) {
                let offset = local * 4;
                let changed = match (a, b) {
                    (Some(a), Some(b)) => a[offset..offset + 4] != b[offset..offset + 4],
                    (Some(a), None) => a[offset..offset + 4] != [0; 4],
                    (None, Some(b)) => b[offset..offset + 4] != [0; 4],
                    (None, None) => false,
                };
                if !changed {
                    continue;
                }
                pixel_count += 1;
                let x = cx * pixel_engine::constants::CHUNK_PX
                    + (local as u32 % pixel_engine::constants::CHUNK_PX);
                let y = cy * pixel_engine::constants::CHUNK_PX
                    + (local as u32 / pixel_engine::constants::CHUNK_PX);
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
        if pixel_count > 0 {
            changes.push(serde_json::json!({
                "layer_id": before_layer.id,
                "layer_name": after_layer.name,
                "pixels": pixel_count,
                "bounds": {
                    "x": min_x,
                    "y": min_y,
                    "width": max_x - min_x + 1,
                    "height": max_y - min_y + 1,
                },
            }));
        }
    }
    changes
}

fn apply_edit_operation(doc: &mut Document, operation: &serde_json::Value) -> EngineResult<()> {
    let op = required_str(operation, "op")?;
    match op {
        "set_pixel" => {
            check_fields(operation, &["op", "layer", "x", "y", "rgba"])?;
            let layer_id = resolve_layer(doc, required_str(operation, "layer")?)?.to_string();
            let x = required_i32(operation, "x")?;
            let y = required_i32(operation, "y")?;
            if x < 0 || y < 0 || x as u32 >= doc.canvas.width_px || y as u32 >= doc.canvas.height_px
            {
                return Err(DrawToolError::out_of_range(format!(
                    "pixel ({x},{y}) is outside {}x{} canvas",
                    doc.canvas.width_px, doc.canvas.height_px
                )));
            }
            let rgba = required_rgba(operation, "rgba")?;
            let layer = doc.get_layer_mut(&layer_id)?;
            if layer.locked {
                return Err(DrawToolError::layer_locked());
            }
            layer
                .store
                .set_pixel(x as u32, y as u32, u32::from_le_bytes(rgba));
            Ok(())
        }
        "draw_shape" => {
            check_fields(
                operation,
                &[
                    "op",
                    "layer",
                    "kind",
                    "stroke_width",
                    "fill",
                    "bbox",
                    "p0",
                    "p1",
                    "radius",
                    "points",
                    "primary",
                    "secondary",
                ],
            )?;
            let layer_id = resolve_layer(doc, required_str(operation, "layer")?)?.to_string();
            let kind = ShapeKind::parse(required_str(operation, "kind")?)?;
            let stroke_width = optional_i32(operation, "stroke_width")?.unwrap_or(1);
            let fill_mode = match operation
                .get("fill")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("outline")
            {
                "outline" => FillMode::Outline,
                "fill" => FillMode::Fill,
                "both" => FillMode::Both,
                other => {
                    return Err(DrawToolError::invalid_state(format!(
                        "unknown fill mode {other:?}"
                    )));
                }
            };
            let spec = ShapeSpec {
                kind,
                stroke_width,
                fill_mode,
                bbox: optional_i32_quad(operation, "bbox")?,
                p0: optional_i32_pair(operation, "p0")?,
                p1: optional_i32_pair(operation, "p1")?,
                radius: optional_i32(operation, "radius")?.unwrap_or(0),
                points: optional_points(operation, "points")?.unwrap_or_default(),
            };
            let primary = u32::from_le_bytes(required_rgba(operation, "primary")?);
            let secondary = optional_rgba(operation, "secondary")?
                .map(u32::from_le_bytes)
                .unwrap_or(primary);
            let layer = doc.get_layer_mut(&layer_id)?;
            if layer.locked {
                return Err(DrawToolError::layer_locked());
            }
            let mut sink = StoreSink(&mut layer.store);
            apply_shape(&mut sink, &spec, primary, secondary)?;
            Ok(())
        }
        "set_layer" => {
            check_fields(
                operation,
                &["op", "layer", "name", "visible", "locked", "opacity"],
            )?;
            if !["name", "visible", "locked", "opacity"]
                .iter()
                .any(|field| operation.get(field).is_some())
            {
                return Err(DrawToolError::invalid_state(
                    "set_layer needs at least one property",
                ));
            }
            let layer_id = resolve_layer(doc, required_str(operation, "layer")?)?.to_string();
            let layer = doc.get_layer_mut(&layer_id)?;
            if let Some(name) = operation.get("name") {
                let name = name
                    .as_str()
                    .ok_or_else(|| DrawToolError::invalid_state("name must be a string"))?;
                pixel_engine::model::layer::validate_layer_name(name)?;
                layer.name = name.to_string();
            }
            if let Some(visible) = operation.get("visible") {
                layer.visible = visible
                    .as_bool()
                    .ok_or_else(|| DrawToolError::invalid_state("visible must be a boolean"))?;
            }
            if let Some(locked) = operation.get("locked") {
                layer.locked = locked
                    .as_bool()
                    .ok_or_else(|| DrawToolError::invalid_state("locked must be a boolean"))?;
            }
            if let Some(opacity) = operation.get("opacity") {
                let opacity = opacity
                    .as_f64()
                    .ok_or_else(|| DrawToolError::invalid_state("opacity must be a number"))?;
                pixel_engine::model::layer::validate_layer_opacity(opacity)?;
                layer.opacity = opacity;
            }
            Ok(())
        }
        "move_layer" => {
            check_fields(operation, &["op", "layer", "to_index"])?;
            let layer_id = resolve_layer(doc, required_str(operation, "layer")?)?.to_string();
            let to = required_usize(operation, "to_index")?;
            if to >= doc.layers.len() {
                return Err(DrawToolError::out_of_range(format!(
                    "to_index {to} outside layer stack"
                )));
            }
            let from = doc
                .index_of(&layer_id)
                .ok_or_else(|| DrawToolError::layer_not_found(&layer_id))?;
            let layer = doc.layers.remove(from);
            doc.layers.insert(to, layer);
            Ok(())
        }
        "set_atlas" => {
            check_fields(operation, &["op", "layout"])?;
            let value = operation
                .get("layout")
                .ok_or_else(|| DrawToolError::invalid_state("set_atlas needs layout"))?;
            if value.is_null() {
                doc.atlas = None;
            } else {
                let layout: AtlasLayout = serde_json::from_value(value.clone()).map_err(|e| {
                    DrawToolError::invalid_state(format!("invalid atlas layout: {e}"))
                })?;
                layout.validate(doc)?;
                doc.atlas = Some(layout);
            }
            Ok(())
        }
        other => Err(DrawToolError::invalid_state(format!(
            "unknown edit operation {other:?}"
        ))),
    }
}
