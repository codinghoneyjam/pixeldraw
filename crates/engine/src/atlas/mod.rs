//! Atlas authoring model — a layer grid, ordered composition slots, and the
//! sheet/`dtAt` payload shared by the GUI and the headless CLI.
//!
//! The atlas has two views over the same document:
//!
//! * [`AtlasMode::Layers`] — one cell per layer, in panel order (top first).
//! * [`AtlasMode::Composed`] — one cell per *slot*; a slot is an ordered list
//!   of layers (bottom → top) that is flattened into one image.
//!
//! A sheet is always `columns × rows` uniform cells. The default cell size
//! clamps each canvas axis to [`ATLAS_DEFAULT_CELL_MAX`], so a 2048×1088
//! document opens as 512×512 and a 4×4 sheet is exactly 2048×2048. Because
//! document canvases are already 32px-aligned, a clamped axis stays a valid
//! document size, which is what makes the atlas → PNG → document round trip
//! land on the same cell grid.

mod meta;
mod sheet;
mod slice;
#[cfg(test)]
mod tests;

pub use meta::{
    ATLAS_META_MIN_VERSION, ATLAS_META_VERSION, AtlasGrid, AtlasMetadata, AtlasSize, AtlasSlotMeta,
};
pub use sheet::{
    AtlasCellRgba, AtlasExport, AtlasSheet, assemble_sheet_rgba, assemble_sheet_rgba_grid,
    assemble_sheet_rgba_grid_with_spacing, build_cell_rgba, build_sheet, build_sheet_with_progress,
    export_atlas_png, export_atlas_png_with_progress, export_png_large, metadata_for,
};
pub use slice::{GridSliceOptions, SlicedAtlasCell, slice_png_grid};

use crate::errors::{DrawToolError, EngineResult};
use crate::model::Document;

/// One cell per document layer is supported across the full document layer cap.
/// The separate atlas pixel budget still bounds the size of the exported sheet.
pub const ATLAS_MAX_CELLS: usize = 64;
/// One composition slot can stack at most this many layers.
pub const ATLAS_MAX_LAYERS_PER_CELL: usize = 64;
/// Default per-cell ceiling; cell dimensions are the canvas clamped to this.
pub const ATLAS_DEFAULT_CELL_MAX: u32 = 512;
/// Hard pixel budget for atlas PNGs. The editor's document cap stays at
/// `MAX_W_PX × MAX_H_PX`; only the atlas export/import path may exceed it.
pub const ATLAS_MAX_PX: u64 = 64 * 1024 * 1024;

/// Which set of images a sheet contains.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum AtlasMode {
    /// One cell per document layer.
    #[default]
    Layers,
    /// One cell per composition slot.
    Composed,
}

impl AtlasMode {
    pub const ALL: [Self; 2] = [Self::Layers, Self::Composed];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Layers => "layers",
            Self::Composed => "composed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "layers" | "layer" | "grid" => Some(Self::Layers),
            "composed" | "compose" | "slots" => Some(Self::Composed),
            _ => None,
        }
    }
}

/// One cell of a sheet: a name plus the layers that make it, bottom → top.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtlasCellSpec {
    pub name: String,
    /// Layer ids in application order (bottom → top).
    pub layer_ids: Vec<String>,
    /// True when more than one source layer was baked into this cell.
    pub merged: bool,
}

impl AtlasCellSpec {
    pub fn single(name: impl Into<String>, layer_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            layer_ids: vec![layer_id.into()],
            merged: false,
        }
    }

    pub fn composed(name: impl Into<String>, layer_ids: Vec<String>) -> Self {
        let merged = layer_ids.len() > 1;
        Self {
            name: name.into(),
            layer_ids,
            merged,
        }
    }
}

/// Everything the atlas view needs besides the document itself.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtlasLayout {
    pub mode: AtlasMode,
    /// 1..=64 columns.
    pub columns: u32,
    /// Optional fixed row count. `None` grows rows to fit the cells.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u32>,
    pub cell_w: u32,
    pub cell_h: u32,
    /// Transparent border around the outside of the exported sheet.
    #[serde(default)]
    pub margin: u32,
    /// Transparent gap between neighbouring cells.
    #[serde(default)]
    pub padding: u32,
    /// When true, slot 1 is an automatic "all layers" composition. It is not
    /// stored in [`Self::slots`], so turning it off never loses user slots.
    pub preset_all: bool,
    /// User slots, in sheet order. Ignored in [`AtlasMode::Layers`].
    pub slots: Vec<AtlasCellSpec>,
}

