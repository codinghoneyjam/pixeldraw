//! Atlas workbench — layered grid, ordered composition slots, lazy previews,
//! and the two save modes.
//!
//! The heavy lifting lives in `pixel_engine::atlas`; this module is the editor
//! around it:
//!
//! * **Layers view** — one cell per document layer, top first.
//! * **Composed view** — one cell per slot. A slot is an ordered layer list
//!   (bottom → top), so the combination order is visible and editable. Slot 1
//!   can be the automatic "all layers" preset and can be turned off without
//!   losing user slots.
//! * **Previews** are baked lazily: at most [`PREVIEWS_PER_FRAME`] cells are
//!   flattened per frame and cached as GPU textures keyed by a layout salt.
//! * **Focus** is a red outline painted *outside* the focused cell, clipped to
//!   the sheet, so it can never cover the cell's own pixels.
//!
//! Save modes:
//! * [`AtlasMode::Layers`] writes every layer in its own cell (the "as-is"
//!   giant sheet).
//! * [`AtlasMode::Composed`] writes each ordered slot combination into a cell.
//!
//! Submodules keep the dialog readable:
//! `state` owns the draft, `layout_ui` edits the grid, `preview` paints the
//! sheet, `slots` edits composed cells, `import` slices metadata-free PNGs,
//! and `export` writes the PNG.

mod export;
mod import;
mod layout_ui;
mod preview;
mod slots;
mod state;
#[cfg(test)]
mod tests;

pub use state::AtlasState;

use pixel_engine::atlas::{AtlasLayout, AtlasMode};
use pixel_engine::io::png::decode_png_large;

use crate::file_ops::open_png_bytes;
use crate::session_glue::AppState;
use egui::{Color32, RichText, Ui, Vec2};

use export::export_atlas;
use import::{GridImportAction, apply_grid_import, grid_import_editor, grid_import_from_image};
use layout_ui::layout_controls;
use preview::preview;
use slots::composed_workspace;

/// How many cells may be flattened in one frame. A cell build flattens the
/// whole document, so this is deliberately small; the preview fills in over a
/// few frames instead of blocking the first paint.
const PREVIEWS_PER_FRAME: usize = 2;
const FOCUS_RED: Color32 = Color32::from_rgb(235, 64, 52);
const SELECT_BLUE: Color32 = Color32::from_rgb(0x00, 0x96, 0xff);
const GRID_GRAY: Color32 = Color32::from_gray(88);

/// Open the atlas as three synchronized native windows. Immediate viewports
/// share the draft on the UI thread, so a grid edit is visible in the preview
/// and selected-slot layer list during the same frame.
pub fn show_windows(ctx: &egui::Context, atlas: &mut AtlasState, app: &mut AppState) {
    let Some(doc) = app.session.doc().cloned() else {
        return;
    };
    if atlas.persisted_layout != doc.atlas {
        atlas.persisted_layout = doc.atlas.clone();
        atlas.layout = atlas
            .persisted_layout
            .clone()
            .unwrap_or_else(|| AtlasLayout::from_document(&doc));
    }
    if atlas.layout.mode != AtlasMode::Composed {
        atlas.layout.mode = AtlasMode::Composed;
    }
    atlas.clamp_focus(atlas.layout.cells(&doc).len());
    let mut should_close = false;
    let mut initial_layout = atlas.layout.clone();

    let grid_id = egui::ViewportId::from_hash_of("pixeldraw_atlas_grid_window");
    let grid_builder = egui::ViewportBuilder::default()
        .with_title("아틀라스 · 그리드 배치")
        .with_position([40.0, 60.0])
        .with_inner_size([480.0, 650.0])
        .with_min_inner_size([340.0, 380.0]);
    ctx.show_viewport_immediate(grid_id, grid_builder, |ctx, _| {
        if ctx.input(|i| i.viewport().close_requested()) {
            should_close = true;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("그리드 배치");
            ui.label("슬롯 위치와 시트 크기를 정합니다.");
            layout_controls(ui, atlas, &doc);
            ui.separator();
            slots::grid_window(ui, atlas, &doc);
            ui.separator();
            open_atlas_button(ui, atlas, app, &mut initial_layout);
            grid_import_panel(ui, atlas, app, &mut initial_layout);
        });
    });

    let preview_id = egui::ViewportId::from_hash_of("pixeldraw_atlas_preview_window");
    let preview_builder = egui::ViewportBuilder::default()
        .with_title("아틀라스 · 프리뷰 및 저장")
        .with_position([540.0, 60.0])
        .with_inner_size([720.0, 650.0])
        .with_min_inner_size([420.0, 380.0]);
    ctx.show_viewport_immediate(preview_id, preview_builder, |ctx, _| {
        if ctx.input(|i| i.viewport().close_requested()) {
            should_close = true;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("프리뷰 및 최종 저장");
            ui.label(format!(
                "{}개 셀 · {}×{} px",
                atlas.layout.cell_count(&doc),
                atlas.layout.cell_w,
                atlas.layout.cell_h
            ));
            ui.separator();
            let cells = atlas.layout.cells(&doc);
            let _ = preview(ui, atlas, app, &cells, false);
            ui.separator();
            export_buttons(ui, atlas, app);
            export_progress(ui, atlas, app);
            if let Some(error) = &atlas.error {
                ui.colored_label(FOCUS_RED, error);
            }
        });
    });

    let layers_id = egui::ViewportId::from_hash_of("pixeldraw_atlas_layers_window");
    let layers_builder = egui::ViewportBuilder::default()
        .with_title("아틀라스 · 선택 슬롯 레이어")
        .with_position([1280.0, 60.0])
        .with_inner_size([420.0, 650.0])
        .with_min_inner_size([320.0, 380.0]);
    ctx.show_viewport_immediate(layers_id, layers_builder, |ctx, _| {
        if ctx.input(|i| i.viewport().close_requested()) {
            should_close = true;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("선택된 슬롯의 레이어");
            ui.label("위에서 아래 순서로 합성됩니다. 행을 끌어 순서를 바꾸세요.");
            ui.separator();
            slots::layer_window(ui, atlas, &doc);
        });
    });

    if should_close {
        atlas.close_requested = true;
    }
}

