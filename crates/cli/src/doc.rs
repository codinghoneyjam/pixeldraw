//! Shared document loading, summaries, and atomic-ish file writes.

use std::fs;
use std::path::Path;

use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::io::serialize::json_to_document;
use pixel_engine::model::Document;

pub(crate) fn doc_summary(doc: &Document) -> serde_json::Value {
    serde_json::json!({
        "id": doc.id,
        "name": doc.name,
        "width": doc.canvas.width_px,
        "height": doc.canvas.height_px,
        "background": doc.canvas.background,
        "layer_count": doc.layers.len(),
        "active_layer_id": doc.active_layer_id,
    })
}

pub(crate) fn layer_rows(doc: &Document) -> Vec<serde_json::Value> {
    doc.layers
        .iter()
        .rev()
        .map(|l| {
            serde_json::json!({
                "id": l.id,
                "name": l.name,
                "visible": l.visible,
                "locked": l.locked,
                "opacity": l.opacity,
                "blend": l.blend,
            })
        })
        .collect()
}

pub(crate) fn load_doc(path: &str) -> EngineResult<(Document, Vec<String>)> {
    let bytes = fs::read(path).map_err(|e| io_error(path, e))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| DrawToolError::invalid_state(format!("{path}: not JSON: {e}")))?;
    json_to_document(&value)
}

pub(crate) fn write_file(path: &str, bytes: &[u8]) -> EngineResult<()> {
    if let Some(parent) = Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|e| io_error(path, e))?;
    }
    fs::write(path, bytes).map_err(|e| io_error(path, e))
}

pub(crate) fn io_error(path: &str, e: std::io::Error) -> DrawToolError {
    DrawToolError::invalid_state(format!("{path}: {e}"))
}

pub(crate) fn need(args: &[String], n: usize, usage: &str) -> EngineResult<()> {
    if args.len() < n {
        return Err(DrawToolError::invalid_state(format!("usage: {usage}")));
    }
    Ok(())
}
