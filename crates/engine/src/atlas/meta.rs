//! `dtAt` metadata — the sheet descriptor stored in the atlas PNG.
//!
//! Version 2 adds the authoring mode, the preset flag and per-slot layer ids on
//! top of the v1 shape. Version 3 adds sheet margin and cell padding. Earlier
//! files default both values to zero.
//! Version 1 files (names + provenance only) still parse:
//! a missing `mode` means [`AtlasMode::Layers`] and a missing `layer_ids` means
//! the importer can only restore baked pixels, which is exactly what v1 held.

use serde::{Deserialize, Serialize};

use super::{ATLAS_MAX_CELLS, AtlasMode};
use crate::errors::{DrawToolError, EngineResult};

/// Current metadata version written by this build.
pub const ATLAS_META_VERSION: u32 = 3;
/// Oldest metadata version this build still reads.
pub const ATLAS_META_MIN_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasSize {
    pub w: u32,
    pub h: u32,
}

impl AtlasSize {
    pub fn new(w: u32, h: u32) -> Self {
        Self { w, h }
    }

    pub fn pixels(self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasGrid {
    pub cols: u32,
    pub rows: u32,
}

impl AtlasGrid {
    pub fn new(cols: u32, rows: u32) -> Self {
        Self { cols, rows }
    }
}

/// One sheet cell, in sheet order (top-first for layers, preset-first for
/// composed sheets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AtlasSlotMeta {
    pub name: String,
    #[serde(default)]
    pub merged: bool,
    /// Source layer names, in application order, kept for provenance.
    #[serde(default)]
    pub sources: Vec<String>,
    /// Source layer ids, in application order. Empty on v1 payloads.
    #[serde(default)]
    pub layer_ids: Vec<String>,
}

/// The `dtAt` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtlasMetadata {
    pub version: u32,
    #[serde(default)]
    pub mode: AtlasMode,
    pub sheet: AtlasSize,
    pub cell: AtlasSize,
    pub grid: AtlasGrid,
    /// Transparent border around the outside of the sheet.
    #[serde(default)]
    pub margin: u32,
    /// Transparent spacing between cells.
    #[serde(default)]
    pub padding: u32,
    /// True when slot 1 is the automatic "all layers" composition.
    #[serde(default)]
    pub preset_all: bool,
    pub slots: Vec<AtlasSlotMeta>,
}

impl AtlasMetadata {
    /// Reject payloads that cannot describe the sheet they travelled with.
    pub fn validate(&self) -> EngineResult<()> {
        if !(ATLAS_META_MIN_VERSION..=ATLAS_META_VERSION).contains(&self.version) {
            return Err(DrawToolError::invalid_state(format!(
                "unsupported atlas metadata version {}",
                self.version
            )));
        }
        if self.cell.w == 0 || self.cell.h == 0 {
            return Err(DrawToolError::invalid_state(
                "atlas cell size must be at least 1x1",
            ));
        }
        if self.margin > 4096 || self.padding > 4096 {
            return Err(DrawToolError::out_of_range(
                "atlas margin and padding must be 0..=4096 pixels",
            ));
        }
        if self.grid.cols == 0 || self.grid.rows == 0 {
            return Err(DrawToolError::invalid_state(
                "atlas grid must have at least one row and column",
            ));
        }
        let capacity = u64::from(self.grid.cols) * u64::from(self.grid.rows);
        if capacity > ATLAS_MAX_CELLS as u64 {
            return Err(DrawToolError::out_of_range(format!(
                "atlas grid holds at most {ATLAS_MAX_CELLS} cells"
            )));
        }
        if self.slots.is_empty() {
            return Err(DrawToolError::invalid_state(
                "atlas metadata holds no slots",
            ));
        }
        if self.slots.len() > ATLAS_MAX_CELLS {
            return Err(DrawToolError::out_of_range(format!(
                "atlas holds at most {ATLAS_MAX_CELLS} slots"
            )));
        }
        if self.slots.len() as u64 > capacity {
            return Err(DrawToolError::invalid_state(format!(
                "atlas grid has room for {capacity} cells but metadata contains {}",
                self.slots.len()
            )));
        }
        let expect_w = sheet_axis(self.grid.cols, self.cell.w, self.margin, self.padding);
        let expect_h = sheet_axis(self.grid.rows, self.cell.h, self.margin, self.padding);
        if expect_w != Some(self.sheet.w) || expect_h != Some(self.sheet.h) {
            return Err(DrawToolError::invalid_state(format!(
                "atlas sheet {}x{} does not match {}x{} cells of {}x{}",
                self.sheet.w,
                self.sheet.h,
                self.grid.cols,
                self.grid.rows,
                self.cell.w,
                self.cell.h
            )));
        }
        Ok(())
    }