impl AtlasLayout {
    /// The atlas default for `doc`: layers view, near-square grid, 512 cap.
    pub fn from_document(doc: &Document) -> Self {
        let (cell_w, cell_h) = default_cell_size(doc.canvas.width_px, doc.canvas.height_px);
        let count = doc.layers.len().clamp(1, ATLAS_MAX_CELLS);
        Self {
            mode: AtlasMode::Layers,
            columns: best_columns(count),
            rows: None,
            cell_w,
            cell_h,
            margin: 0,
            padding: 0,
            preset_all: true,
            slots: Vec::new(),
        }
    }

    /// The automatic first slot: every layer, bottom → top.
    pub fn all_layers_spec(doc: &Document) -> AtlasCellSpec {
        AtlasCellSpec::composed(
            "All Layers",
            doc.layers.iter().map(|layer| layer.id.clone()).collect(),
        )
    }

    /// The cells this layout would write, in sheet order.
    ///
    /// In layers mode the order matches the layer panel (top first). In
    /// composed mode the preset slot (when enabled) is first, then user slots.
    pub fn cells(&self, doc: &Document) -> Vec<AtlasCellSpec> {
        match self.mode {
            AtlasMode::Layers => doc
                .layers
                .iter()
                .rev()
                .map(|layer| AtlasCellSpec::single(&layer.name, &layer.id))
                .collect(),
            AtlasMode::Composed => {
                let mut cells = Vec::with_capacity(self.slots.len() + 1);
                if self.preset_all {
                    cells.push(Self::all_layers_spec(doc));
                }
                cells.extend(self.slots.iter().cloned());
                cells
            }
        }
    }

    pub fn cell_count(&self, doc: &Document) -> usize {
        self.cells(doc).len()
    }

    pub fn rows(&self, doc: &Document) -> u32 {
        if let Some(rows) = self.rows {
            return rows;
        }
        let count = self.cell_count(doc).max(1) as u32;
        count.div_ceil(self.columns.clamp(1, ATLAS_MAX_CELLS as u32))
    }

    /// `(width, height)` of the sheet at native cell resolution.
    pub fn sheet_size(&self, doc: &Document) -> (u32, u32) {
        let columns = self.columns.clamp(1, ATLAS_MAX_CELLS as u32);
        let rows = self.rows(doc);
        let axis = |count: u32, cell: u32| {
            (u64::from(self.margin) * 2
                + u64::from(count) * u64::from(cell.max(1))
                + u64::from(count.saturating_sub(1)) * u64::from(self.padding))
            .min(u64::from(u32::MAX)) as u32
        };
        (axis(columns, self.cell_w), axis(rows, self.cell_h))
    }

