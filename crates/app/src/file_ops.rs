//! Document file operations — new/open/save, PNG/JSON (dialog-free core).
//
// The native pickers live in `platform` and the buttons in `shell`; the
// atlas-specific sheet/grid paths live in the `atlas` submodule.

mod atlas;

use pixel_engine::atlas::AtlasMetadata;
use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::io::export::{FlattenOptions, flatten_to_rgba};
use pixel_engine::io::png::{decode_png_large, encode_png};
use pixel_engine::io::serialize::{
    document_to_json, import_layer_json, json_to_document, layer_to_json,
};
use pixel_engine::io::validate::{validate_document, validate_layer_file};
use pixel_engine::model::{Document, NewDocumentOptions};
use pixel_engine::pixel::pack_rgba_bytes;

use crate::session_glue::AppState;

pub use atlas::{export_atlas_bytes, export_atlas_sheet, import_png_grid};

/// Metadata payload written into exported atlases (`dtAt` PNG chunk).
pub use pixel_engine::atlas::{ATLAS_META_VERSION, AtlasGrid, AtlasSize, AtlasSlotMeta};

/// Blank document at `w,h`, view refit.
pub fn new_document(state: &mut AppState, w: u32, h: u32) -> EngineResult<()> {
    new_document_with_bg(state, w, h, "transparent")
}

/// Blank document at `w,h` with background color, view refit.
pub fn new_document_with_bg(
    state: &mut AppState,
    w: u32,
    h: u32,
    background: &str,
) -> EngineResult<()> {
    let bg = match background {
        "transparent" => "transparent",
        s if s.starts_with('#') && s.len() == 7 => s,
        _ => "transparent",
    };
    state
        .session
        .new_document(NewDocumentOptions::default().size(w, h).background(bg))?;
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
///
/// Compact, not pretty: PERF-3 measured 9806 KB → 7455 KB (−24%) on the busy
/// bench document, and the stringify itself is under 1% of the save (per-chunk
/// PNG encoding dominates). Both `serde_json` and the JS parser read either
/// form, so save compatibility is unchanged. (JS writes files through its own
/// `stringifyReadableJson`; that readable port has not happened yet — the
/// whitespace difference is cosmetic and both sides parse each other fine.)
pub fn save_json_bytes(state: &AppState) -> EngineResult<Vec<u8>> {
    let doc = state.session.require_doc()?;
    let value = document_to_json(doc)?;
    serde_json::to_vec(&value)
        .map_err(|e| DrawToolError::invalid_state(format!("document JSON encode failed: {e}")))
}

/// Serialize one editable raster layer as a standalone `draw_tool.layer` file.
pub fn export_layer_json_bytes(state: &AppState, layer_id: &str) -> EngineResult<Vec<u8>> {
    let doc = state.session.require_doc()?;
    let value = layer_to_json(doc, layer_id)?;
    serde_json::to_vec(&value)
        .map_err(|e| DrawToolError::invalid_state(format!("layer JSON encode failed: {e}")))
}

/// Import a standalone layer without dropping pixels when its source canvas is
/// larger than the open document. The destination expands to contain both.
pub fn import_layer_json_bytes(state: &mut AppState, bytes: &[u8]) -> EngineResult<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| DrawToolError::invalid_state(format!("layer file is not JSON: {e}")))?;
    let report = validate_layer_file(&value);
    if !report.ok {
        let first = report
            .errors
            .first()
            .map(|e| e.message.as_str())
            .unwrap_or("invalid layer file");
        return Err(DrawToolError::invalid_state(format!(
            "layer file failed validation: {first}"
        )));
    }

    let (target_w, target_h, layer) = {
        let current = state.session.require_doc()?;
        if current.layers.len() >= pixel_engine::constants::MAX_LAYERS {
            return Err(DrawToolError::out_of_range(
                "document already has the maximum number of layers",
            ));
        }
        let source = &value["source_canvas"];
        let width = source["width_px"].as_u64().unwrap_or(0) as u32;
        let height = source["height_px"].as_u64().unwrap_or(0) as u32;
        let target_w = current.canvas.width_px.max(width);
        let target_h = current.canvas.height_px.max(height);

        // Prepare and validate the imported layer against the final canvas
        // before changing the live session or its undo history.
        let mut prepared_doc: Document = current.clone();
        prepared_doc.set_canvas_size(target_w, target_h)?;
        let (layer, dropped, _warnings) = import_layer_json(&mut prepared_doc, &value)?;
        if dropped != 0 {
            return Err(DrawToolError::invalid_state(format!(
                "layer import would drop {dropped} chunks outside the destination canvas"
            )));
        }
        (target_w, target_h, layer)
    };

    state.session.resize_canvas(target_w, target_h)?;
    // `import_layer_json` advanced a prepared copy's id counter. Advance the
    // live counter too so the next layer cannot reuse the imported id.
    state.session.doc_mut()?.ids.seed([layer.id.as_str()]);
    state.session.insert_layer(layer, None)
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
///
/// The large decoder is used because an atlas sheet may exceed the editor's
/// document caps; ordinary oversized PNGs are still rejected by
/// [`png_canvas_size`] when they reach the flat path.
pub fn open_png_bytes(state: &mut AppState, bytes: &[u8]) -> EngineResult<()> {
    let img = decode_png_large(bytes)?;
    if let Some(raw) = img.metadata_chunk("dtAt") {
        let meta = AtlasMetadata::from_json_bytes(raw)?;
        return atlas::open_atlas_image(state, &img, &meta);
    }
    open_flat_png(state, &img)
}