fn note(app: &mut AppState, text: impl Into<String>) {
    app.status.push(crate::session_glue::StatusMessage {
        level: "info".to_string(),
        text: text.into(),
    });
    if app.status.len() > 8 {
        app.status.remove(0);
    }
}

fn layout_valid(app: &AppState, layout: &AtlasLayout, mode: AtlasMode) -> bool {
    let Some(doc) = app.session.doc() else {
        return false;
    };
    let mut layout = layout.clone();
    layout.mode = mode;
    layout.validate(doc).is_ok()
}

/// Render the atlas workbench inside the modal window.
pub fn show(ui: &mut Ui, atlas: &mut AtlasState, app: &mut AppState) {
    let Some(doc) = app.session.doc() else {
        ui.label("문서가 없습니다.");
        return;
    };
    if atlas.persisted_layout != doc.atlas {
        atlas.persisted_layout = doc.atlas.clone();
        atlas.layout = atlas
            .persisted_layout
            .clone()
            .unwrap_or_else(|| AtlasLayout::from_document(doc));
    }
    let mut initial_layout = atlas.layout.clone();
    let cells = atlas.cells(doc);
    atlas.clamp_focus(cells.len());

    // Composed mode is a three-pane workspace, so it asks for more width when
    // the viewport allows it. Layers mode keeps the compact single column.
    let composed_width = atlas.layout.mode == AtlasMode::Composed;
    let viewport_w = ui.ctx().input(|input| {
        input
            .viewport()
            .inner_rect
            .map(|rect| rect.width())
            .unwrap_or(1200.0)
    });
    ui.set_min_width(if composed_width {
        (viewport_w - 120.0).clamp(720.0, 1120.0)
    } else {
        720.0
    });

    ui.horizontal(|ui| {
        ui.label(RichText::new("아틀라스").size(16.0).strong());
        ui.label(format!(
            "{} · {}셀 · 셀 {}×{} px",
            match atlas.layout.mode {
                AtlasMode::Layers => "레이어 보기",
                AtlasMode::Composed => "합성 보기",
            },
            cells.len(),
            atlas.layout.cell_w,
            atlas.layout.cell_h
        ));
    });
    ui.separator();

    // --- view + layout -----------------------------------------------------
    ui.horizontal(|ui| {
        ui.label("보기:");
        ui.radio_value(&mut atlas.layout.mode, AtlasMode::Layers, "레이어");
        ui.radio_value(&mut atlas.layout.mode, AtlasMode::Composed, "합성 슬롯");
    });
    layout_controls(ui, atlas, doc);
    let composed = atlas.layout.mode == AtlasMode::Composed;

    // --- workspace ---------------------------------------------------------
    ui.separator();
    if composed {
        composed_workspace(ui, atlas, app, &cells);
    } else {
        let _ = preview(ui, atlas, app, &cells, true);
        layer_edit_button(ui, atlas, app);
        ui.separator();
        ui.label(
            RichText::new(
                "레이어 보기는 각 레이어를 한 셀에 그대로 배치합니다. 합성 슬롯을 편집하려면 보기를 바꾸세요.",
            )
            .size(12.0)
            .color(Color32::from_gray(150)),
        );
    }

    // --- actions -----------------------------------------------------------
    ui.separator();
    export_buttons(ui, atlas, app);
    export_progress(ui, atlas, app);
    open_atlas_button(ui, atlas, app, &mut initial_layout);
    grid_import_panel(ui, atlas, app, &mut initial_layout);
}