    /// Reject layouts that cannot be built or encoded.
    pub fn validate(&self, doc: &Document) -> EngineResult<()> {
        if !(1..=ATLAS_MAX_CELLS as u32).contains(&self.columns) {
            return Err(DrawToolError::out_of_range(format!(
                "atlas columns must be 1..={ATLAS_MAX_CELLS}"
            )));
        }
        if let Some(rows) = self.rows
            && !(1..=ATLAS_MAX_CELLS as u32).contains(&rows)
        {
            return Err(DrawToolError::out_of_range(format!(
                "atlas rows must be 1..={ATLAS_MAX_CELLS}"
            )));
        }
        if self.cell_w == 0 || self.cell_h == 0 {
            return Err(DrawToolError::out_of_range(
                "atlas cell size must be at least 1x1",
            ));
        }
        if self.margin > 4096 || self.padding > 4096 {
            return Err(DrawToolError::out_of_range(
                "atlas margin and padding must be 0..=4096 pixels",
            ));
        }
        if self.mode == AtlasMode::Layers && doc.layers.len() > ATLAS_MAX_CELLS {
            return Err(DrawToolError::out_of_range(format!(
                "layer atlas has {} layers; select at most {ATLAS_MAX_CELLS} layers",
                doc.layers.len()
            )));
        }
        if self.mode == AtlasMode::Composed && self.slots.len() > ATLAS_MAX_CELLS {
            return Err(DrawToolError::out_of_range(format!(
                "atlas holds at most {ATLAS_MAX_CELLS} user slots"
            )));
        }
        let cells = self.cells(doc);
        if cells.is_empty() {
            return Err(DrawToolError::invalid_state(
                "atlas needs at least one cell",
            ));
        }
        if cells.len() > ATLAS_MAX_CELLS {
            return Err(DrawToolError::out_of_range(format!(
                "atlas holds at most {ATLAS_MAX_CELLS} cells"
            )));
        }
        let rows = self.rows(doc);
        let capacity = u64::from(self.columns) * u64::from(rows);
        if capacity > ATLAS_MAX_CELLS as u64 {
            return Err(DrawToolError::out_of_range(format!(
                "atlas grid holds at most {ATLAS_MAX_CELLS} cells"
            )));
        }
        if cells.len() as u64 > capacity {
            return Err(DrawToolError::out_of_range(format!(
                "atlas grid has room for {capacity} cells but contains {}",
                cells.len()
            )));
        }
        for cell in &cells {
            if cell.layer_ids.len() > ATLAS_MAX_LAYERS_PER_CELL {
                return Err(DrawToolError::out_of_range(format!(
                    "atlas slot {} has more than {ATLAS_MAX_LAYERS_PER_CELL} layers",
                    cell.name
                )));
            }
            for id in &cell.layer_ids {
                if doc.find_layer(id).is_none() {
                    return Err(DrawToolError::layer_not_found(id));
                }
            }
        }
        if self.mode == AtlasMode::Composed {
            for cell in &cells {
                let name = cell.name.trim();
                if name.is_empty() || name.chars().count() > 64 {
                    return Err(DrawToolError::invalid_state(
                        "atlas slot names must contain 1..=64 characters",
                    ));
                }
            }
            for (i, cell) in cells.iter().enumerate() {
                if cells[..i]
                    .iter()
                    .any(|previous| previous.name.trim().eq_ignore_ascii_case(cell.name.trim()))
                {
                    return Err(DrawToolError::invalid_state(format!(
                        "atlas slot name {:?} is used more than once",
                        cell.name.trim()
                    )));
                }
            }
        }
        let (w, h) = self.sheet_size(doc);
        if w == 0 || h == 0 {
            return Err(DrawToolError::out_of_range("atlas sheet is empty"));
        }
        let px = u64::from(w) * u64::from(h);
        if px > ATLAS_MAX_PX {
            return Err(DrawToolError::out_of_range(format!(
                "atlas sheet {w}x{h} exceeds the atlas pixel budget ({ATLAS_MAX_PX})"
            )));
        }
        Ok(())
    }
}

/// Clamp each axis to [`ATLAS_DEFAULT_CELL_MAX`]. Never returns a zero
/// dimension. Every valid document canvas is 32px-aligned, so the result is
/// still a valid cell/document size (the UI snaps custom sizes too).
pub fn default_cell_size(canvas_w: u32, canvas_h: u32) -> (u32, u32) {
    (
        canvas_w.clamp(1, ATLAS_DEFAULT_CELL_MAX),
        canvas_h.clamp(1, ATLAS_DEFAULT_CELL_MAX),
    )
}

/// A near-square column count, capped by the atlas cell limit.
pub fn best_columns(count: usize) -> u32 {
    let n = count.clamp(1, ATLAS_MAX_CELLS);
    (n as f64).sqrt().ceil().clamp(1.0, n as f64) as u32
}

/// Layer ids resolved from user text: exact id first, then exact name, then a
/// case-insensitive name. An ambiguous or missing selector is an error.
pub fn resolve_layer<'a>(
    doc: &'a Document,
    selector: &str,
) -> EngineResult<&'a crate::model::Layer> {
    if let Some(layer) = doc.find_layer(selector) {
        return Ok(layer);
    }
    if let Some(layer) = doc.layers.iter().find(|layer| layer.name == selector) {
        return Ok(layer);
    }
    let matches: Vec<_> = doc
        .layers
        .iter()
        .filter(|layer| layer.name.eq_ignore_ascii_case(selector))
        .collect();
    match matches.len() {
        0 => Err(DrawToolError::layer_not_found(selector)),
        1 => Ok(matches[0]),
        _ => Err(DrawToolError::invalid_state(format!(
            "layer name {selector:?} is ambiguous"
        ))),
    }
}

/// Resolve every selector in order. Duplicates are preserved: a slot may
/// legitimately stack the same layer twice.
pub fn resolve_layer_ids(doc: &Document, selectors: &[String]) -> EngineResult<Vec<String>> {
    selectors
        .iter()
        .map(|selector| resolve_layer(doc, selector).map(|layer| layer.id.clone()))
        .collect()
}
