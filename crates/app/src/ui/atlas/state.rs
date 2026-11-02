//! Atlas dialog state: the layout draft plus the pending grid-import payload.

use std::collections::BTreeSet;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use pixel_engine::atlas::{AtlasCellSpec, AtlasLayout, AtlasMode, GridSliceOptions};
use pixel_engine::io::png::PngImage;
use pixel_engine::model::{Document, SetAtlasLayoutCommand};

use crate::session_glue::AppState;

#[derive(Debug, Clone)]
pub struct AtlasState {
    pub layout: AtlasLayout,
    /// Snapshot used to notice undo/redo or document replacement while the dialog is open.
    pub(super) persisted_layout: Option<AtlasLayout>,
    /// Cell index with the red outside outline.
    pub focus: Option<usize>,
    /// Multi-selected cells (the blue inside outline).
    pub selected: BTreeSet<usize>,
    pub error: Option<String>,
    /// Dialog owner closes after the user jumps from a layer cell to editing.
    pub close_requested: bool,
    /// Pending metadata-free PNG import, kept in memory until the user applies it.
    pub grid_import: Option<GridImportState>,
    pub(super) export_task: Option<AtlasExportTask>,
}

#[derive(Debug, Clone)]
pub(super) struct AtlasExportTask {
    pub progress: Arc<Mutex<AtlasExportProgress>>,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Default)]
pub(super) struct AtlasExportProgress {
    pub done_cells: usize,
    pub total_cells: usize,
    pub finished: Option<Result<(), String>>,
}

impl AtlasExportTask {
    pub fn new(cancel: Arc<AtomicBool>) -> Self {
        Self {
            progress: Arc::new(Mutex::new(AtlasExportProgress::default())),
            cancel,
        }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridImportState {
    pub image: PngImage,
    pub options: GridSliceOptions,
    pub cache_key: u64,
    pub confirm_replace: bool,
}

impl Default for AtlasState {
    fn default() -> Self {
        Self {
            layout: AtlasLayout {
                mode: AtlasMode::Layers,
                columns: 1,
                rows: None,
                cell_w: 1,
                cell_h: 1,
                margin: 0,
                padding: 0,
                preset_all: true,
                slots: Vec::new(),
            },
            persisted_layout: None,
            focus: None,
            selected: BTreeSet::new(),
            error: None,
            close_requested: false,
            grid_import: None,
            export_task: None,
        }
    }
}

impl AtlasState {
    pub fn from_app(app: &AppState) -> Self {
        let Some(doc) = app.session.doc() else {
            return Self::default();
        };
        let persisted_layout = doc.atlas.clone();
        let layout = persisted_layout.clone().unwrap_or_else(|| {
            let mut layout = AtlasLayout::from_document(doc);
            layout.preset_all = false;
            layout.slots = vec![AtlasCellSpec::composed(
                "Slot 1",
                doc.layers.iter().map(|layer| layer.id.clone()).collect(),
            )];
            layout
        });
        Self {
            layout,
            persisted_layout,
            focus: Some(0),
            selected: BTreeSet::new(),
            error: None,
            close_requested: false,
            grid_import: None,
            export_task: None,
        }
    }

    /// Re-read the document (after import or a document switch).
    pub fn rebuild(&mut self, app: &AppState) {
        *self = Self::from_app(app);
    }

    pub fn cells(&self, doc: &Document) -> Vec<AtlasCellSpec> {
        self.layout.cells(doc)
    }

    /// Keep focus inside the current cell list.
    pub fn clamp_focus(&mut self, count: usize) {
        if count == 0 {
            self.focus = None;
            self.selected.clear();
            return;
        }
        if self.focus.is_none_or(|f| f >= count) {
            self.focus = Some(count - 1);
        }
        self.selected.retain(|&i| i < count);
    }

    /// Snap a document canvas dimension to the engine's 32px grid. Documents
    /// already satisfy this alignment; this is used by the original-size shortcut.
    pub fn snap_cell(value: u32) -> u32 {
        (value.max(32).div_ceil(32) * 32).min(4096)
    }

    /// The preset "all layers" spec, bottom → top.
    pub fn all_layers(&self, doc: &Document) -> AtlasCellSpec {
        AtlasLayout::all_layers_spec(doc)
    }

    /// Resolve the focused layer-mode cell to its document layer id.
    pub fn focused_layer_id(&self, doc: &Document) -> Option<String> {
        self.focus
            .and_then(|index| self.layout.cells(doc).get(index).cloned())
            .and_then(|cell| cell.layer_ids.first().cloned())
            .filter(|id| doc.find_layer(id).is_some())
    }

    /// Persist the dialog's draft only after an explicit confirmation.
    pub fn commit_layout(&mut self, app: &mut AppState) -> Result<(), String> {
        let doc = app
            .session
            .doc()
            .ok_or_else(|| "문서가 없습니다.".to_string())?;
        self.layout
            .validate(doc)
            .map_err(|error| format!("레이아웃 오류: {}", error.message))?;
        let after = Some(self.layout.clone());
        if self.persisted_layout == after {
            return Ok(());
        }
        app.session
            .execute(SetAtlasLayoutCommand::new(
                self.persisted_layout.clone(),
                after.clone(),
            ))
            .map_err(|error| format!("레이아웃 저장 실패: {}", error.message))?;
        self.persisted_layout = after;
        Ok(())
    }
}
