//! Document file operations — new/open/save, PNG/JSON (dialog-free core).
//!
//! The native pickers live in `platform` and the buttons in `shell`; the
//! functions here take bytes and paths so they stay headless-testable. PNG
//! opens become a fresh document (JS parity for "open"; "import as layer" is
//! Phase 3 UI).

use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::io::export::{FlattenOptions, flatten_to_rgba};
use pixel_engine::io::png::{decode_png, encode_png};
use pixel_engine::io::serialize::{document_to_json, json_to_document};
use pixel_engine::io::validate::validate_document;
use pixel_engine::model::NewDocumentOptions;
use pixel_engine::pixel::pack_rgba_bytes;

use crate::session_glue::AppState;

/// Blank document at `w,h`, view refit.
pub fn new_document(state: &mut AppState, w: u32, h: u32) -> EngineResult<()> {
    state
        .session
        .new_document(NewDocumentOptions::default().size(w, h))?;
    state.after_document_replaced();
    // The queue carries the replacement; drain it now so one frame is enough.
    state.drain_events();
    Ok(())
}

/// Parse, validate and install a saved document.
pub fn open_json_bytes(state: &mut AppState, bytes: &[u8]) -> EngineResult<()> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| DrawToolError::invalid_state(format!("document is not JSON: {e}")))?;
    let report = validate_document(&value);
    if !report.ok {
        let first = report
            .errors
            .first()
            .map(|e| e.message.clone())
            .unwrap_or_default();
        return Err(DrawToolError::invalid_state(format!(
            "document failed validation: {first}"
        )));
    }
    let (doc, _warnings) = json_to_document(&value)?;
    state.session.load_document(doc, true)?;
    state.after_document_replaced();
    state.drain_events();
    Ok(())
}

/// Serialize the current document (key order pinned by 1.13).
pub fn save_json_bytes(state: &AppState) -> EngineResult<Vec<u8>> {
    let doc = state.session.require_doc()?;
    let value = document_to_json(doc)?;
    serde_json::to_vec_pretty(&value)
        .map_err(|e| DrawToolError::invalid_state(format!("document JSON encode failed: {e}")))
}

/// Flatten and encode the current canvas as PNG.
pub fn export_png_bytes(state: &AppState) -> EngineResult<Vec<u8>> {
    let doc = state.session.require_doc()?;
    let flat = flatten_to_rgba(
        &doc.layer_sources(),
        doc.canvas.width_px,
        doc.canvas.height_px,
        FlattenOptions {
            include_background: true,
        },
    );
    encode_png(&flat, doc.canvas.width_px, doc.canvas.height_px)
}

/// Decode a PNG file into a fresh document, pixels stamped in one stroke.
pub fn open_png_bytes(state: &mut AppState, bytes: &[u8]) -> EngineResult<()> {
    let img = decode_png(bytes)?;
    if !pixel_engine::constants::is_valid_canvas_size(img.width, img.height) {
        return Err(DrawToolError::invalid_state(format!(
            "PNG size {}x{} is not a valid canvas",
            img.width, img.height
        )));
    }
    state
        .session
        .new_document(NewDocumentOptions::default().size(img.width, img.height))?;
    let (w, h) = (img.width as usize, img.height as usize);
    state.session.stroke_begin(
        pixel_engine::model::edit_session::EditOptions::on_active_layer().labeled("PNG 가져오기"),
    )?;
    let result = state.session.stroke_with(|writer| {
        for y in 0..h {
            for x in 0..w {
                let o = (y * w + x) * 4;
                let packed = pack_rgba_bytes(
                    img.rgba[o],
                    img.rgba[o + 1],
                    img.rgba[o + 2],
                    img.rgba[o + 3],
                );
                if packed != 0 {
                    writer.set(x as u32, y as u32, packed);
                }
            }
        }
    });
    if let Err(e) = result {
        state.session.stroke_cancel();
        return Err(e);
    }
    state.session.stroke_commit(Some("PNG 가져오기"))?;
    state.after_document_replaced();
    state.drain_events();
    Ok(())
}
