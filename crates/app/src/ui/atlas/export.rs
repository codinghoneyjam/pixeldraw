//! Atlas PNG export (both save modes) and the save-file dialog.

use pixel_engine::atlas::AtlasMode;
use std::sync::atomic::Ordering;
use std::thread;

use crate::session_glue::AppState;

use super::note;
use super::state::{AtlasExportTask, AtlasState};

pub(super) fn note_export_complete(app: &mut AppState) {
    note(app, "아틀라스 PNG 저장됨");
}

pub(super) fn export_atlas(atlas: &mut AtlasState, app: &mut AppState, mode: AtlasMode) {
    atlas.error = None;
    let Some(doc) = app.session.doc() else {
        atlas.error = Some("문서가 없습니다.".into());
        return;
    };
    let mut layout = atlas.layout.clone();
    layout.mode = mode;
    if let Err(e) = layout.validate(doc) {
        atlas.error = Some(format!("레이아웃 오류: {}", e.message));
        return;
    }
    let stem = match mode {
        AtlasMode::Layers => "atlas_layers",
        AtlasMode::Composed => "atlas_slots",
    };
    let Some(path) = pixel_platform::pick_save_file(pixel_platform::FileKind::Png, stem) else {
        return;
    };
    let doc = doc.clone();
    let task = AtlasExportTask::new(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        false,
    )));
    let progress = std::sync::Arc::clone(&task.progress);
    let cancelled = task.cancel_flag();
    atlas.export_task = Some(task);
    atlas.layout.mode = mode;
    thread::spawn(move || {
        let result =
            pixel_engine::atlas::export_atlas_png_with_progress(&doc, &layout, |done, total| {
                if let Ok(mut progress) = progress.lock() {
                    progress.done_cells = done;
                    progress.total_cells = total;
                }
                !cancelled.load(Ordering::Relaxed)
            })
            .map_err(|error| error.message)
            .and_then(|export| {
                if cancelled.load(Ordering::Relaxed) {
                    return Err("cancelled".into());
                }
                pixel_platform::write_bytes_atomic(&path, &export.bytes)
                    .map_err(|error| error.to_string())
            });
        if let Ok(mut progress) = progress.lock() {
            progress.finished = Some(result);
        }
    });
}
