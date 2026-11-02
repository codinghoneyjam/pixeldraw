//! Sheet rasterisation and export.
//!
//! Every cell is flattened from a [`AtlasCellSpec`]'s ordered layer list, then
//! resampled once to the cell size. The sheet is the row-major concatenation of
//! those cells, so the same bytes feed both the PNG encoder and the GUI preview
//! cache.

use crate::errors::{DrawToolError, EngineResult};
use crate::io::export::{FlattenOptions, flatten_to_rgba};
use crate::io::png::encode_png_large_with_chunks;
use crate::model::Document;

use super::{
    ATLAS_MAX_CELLS, ATLAS_MAX_PX, AtlasCellSpec, AtlasGrid, AtlasLayout, AtlasMetadata, AtlasMode,
    AtlasSize, AtlasSlotMeta,
};

/// One fully rendered sheet plus the metadata that travels with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlasSheet {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, row-major RGBA.
    pub rgba: Vec<u8>,
    pub metadata: AtlasMetadata,
}

/// A baked PNG and the descriptor written into its `dtAt` chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlasExport {
    pub bytes: Vec<u8>,
    pub metadata: AtlasMetadata,
}

/// A single cell, already flattened and resampled to the atlas cell size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlasCellRgba {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl AtlasCellRgba {
    pub fn is_empty(&self) -> bool {
        self.rgba.iter().all(|&b| b == 0)
    }
}

/// Flatten one cell's layers in the given bottom → top order.
///
/// Hidden layers and zero-opacity layers contribute nothing, exactly as they do
/// on the canvas: the same `flatten_to_rgba` path is used, so atlas pixels and
/// screen pixels cannot disagree. The finished canvas is resampled to
/// `cell_w × cell_h` with nearest-neighbour sampling and centred (aspect-fit)
/// letterboxing.
pub fn build_cell_rgba(
    doc: &Document,
    cell: &AtlasCellSpec,
    cell_w: u32,
    cell_h: u32,
) -> EngineResult<AtlasCellRgba> {
    if cell_w == 0 || cell_h == 0 {
        return Err(DrawToolError::out_of_range(
            "atlas cell size must be at least 1x1",
        ));
    }
    let mut sources = Vec::with_capacity(cell.layer_ids.len());
    for id in &cell.layer_ids {
        let layer = doc.get_layer(id)?;
        sources.push(layer.as_source());
    }
    let flat = flatten_to_rgba(
        &sources,
        doc.canvas.width_px,
        doc.canvas.height_px,
        FlattenOptions {
            include_background: true,
        },
    );
    let rgba = fit_rgba(
        &flat,
        doc.canvas.width_px,
        doc.canvas.height_px,
        cell_w,
        cell_h,
    );
    Ok(AtlasCellRgba {
        name: cell.name.clone(),
        width: cell_w,
        height: cell_h,
        rgba,
    })
}

