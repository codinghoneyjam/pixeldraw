//! Atlas-specific file operations: sheet export, dtAt reopen, and grid slicing.

use pixel_engine::atlas::{
    AtlasCellSpec, AtlasLayout, AtlasMetadata, AtlasMode, AtlasSheet, GridSliceOptions,
    SlicedAtlasCell, slice_png_grid,
};
use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::io::png::PngImage;
use pixel_engine::model::NewDocumentOptions;
use pixel_engine::pixel::pack_rgba_bytes;

use crate::session_glue::AppState;

/// Replace the current document with one editable layer per selected grid cell.
///
/// Pixel contents are copied at their original coordinates. When a cell is
/// smaller than the editor's 32px chunk minimum, the new canvas is padded on
/// the right and bottom; the atlas cell size follows that editable canvas so
/// exporting does not resample the imported pixels.
pub fn import_png_grid(
    state: &mut AppState,
    image: &PngImage,
    options: GridSliceOptions,
) -> EngineResult<()> {
    if image.metadata_chunk("dtAt").is_some() {
        return Err(DrawToolError::invalid_state(
            "PNG already contains Pixeldraw atlas metadata; open it directly to restore its cells",
        ));
    }
    let cells: Vec<SlicedAtlasCell> = slice_png_grid(image, options)?;
    let (canvas_w, canvas_h) = atlas_document_size(options.cell_w, options.cell_h)?;

    state.session.new_document(
        NewDocumentOptions::default()
            .size(canvas_w, canvas_h)
            .name("Imported Atlas"),
    )?;
    let mut layer_ids = vec![String::new(); cells.len()];
    for i in (0..cells.len()).rev() {
        let cell = &cells[i];
        let name = format!("Cell {:02}", cell.index + 1);
        let id = if i + 1 == cells.len() {
            let id = state.session.require_doc()?.active_layer_id.clone();
            state.session.rename_layer(&id, &name)?;
            id
        } else {
            state.session.add_layer(Some(&name))?
        };
        {
            let layer = state.session.doc_mut()?.get_layer_mut(&id)?;
            let mut src = 0usize;
            for y in 0..cell.height {
                for x in 0..cell.width {
                    let pixel = &cell.rgba[src..src + 4];
                    let packed = pack_rgba_bytes(pixel[0], pixel[1], pixel[2], pixel[3]);
                    if packed != 0 {
                        layer.store.set_pixel(x, y, packed);
                    }
                    src += 4;
                }
            }
        }
        layer_ids[i] = id;
    }
    if let Some(first) = layer_ids.first() {
        let _ = state.session.set_active_layer(first);
    }
    let layout = AtlasLayout {
        mode: AtlasMode::Layers,
        columns: options.columns,
        rows: Some(options.rows),
        cell_w: canvas_w,
        cell_h: canvas_h,
        margin: 0,
        padding: 0,
        preset_all: false,
        slots: Vec::new(),
    };
    layout.validate(state.session.require_doc()?)?;
    state.session.doc_mut()?.atlas = Some(layout);
    state.session.history_mut().clear();
    state.session.history_mut().mark_saved();
    state.after_document_replaced();
    state.drain_events();
    Ok(())
}

/// Bake the current document with `layout` and return the atlas PNG bytes.
pub fn export_atlas_bytes(state: &AppState, layout: &AtlasLayout) -> EngineResult<Vec<u8>> {
    let doc = state.session.require_doc()?;
    Ok(pixel_engine::atlas::export_atlas_png(doc, layout)?.bytes)
}

/// Bake the current document with `layout` for callers that also need the
/// metadata (the CLI's `--describe` path).
pub fn export_atlas_sheet(state: &AppState, layout: &AtlasLayout) -> EngineResult<AtlasSheet> {
    let doc = state.session.require_doc()?;
    pixel_engine::atlas::build_sheet(doc, layout)
}