    /// True when the stored sheet size agrees with grid, margin and padding.
    pub fn sheet_matches_grid(&self) -> bool {
        sheet_axis(self.grid.cols, self.cell.w, self.margin, self.padding) == Some(self.sheet.w)
            && sheet_axis(self.grid.rows, self.cell.h, self.margin, self.padding)
                == Some(self.sheet.h)
    }

    pub fn to_json_bytes(&self) -> EngineResult<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| DrawToolError::invalid_state(format!("atlas metadata encode failed: {e}")))
    }

    pub fn from_json_bytes(bytes: &[u8]) -> EngineResult<Self> {
        let meta: Self = serde_json::from_slice(bytes).map_err(|e| {
            DrawToolError::invalid_state(format!("atlas metadata is not JSON: {e}"))
        })?;
        meta.validate()?;
        Ok(meta)
    }

    pub fn from_value(value: &serde_json::Value) -> EngineResult<Self> {
        let meta: Self = serde_json::from_value(value.clone()).map_err(|e| {
            DrawToolError::invalid_state(format!("atlas metadata is not valid: {e}"))
        })?;
        meta.validate()?;
        Ok(meta)
    }

    pub fn to_value(&self) -> EngineResult<serde_json::Value> {
        serde_json::to_value(self)
            .map_err(|e| DrawToolError::invalid_state(format!("atlas metadata encode failed: {e}")))
    }
}

fn sheet_axis(count: u32, cell: u32, margin: u32, padding: u32) -> Option<u32> {
    u32::try_from(
        u64::from(margin) * 2
            + u64::from(count) * u64::from(cell)
            + u64::from(count.saturating_sub(1)) * u64::from(padding),
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_payload_reads_as_layers_mode() {
        let raw = br#"{
            "version": 1,
            "sheet": {"w": 128, "h": 64},
            "cell": {"w": 64, "h": 64},
            "grid": {"cols": 2, "rows": 1},
            "slots": [
                {"name": "Top", "merged": false, "sources": ["Top"]},
                {"name": "Layer 1", "merged": false, "sources": ["Layer 1"]}
            ]
        }"#;
        let meta = AtlasMetadata::from_json_bytes(raw).unwrap();
        assert_eq!(meta.mode, AtlasMode::Layers);
        assert!(!meta.preset_all);
        assert_eq!(meta.slots.len(), 2);
        assert!(meta.slots[0].layer_ids.is_empty());
        assert!(meta.sheet_matches_grid());
    }

    #[test]
    fn v2_payload_round_trips() {
        let raw = br#"{
            "version": 2,
            "mode": "composed",
            "sheet": {"w": 512, "h": 512},
            "cell": {"w": 256, "h": 256},
            "grid": {"cols": 2, "rows": 2},
            "preset_all": true,
            "slots": [{"name": "All Layers", "merged": true, "sources": ["A", "B"], "layer_ids": ["layer-1", "layer-2"]}]
        }"#;
        let meta = AtlasMetadata::from_json_bytes(raw).unwrap();
        assert_eq!(meta.version, 2);
        assert_eq!(meta.margin, 0);
        assert_eq!(meta.padding, 0);
        assert_eq!(meta.mode, AtlasMode::Composed);
        assert!(meta.preset_all);
    }

    #[test]
    fn v3_payload_round_trips_margin_and_padding() {
        let meta = AtlasMetadata {
            version: ATLAS_META_VERSION,
            mode: AtlasMode::Composed,
            sheet: AtlasSize::new(144, 144),
            cell: AtlasSize::new(64, 64),
            grid: AtlasGrid::new(2, 2),
            margin: 4,
            padding: 8,
            preset_all: true,
            slots: vec![AtlasSlotMeta {
                name: "All Layers".into(),
                merged: true,
                sources: vec!["A".into(), "B".into()],
                layer_ids: vec!["layer-1".into(), "layer-2".into()],
            }],
        };
        let bytes = meta.to_json_bytes().unwrap();
        assert_eq!(AtlasMetadata::from_json_bytes(&bytes).unwrap(), meta);
    }

    #[test]
    fn an_unknown_version_is_rejected() {
        let raw = br#"{
            "version": 99,
            "sheet": {"w": 32, "h": 32},
            "cell": {"w": 32, "h": 32},
            "grid": {"cols": 1, "rows": 1},
            "slots": [{"name": "A"}]
        }"#;
        assert!(AtlasMetadata::from_json_bytes(raw).is_err());
    }
}