fn layer_edit_button(ui: &mut Ui, atlas: &mut AtlasState, app: &mut AppState) {
    let Some(doc) = app.session.doc() else {
        return;
    };
    let edit_layer_id = atlas.focused_layer_id(doc);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                edit_layer_id.is_some(),
                egui::Button::new("선택한 레이어 편집"),
            )
            .on_hover_text("셀에 연결된 레이어를 활성화하고 아틀라스 창을 닫습니다")
            .clicked()
            && let Some(id) = edit_layer_id.as_deref()
        {
            match app.session.set_active_layer(id) {
                Ok(_) => atlas.close_requested = true,
                Err(e) => atlas.error = Some(format!("레이어 선택 실패: {}", e.message)),
            }
        }
    });
}

fn export_buttons(ui: &mut Ui, atlas: &mut AtlasState, app: &mut AppState) {
    ui.horizontal(|ui| {
        let idle = atlas.export_task.is_none();
        let valid_layers = layout_valid(app, &atlas.layout, AtlasMode::Layers);
        let valid_composed = layout_valid(app, &atlas.layout, AtlasMode::Composed);
        if ui
            .add_enabled(
                idle && valid_layers,
                egui::Button::new("레이어별 PNG 저장").min_size(Vec2::new(0.0, 26.0)),
            )
            .on_hover_text("각 레이어를 자기 셀에 배치한 큰 시트를 저장")
            .clicked()
        {
            export_atlas(atlas, app, AtlasMode::Layers);
        }
        if ui
            .add_enabled(
                idle && valid_composed,
                egui::Button::new("합성 슬롯 PNG 저장").min_size(Vec2::new(0.0, 26.0)),
            )
            .on_hover_text("각 슬롯의 레이어를 순서대로 합성해 한 셀에 저장")
            .clicked()
        {
            export_atlas(atlas, app, AtlasMode::Composed);
        }
    });
}

fn export_progress(ui: &mut Ui, atlas: &mut AtlasState, app: &mut AppState) {
    let Some(task) = atlas.export_task.clone() else {
        return;
    };
    let snapshot = task.progress.lock().ok().map(|progress| {
        (
            progress.done_cells,
            progress.total_cells,
            progress.finished.clone(),
        )
    });
    let Some((done, total, finished)) = snapshot else {
        atlas.export_task = None;
        atlas.error = Some("아틀라스 내보내기 진행 상태를 읽지 못했습니다.".into());
        return;
    };
    if let Some(result) = finished {
        atlas.export_task = None;
        match result {
            Ok(()) => crate::ui::atlas::export::note_export_complete(app),
            Err(message) if message == "cancelled" => {
                atlas.error = Some("내보내기를 취소했습니다.".into())
            }
            Err(message) => atlas.error = Some(format!("아틀라스 저장 실패: {message}")),
        }
        return;
    }
    ui.horizontal(|ui| {
        let fraction = if total == 0 {
            0.0
        } else {
            (done as f32 / total as f32) * 0.9
        };
        ui.add(
            egui::ProgressBar::new(fraction)
                .desired_width(180.0)
                .show_percentage(),
        );
        ui.label(if done == total && total > 0 {
            "PNG 인코딩 및 저장 중".to_string()
        } else {
            format!("셀 {done}/{total} 렌더링 중")
        });
        if ui.button("취소").clicked() {
            task.cancel();
        }
    });
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(50));
}

fn open_atlas_button(
    ui: &mut Ui,
    atlas: &mut AtlasState,
    app: &mut AppState,
    initial_layout: &mut AtlasLayout,
) {
    ui.horizontal(|ui| {
        if ui.button("PNG 그리드 분할…").clicked()
            && let Some(path) = pixel_platform::pick_open_file(pixel_platform::FileKind::Png)
        {
            match std::fs::read(&path) {
                Ok(bytes) => match decode_png_large(&bytes) {
                    Ok(image) if image.metadata_chunk("dtAt").is_some() => {
                        match open_png_bytes(app, &bytes) {
                            Ok(()) => {
                                atlas.rebuild(app);
                                *initial_layout = atlas.layout.clone();
                                atlas.grid_import = None;
                                note(app, "Pixeldraw 아틀라스 가져옴");
                            }
                            Err(e) => atlas.error = Some(format!("가져오기 실패: {}", e.message)),
                        }
                    }
                    Ok(image) => {
                        atlas.grid_import = Some(grid_import_from_image(image));
                        atlas.error = None;
                    }
                    Err(e) => atlas.error = Some(format!("PNG 읽기 실패: {}", e.message)),
                },
                Err(e) => atlas.error = Some(format!("파일 읽기 실패: {e}")),
            }
        }
    });
}

fn grid_import_panel(
    ui: &mut Ui,
    atlas: &mut AtlasState,
    app: &mut AppState,
    initial_layout: &mut AtlasLayout,
) {
    if atlas.grid_import.is_some() {
        let action = atlas
            .grid_import
            .as_mut()
            .map(|import| grid_import_editor(ui, import, app))
            .unwrap_or(GridImportAction::None);
        match action {
            GridImportAction::Apply => {
                if let Some(import) = atlas.grid_import.clone() {
                    apply_grid_import(&import, atlas, app, initial_layout);
                }
            }
            GridImportAction::Cancel => atlas.grid_import = None,
            GridImportAction::None => {}
        }
    }
}