/// Rebuild one document layer per atlas slot from an exported sheet.
///
/// The reopened canvas is the **cell**, not the whole sheet: each cell becomes
/// its own full-size layer, so the round trip returns to the same editing unit
/// the atlas was authored from. Metadata slots are stored sheet-first (top
/// first in layers mode), so layers are created bottom-first to keep the panel
/// order visually identical.
pub(super) fn open_atlas_image(
    state: &mut AppState,
    img: &PngImage,
    meta: &AtlasMetadata,
) -> EngineResult<()> {
    meta.validate()?;
    if meta.sheet.w != img.width || meta.sheet.h != img.height {
        return Err(DrawToolError::invalid_state(format!(
            "atlas sheet metadata says {}x{} but the PNG is {}x{}",
            meta.sheet.w, meta.sheet.h, img.width, img.height
        )));
    }
    if !meta.sheet_matches_grid() {
        return Err(DrawToolError::invalid_state(
            "atlas metadata grid does not match its sheet size",
        ));
    }
    let (cell_w, cell_h) = atlas_document_size(meta.cell.w, meta.cell.h)?;
    state
        .session
        .new_document(NewDocumentOptions::default().size(cell_w, cell_h))?;

    let mut layer_ids: Vec<String> = vec![String::new(); meta.slots.len()];
    for i in (0..meta.slots.len()).rev() {
        let slot = &meta.slots[i];
        let name = atlas_layer_name(&slot.name, i);
        let id = if i + 1 == meta.slots.len() {
            let id = state.session.require_doc()?.active_layer_id.clone();
            state.session.rename_layer(&id, &name)?;
            id
        } else {
            state.session.add_layer(Some(&name))?
        };

        let ix = i as u32 % meta.grid.cols;
        let iy = i as u32 / meta.grid.cols;
        let x0 = u64::from(meta.margin)
            + u64::from(ix) * (u64::from(meta.cell.w) + u64::from(meta.padding));
        let y0 = u64::from(meta.margin)
            + u64::from(iy) * (u64::from(meta.cell.h) + u64::from(meta.padding));
        if x0 < u64::from(img.width) && y0 < u64::from(img.height) {
            let (x0, y0) = (x0 as u32, y0 as u32);
            let rw = meta.cell.w.min(img.width - x0).min(cell_w);
            let rh = meta.cell.h.min(img.height - y0).min(cell_h);
            let doc = state.session.doc_mut()?;
            let layer = doc.get_layer_mut(&id)?;
            for y in 0..rh {
                for x in 0..rw {
                    let sx = x0 + x;
                    let sy = y0 + y;
                    let o = ((sy as usize * img.width as usize) + sx as usize) * 4;
                    let packed = pack_rgba_bytes(
                        img.rgba[o],
                        img.rgba[o + 1],
                        img.rgba[o + 2],
                        img.rgba[o + 3],
                    );
                    if packed != 0 {
                        layer.store.set_pixel(x, y, packed);
                    }
                }
            }
        }
        layer_ids[i] = id;
    }
    if let Some(first) = layer_ids.first() {
        let _ = state.session.set_active_layer(first);
    }
    let mut layout = AtlasLayout {
        mode: meta.mode,
        columns: meta.grid.cols,
        rows: Some(meta.grid.rows),
        cell_w: meta.cell.w,
        cell_h: meta.cell.h,
        margin: meta.margin,
        padding: meta.padding,
        preset_all: meta.version >= 2 && meta.preset_all,
        slots: Vec::new(),
    };
    if layout.mode == AtlasMode::Composed {
        let first_user_slot = usize::from(layout.preset_all);
        layout.slots = meta
            .slots
            .iter()
            .enumerate()
            .skip(first_user_slot)
            .map(|(i, slot)| AtlasCellSpec {
                name: slot.name.clone(),
                layer_ids: vec![layer_ids[i].clone()],
                merged: false,
            })
            .collect();
    }
    layout.validate(state.session.require_doc()?)?;
    state.session.doc_mut()?.atlas = Some(layout);
    state.session.history_mut().clear();
    state.session.history_mut().mark_saved();
    state.after_document_replaced();
    state.drain_events();
    Ok(())
}

/// Atlas cells are authored on the engine's 32px document grid. Round a
/// cell size up to that grid, clamped to the editor's canvas caps.
fn atlas_document_size(w: u32, h: u32) -> EngineResult<(u32, u32)> {
    use pixel_engine::constants::{CHUNK_PX, MAX_H_PX, MAX_W_PX, MIN_SIZE_PX};
    let align = |v: u32| v.max(1).div_ceil(CHUNK_PX) * CHUNK_PX;
    let cw = align(w).max(MIN_SIZE_PX);
    let ch = align(h).max(MIN_SIZE_PX);
    if cw > MAX_W_PX || ch > MAX_H_PX {
        return Err(DrawToolError::out_of_range(format!(
            "atlas cell {cw}x{ch} exceeds the {}x{} canvas cap",
            MAX_W_PX, MAX_H_PX
        )));
    }
    Ok((cw, ch))
}

fn atlas_layer_name(name: &str, index: usize) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return format!("Slot {}", index + 1);
    }
    let mut out = String::new();
    let mut units = 0usize;
    for ch in trimmed.chars() {
        let n = ch.len_utf16();
        if units + n > 128 {
            break;
        }
        units += n;
        out.push(ch);
    }
    if out.trim().is_empty() {
        format!("Slot {}", index + 1)
    } else {
        out
    }
}

#[cfg(test)]
mod tests;