fn open_flat_png(state: &mut AppState, img: &pixel_engine::io::png::PngImage) -> EngineResult<()> {
    let Some((canvas_w, canvas_h)) = png_canvas_size(img.width, img.height) else {
        return Err(DrawToolError::invalid_state(format!(
            "PNG size {}x{} cannot fit a {}x{} canvas",
            img.width,
            img.height,
            pixel_engine::constants::MAX_W_PX,
            pixel_engine::constants::MAX_H_PX
        )));
    };
    state
        .session
        .new_document(NewDocumentOptions::default().size(canvas_w, canvas_h))?;
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

/// Canvas size for an imported PNG: round up to the engine's 32px chunk grid.
///
/// The document canvas is chunk-aligned, but real PNGs (title art in
/// `asset_work`, for example) are not. Padding the canvas to the next chunk
/// keeps every source pixel at its original coordinate instead of rejecting
/// the file. Returns `None` when the padded size would exceed the caps.
fn png_canvas_size(w: u32, h: u32) -> Option<(u32, u32)> {
    use pixel_engine::constants::{CHUNK_PX, MAX_H_PX, MAX_W_PX, MIN_SIZE_PX};
    let align = |v: u32| v.max(1).div_ceil(CHUNK_PX) * CHUNK_PX;
    let (cw, ch) = (align(w).max(MIN_SIZE_PX), align(h).max(MIN_SIZE_PX));
    (cw <= MAX_W_PX && ch <= MAX_H_PX).then_some((cw, ch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_with_transparent_bg() {
        let mut state = AppState::new();
        new_document_with_bg(&mut state, 128, 128, "transparent").unwrap();
        let doc = state.session.doc().unwrap();
        assert_eq!(doc.canvas.background, "transparent");
    }

    #[test]
    fn new_document_with_white_bg() {
        let mut state = AppState::new();
        new_document_with_bg(&mut state, 128, 128, "#ffffff").unwrap();
        let doc = state.session.doc().unwrap();
        assert_eq!(doc.canvas.background, "#ffffff");
    }

    #[test]
    fn new_document_rejects_invalid_bg() {
        let mut state = AppState::new();
        new_document_with_bg(&mut state, 128, 128, "invalid").unwrap();
        let doc = state.session.doc().unwrap();
        assert_eq!(doc.canvas.background, "transparent");
    }

    #[test]
    fn png_import_pads_to_the_chunk_grid() {
        let (w, h) = (40u32, 40u32);
        let mut rgba = vec![0u8; w as usize * h as usize * 4];
        let o = (4 * w as usize + 3) * 4;
        rgba[o..o + 4].copy_from_slice(&[255, 0, 0, 255]);
        let bytes = encode_png(&rgba, w, h).unwrap();

        let mut state = AppState::new();
        open_png_bytes(&mut state, &bytes).unwrap();
        let doc = state.session.require_doc().unwrap();
        assert_eq!((doc.canvas.width_px, doc.canvas.height_px), (64, 64));
        assert_eq!(
            doc.layers[0].store.get_pixel(3, 4),
            pixel_engine::pixel::pack_rgba(255.0, 0.0, 0.0, 255.0)
        );
        assert_eq!(doc.layers[0].store.get_pixel(50, 50), 0);
    }

    #[test]
    fn every_asset_work_png_imports() {
        fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect(&path, out);
                } else if path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
                {
                    out.push(path);
                }
            }
        }

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../asset_work");
        let mut pngs = Vec::new();
        collect(&root, &mut pngs);
        pngs.sort();
        assert!(!pngs.is_empty(), "asset_work PNG corpus is missing");
        let mut state = AppState::new();
        for path in &pngs {
            let bytes =
                std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            open_png_bytes(&mut state, &bytes)
                .unwrap_or_else(|e| panic!("import {}: {e}", path.display()));
        }
    }

    #[test]
    fn layer_json_round_trip_expands_canvas_without_losing_pixels_or_properties() {
        let mut source = AppState::new();
        new_document_with_bg(&mut source, 64, 64, "transparent").unwrap();
        let source_id = source
            .session
            .require_doc()
            .unwrap()
            .active_layer_id
            .clone();
        source
            .session
            .stroke_begin(
                pixel_engine::model::edit_session::EditOptions::on_layer(&source_id)
                    .labeled("test"),
            )
            .unwrap();
        source
            .session
            .stroke_with(|writer| {
                writer.set(
                    47,
                    39,
                    pixel_engine::pixel::pack_rgba_bytes(17, 99, 231, 127),
                );
            })
            .unwrap();
        source.session.stroke_commit(Some("test")).unwrap();
        {
            let layer = source
                .session
                .doc_mut()
                .unwrap()
                .get_layer_mut(&source_id)
                .unwrap();
            layer.visible = false;
            layer.locked = true;
            layer.opacity = 0.375;
            layer.blend = "multiply";
        }
        let bytes = export_layer_json_bytes(&source, &source_id).unwrap();

        let mut destination = AppState::new();
        new_document_with_bg(&mut destination, 32, 32, "transparent").unwrap();
        let imported_id = import_layer_json_bytes(&mut destination, &bytes).unwrap();
        let doc = destination.session.require_doc().unwrap();
        assert_eq!((doc.canvas.width_px, doc.canvas.height_px), (64, 64));
        let imported = doc.find_layer(&imported_id).unwrap();
        assert_eq!(imported.store.get_pixel(47, 39), 0x7fe76311);
        assert!(!imported.visible);
        assert!(imported.locked);
        assert_eq!(imported.opacity, 0.375);
        assert_eq!(imported.blend, "multiply");
        assert!(doc.find_layer(&doc.ids.clone().next()).is_none());
    }
}
