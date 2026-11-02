//! Patch JSON accessors and the chunk-store pixel sink used by edit operations.

use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::model::Document;
use pixel_engine::raster::PixelSink;

pub(super) struct StoreSink<'a>(pub(super) &'a mut pixel_engine::chunkstore::ChunkStore);

impl PixelSink for StoreSink<'_> {
    fn set(&mut self, x: i32, y: i32, packed: u32) {
        if x >= 0 && y >= 0 {
            self.0.set_pixel(x as u32, y as u32, packed);
        }
    }

    fn get(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 {
            0
        } else {
            self.0.get_pixel(x as u32, y as u32)
        }
    }
}

pub(super) fn check_fields(value: &serde_json::Value, allowed: &[&str]) -> EngineResult<()> {
    let object = value
        .as_object()
        .ok_or_else(|| DrawToolError::invalid_state("patch values must be JSON objects"))?;
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(DrawToolError::invalid_state(format!(
            "unknown patch field {key:?}"
        )));
    }
    Ok(())
}

pub(super) fn required_str<'a>(value: &'a serde_json::Value, field: &str) -> EngineResult<&'a str> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| DrawToolError::invalid_state(format!("{field} must be a non-empty string")))
}

pub(super) fn required_i32(value: &serde_json::Value, field: &str) -> EngineResult<i32> {
    value
        .get(field)
        .and_then(serde_json::Value::as_i64)
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| DrawToolError::invalid_state(format!("{field} must be a 32-bit integer")))
}

pub(super) fn optional_i32(value: &serde_json::Value, field: &str) -> EngineResult<Option<i32>> {
    value
        .get(field)
        .map(|_| required_i32(value, field))
        .transpose()
}

pub(super) fn required_usize(value: &serde_json::Value, field: &str) -> EngineResult<usize> {
    value
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| {
            DrawToolError::invalid_state(format!("{field} must be a non-negative integer"))
        })
}

pub(super) fn required_rgba(value: &serde_json::Value, field: &str) -> EngineResult<[u8; 4]> {
    optional_rgba(value, field)?
        .ok_or_else(|| DrawToolError::invalid_state(format!("{field} must be [r,g,b,a]")))
}

pub(super) fn optional_rgba(
    value: &serde_json::Value,
    field: &str,
) -> EngineResult<Option<[u8; 4]>> {
    let Some(raw) = value.get(field) else {
        return Ok(None);
    };
    let channels = raw
        .as_array()
        .filter(|a| a.len() == 4)
        .ok_or_else(|| DrawToolError::invalid_state(format!("{field} must be [r,g,b,a]")))?;
    let mut rgba = [0; 4];
    for (i, channel) in channels.iter().enumerate() {
        rgba[i] = u8::try_from(channel.as_u64().ok_or_else(|| {
            DrawToolError::invalid_state(format!("{field}[{i}] must be 0..=255"))
        })?)
        .map_err(|_| DrawToolError::out_of_range(format!("{field}[{i}] must be 0..=255")))?;
    }
    Ok(Some(rgba))
}

pub(super) fn optional_i32_pair(
    value: &serde_json::Value,
    field: &str,
) -> EngineResult<Option<(i32, i32)>> {
    let Some(raw) = value.get(field) else {
        return Ok(None);
    };
    let a = raw
        .as_array()
        .filter(|a| a.len() == 2)
        .ok_or_else(|| DrawToolError::invalid_state(format!("{field} must be [x,y]")))?;
    let pair = |n: &serde_json::Value| {
        n.as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(|| {
                DrawToolError::invalid_state(format!("{field} values must be 32-bit integers"))
            })
    };
    Ok(Some((pair(&a[0])?, pair(&a[1])?)))
}

pub(super) fn optional_i32_quad(
    value: &serde_json::Value,
    field: &str,
) -> EngineResult<Option<[i32; 4]>> {
    let Some(raw) = value.get(field) else {
        return Ok(None);
    };
    let a = raw
        .as_array()
        .filter(|a| a.len() == 4)
        .ok_or_else(|| DrawToolError::invalid_state(format!("{field} must be [x,y,w,h]")))?;
    let mut out = [0; 4];
    for (i, n) in a.iter().enumerate() {
        out[i] = n
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(|| {
                DrawToolError::invalid_state(format!("{field} values must be 32-bit integers"))
            })?;
    }
    Ok(Some(out))
}

pub(super) fn optional_points(
    value: &serde_json::Value,
    field: &str,
) -> EngineResult<Option<Vec<[i32; 2]>>> {
    let Some(raw) = value.get(field) else {
        return Ok(None);
    };
    let points = raw
        .as_array()
        .ok_or_else(|| DrawToolError::invalid_state("points must be an array of [x,y]"))?;
    let mut out = Vec::with_capacity(points.len());
    for point in points {
        let pair = point
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or_else(|| DrawToolError::invalid_state("each point must be [x,y]"))?;
        let x = pair[0]
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(|| {
                DrawToolError::invalid_state("point coordinates must be 32-bit integers")
            })?;
        let y = pair[1]
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(|| {
                DrawToolError::invalid_state("point coordinates must be 32-bit integers")
            })?;
        out.push([x, y]);
    }
    Ok(Some(out))
}

pub(super) fn resolve_layer<'a>(doc: &'a Document, selector: &str) -> EngineResult<&'a str> {
    if let Some(layer) = doc.find_layer(selector) {
        return Ok(&layer.id);
    }
    let mut matches = doc.layers.iter().filter(|layer| layer.name == selector);
    let first = matches
        .next()
        .ok_or_else(|| DrawToolError::layer_not_found(selector))?;
    if matches.next().is_some() {
        return Err(DrawToolError::invalid_state(format!(
            "layer name {selector:?} is ambiguous; use layer id"
        )));
    }
    Ok(&first.id)
}