/// Fit `src` into `dst_w × dst_h` preserving aspect ratio, nearest-neighbour.
///
/// The unused margin stays transparent. When the aspect ratios already match —
/// the default atlas sizing — this is a straight uniform scale.
pub fn fit_rgba(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (dst_w as usize) * (dst_h as usize) * 4];
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return out;
    }
    let scale_x = dst_w as f64 / src_w as f64;
    let scale_y = dst_h as f64 / src_h as f64;
    let scale = scale_x.min(scale_y);
    let used_w = ((src_w as f64 * scale).round() as u32).clamp(1, dst_w);
    let used_h = ((src_h as f64 * scale).round() as u32).clamp(1, dst_h);
    let ox = (dst_w - used_w) / 2;
    let oy = (dst_h - used_h) / 2;
    let src_stride = src_w as usize * 4;
    let dst_stride = dst_w as usize * 4;
    for dy in 0..used_h {
        // Pixel-centre sampling keeps the nearest-neighbour grid symmetric.
        let sy = (((dy as f64 + 0.5) * src_h as f64 / used_h as f64) as u32).min(src_h - 1);
        let src_row = sy as usize * src_stride;
        let dst_row = (oy + dy) as usize * dst_stride + ox as usize * 4;
        for dx in 0..used_w {
            let sx = (((dx as f64 + 0.5) * src_w as f64 / used_w as f64) as u32).min(src_w - 1);
            let s = src_row + sx as usize * 4;
            let d = dst_row + dx as usize * 4;
            out[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    out
}

/// Place cell buffers row-major into one sheet.
pub fn assemble_sheet_rgba(
    cell_w: u32,
    cell_h: u32,
    cols: u32,
    cells: &[Vec<u8>],
) -> EngineResult<(u32, u32, Vec<u8>)> {
    if cols == 0 || cells.is_empty() {
        return Err(DrawToolError::invalid_state(
            "atlas needs at least one cell and one column",
        ));
    }
    let rows = (cells.len() as u32).div_ceil(cols);
    assemble_sheet_rgba_grid(cell_w, cell_h, cols, rows, cells)
}

/// Assemble cells into an explicitly sized grid. Unused cells remain
/// transparent, which lets the author keep a stable frame-sheet footprint.
pub fn assemble_sheet_rgba_grid(
    cell_w: u32,
    cell_h: u32,
    cols: u32,
    rows: u32,
    cells: &[Vec<u8>],
) -> EngineResult<(u32, u32, Vec<u8>)> {
    assemble_sheet_rgba_grid_with_spacing(cell_w, cell_h, cols, rows, 0, 0, cells)
}

/// Assemble cells with a transparent outer margin and transparent gaps.
pub fn assemble_sheet_rgba_grid_with_spacing(
    cell_w: u32,
    cell_h: u32,
    cols: u32,
    rows: u32,
    margin: u32,
    padding: u32,
    cells: &[Vec<u8>],
) -> EngineResult<(u32, u32, Vec<u8>)> {
    if cols == 0 || rows == 0 || cells.is_empty() {
        return Err(DrawToolError::invalid_state(
            "atlas needs at least one cell and a non-empty grid",
        ));
    }
    if cols as usize > ATLAS_MAX_CELLS {
        return Err(DrawToolError::out_of_range(format!(
            "atlas holds at most {ATLAS_MAX_CELLS} columns"
        )));
    }
    let capacity = u64::from(cols) * u64::from(rows);
    if capacity > ATLAS_MAX_CELLS as u64 || cells.len() as u64 > capacity {
        return Err(DrawToolError::out_of_range(format!(
            "atlas grid capacity {capacity} cannot hold {} cells (maximum {ATLAS_MAX_CELLS})",
            cells.len()
        )));
    }
    let sheet_w = u64::from(margin) * 2
        + u64::from(cols) * u64::from(cell_w)
        + u64::from(cols - 1) * u64::from(padding);
    let sheet_h = u64::from(margin) * 2
        + u64::from(rows) * u64::from(cell_h)
        + u64::from(rows - 1) * u64::from(padding);
    if sheet_w > u64::from(u32::MAX) || sheet_h > u64::from(u32::MAX) {
        return Err(DrawToolError::out_of_range(
            "atlas sheet dimensions overflow",
        ));
    }
    let (sheet_w, sheet_h) = (sheet_w as u32, sheet_h as u32);
    let expected = cell_w as usize * cell_h as usize * 4;
    for (i, cell) in cells.iter().enumerate() {
        if cell.len() != expected {
            return Err(DrawToolError::invalid_state(format!(
                "atlas cell {} has {} bytes, expected {expected}",
                i + 1,
                cell.len()
            )));
        }
    }
    let mut sheet = vec![0u8; sheet_w as usize * sheet_h as usize * 4];
    let dst_stride = sheet_w as usize * 4;
    let cell_stride = cell_w as usize * 4;
    for (i, cell) in cells.iter().enumerate() {
        let ox =
            margin as usize * 4 + (i as u32 % cols) as usize * (cell_stride + padding as usize * 4);
        let oy =
            margin as usize + (i as u32 / cols) as usize * (cell_h as usize + padding as usize);
        for y in 0..cell_h as usize {
            let src = y * cell_stride;
            let dst = (oy + y) * dst_stride + ox;
            sheet[dst..dst + cell_stride].copy_from_slice(&cell[src..src + cell_stride]);
        }
    }
    Ok((sheet_w, sheet_h, sheet))
}

/// Render every cell a layout describes and assemble the sheet.
pub fn build_sheet(doc: &Document, layout: &AtlasLayout) -> EngineResult<AtlasSheet> {
    build_sheet_with_progress(doc, layout, |_, _| true)
}

/// Render a sheet while reporting completed cells. Return `false` to cancel
/// between cells; large-sheet callers can keep their UI responsive and abort
/// before the PNG encoder allocates its output buffer.
pub fn build_sheet_with_progress(
    doc: &Document,
    layout: &AtlasLayout,
    mut continue_export: impl FnMut(usize, usize) -> bool,
) -> EngineResult<AtlasSheet> {
    layout.validate(doc)?;
    let specs = layout.cells(doc);
    let (sheet_w, sheet_h) = layout.sheet_size(doc);
    let mut cells = Vec::with_capacity(specs.len());
    for (index, spec) in specs.iter().enumerate() {
        if !continue_export(index, specs.len()) {
            return Err(DrawToolError::invalid_state("atlas export cancelled"));
        }
        cells.push(build_cell_rgba(doc, spec, layout.cell_w, layout.cell_h)?.rgba);
        if !continue_export(index + 1, specs.len()) {
            return Err(DrawToolError::invalid_state("atlas export cancelled"));
        }
    }
    let (width, height, rgba) = assemble_sheet_rgba_grid_with_spacing(
        layout.cell_w,
        layout.cell_h,
        layout.columns,
        layout.rows(doc),
        layout.margin,
        layout.padding,
        &cells,
    )?;
    debug_assert_eq!((width, height), (sheet_w, sheet_h));
    let metadata = metadata_for(doc, layout, &specs, sheet_w, sheet_h);
    Ok(AtlasSheet {
        width,
        height,
        rgba,
        metadata,
    })
}

/// Build the `dtAt` descriptor that matches the cells on the sheet.
pub fn metadata_for(
    doc: &Document,
    layout: &AtlasLayout,
    specs: &[AtlasCellSpec],
    sheet_w: u32,
    sheet_h: u32,
) -> AtlasMetadata {
    let rows = layout.rows(doc);
    AtlasMetadata {
        version: super::meta::ATLAS_META_VERSION,
        mode: layout.mode,
        sheet: AtlasSize::new(sheet_w, sheet_h),
        cell: AtlasSize::new(layout.cell_w, layout.cell_h),
        grid: AtlasGrid::new(layout.columns, rows),
        margin: layout.margin,
        padding: layout.padding,
        preset_all: layout.mode == AtlasMode::Composed && layout.preset_all,
        slots: specs
            .iter()
            .map(|spec| AtlasSlotMeta {
                name: spec.name.clone(),
                merged: spec.merged,
                sources: spec
                    .layer_ids
                    .iter()
                    .filter_map(|id| doc.find_layer(id).map(|layer| layer.name.clone()))
                    .collect(),
                layer_ids: spec.layer_ids.clone(),
            })
            .collect(),
    }
}

/// Render the sheet and encode it as a PNG carrying `dtAt` metadata.
pub fn export_atlas_png(doc: &Document, layout: &AtlasLayout) -> EngineResult<AtlasExport> {
    let sheet = build_sheet(doc, layout)?;
    encode_sheet(sheet)
}

/// Render and encode a sheet, reporting cell progress and honoring
/// cancellation between cells and before PNG encoding.
pub fn export_atlas_png_with_progress(
    doc: &Document,
    layout: &AtlasLayout,
    continue_export: impl FnMut(usize, usize) -> bool,
) -> EngineResult<AtlasExport> {
    let sheet = build_sheet_with_progress(doc, layout, continue_export)?;
    encode_sheet(sheet)
}

fn encode_sheet(sheet: AtlasSheet) -> EngineResult<AtlasExport> {
    let json = sheet.metadata.to_json_bytes()?;
    let bytes =
        encode_png_large_with_chunks(&sheet.rgba, sheet.width, sheet.height, &[("dtAt", &json)])?;
    Ok(AtlasExport {
        bytes,
        metadata: sheet.metadata,
    })
}

/// Encode an already-built RGBA buffer through the atlas pixel budget.
pub fn export_png_large(rgba: &[u8], width: u32, height: u32) -> EngineResult<Vec<u8>> {
    if u64::from(width) * u64::from(height) > ATLAS_MAX_PX {
        return Err(DrawToolError::out_of_range(format!(
            "atlas image {width}x{height} exceeds the atlas pixel budget ({ATLAS_MAX_PX})"
        )));
    }
    crate::io::png::encode_png_large(rgba, width, height)
}
