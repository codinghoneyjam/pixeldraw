//! Menubar action dispatch.
//!
//! Split out of `menubar.rs`: one pure match from `Action` to a session/UI
//! effect. It is the only part of the menubar that does not draw anything, so
//! it gets its own file and its own tests.

use crate::session_glue::AppState;
use crate::ui::UiState;
use crate::ui::actions::Action;
use crate::ui::dialogs;
use pixel_platform;

pub fn dispatch_action(action: Action, app: &mut AppState, ui_state: &mut UiState) {
    use Action::*;
    match action {
        NewDocument => {
            ui_state.dialogs.active = Some(dialogs::Dialog::new_document());
        }
        OpenFile => {
            if let Some(path) = pixel_platform::pick_open_file(pixel_platform::FileKind::Any) {
                crate::shell::open_any(app, &path);
            }
        }
        Save => {
            if let Ok(bytes) = crate::file_ops::save_json_bytes(app)
                && let Some(path) =
                    pixel_platform::pick_save_file(pixel_platform::FileKind::Json, "untitled")
            {
                let _ = pixel_platform::write_bytes_atomic(&path, &bytes);
            }
        }
        SaveAs => {
            if let Ok(bytes) = crate::file_ops::save_json_bytes(app)
                && let Some(path) =
                    pixel_platform::pick_save_file(pixel_platform::FileKind::Json, "untitled")
            {
                let _ = pixel_platform::write_bytes_atomic(&path, &bytes);
            }
        }
        ExportPng => {
            if let Ok(bytes) = crate::file_ops::export_png_bytes(app)
                && let Some(path) =
                    pixel_platform::pick_save_file(pixel_platform::FileKind::Png, "canvas")
            {
                let _ = pixel_platform::write_bytes_atomic(&path, &bytes);
            }
        }
        Undo => {
            let _ = app.session.undo();
        }
        Redo => {
            let _ = app.session.redo();
        }
        ZoomIn => {
            app.view_store.zoom_in();
        }
        ZoomOut => {
            app.view_store.zoom_out();
        }
        ZoomFit => {
            app.view_store.fit();
        }
        ZoomActual => {
            app.view_store.actual_size();
        }
        ToggleGrid => {
            let current = app.session.settings().grid_mode;
            let next = match current {
                "off" => "unit",
                "unit" => "tile",
                "tile" => "pixel",
                _ => "off",
            };
            let _ = app
                .session
                .set_setting("gridMode", pixel_engine::model::SettingInput::Str(next));
        }
        NewLayer => {
            let _ = app.session.add_layer(None);
        }
        DuplicateLayer => {
            if let Some(id) = app.session.doc().map(|d| d.active_layer_id.clone()) {
                let _ = app.session.duplicate_layer(&id);
            }
        }
        DeleteLayer => {
            if let Some(id) = app.session.doc().map(|d| d.active_layer_id.clone()) {
                let _ = app.session.remove_layer(&id);
            }
        }
        MergeDown => {
            if let Some(id) = app.session.doc().map(|d| d.active_layer_id.clone()) {
                let _ = app.session.merge_down(&id);
            }
        }
        LayerUp | LayerDown => {
            let delta = if matches!(action, LayerUp) { 1 } else { -1 };
            let target = app.session.doc().and_then(|d| {
                d.index_of(&d.active_layer_id.clone())
                    .map(|idx| (d.active_layer_id.clone(), idx as i64))
            });
            if let Some((id, idx)) = target {
                let _ = app.session.move_layer(&id, idx + delta);
            }
        }
        Preferences => {
            ui_state.dialogs.active = Some(dialogs::Dialog::preferences());
        }
        ShowHelp => {
            ui_state.dialogs.active = Some(dialogs::Dialog::new(dialogs::DialogKind::Help));
        }
        _ => {}
    }
}
