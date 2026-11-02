//! Single action router (M0-1).
//!
//! Both the menubar and the shell shortcut path funnel through
//! [`dispatch`] so a new [`Action`] variant is handled in exactly one
//! place. Previously `shell.rs` and `menubar.rs` each carried their own
//! `match`, and a handler added to one side was silently missing from the
//! other.

use crate::session_glue::AppState;
use crate::ui::UiState;

use super::actions::Action;
use super::dialogs::Dialog;

/// What [`dispatch`] decided the caller still has to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionOutcome {
    Handled,
    Unsupported,
}

/// Dispatch one action. Returns `Handled` when something happened and
/// `Unsupported` for variants not yet wired (so the UI can grey them out).
pub fn dispatch(action: Action, app: &mut AppState, ui_state: &mut UiState) -> ActionOutcome {
    use Action::*;
    match action {
        NewDocument => {
            ui_state.dialogs.active = Some(Dialog::new_document());
            ActionOutcome::Handled
        }
        OpenFile => {
            if let Some(path) = pixel_platform::pick_open_file(pixel_platform::FileKind::Any) {
                crate::shell::open_any(app, &path);
            }
            ActionOutcome::Handled
        }
        ImportPng => {
            if let Some(path) = pixel_platform::pick_open_file(pixel_platform::FileKind::Png) {
                crate::shell::open_any(app, &path);
            }
            ActionOutcome::Handled
        }
        ImportLayer => {
            if let Some(path) = pixel_platform::pick_open_file(pixel_platform::FileKind::Json) {
                match pixel_platform::read_bytes(&path) {
                    Ok(bytes) => match crate::file_ops::import_layer_json_bytes(app, &bytes) {
                        Ok(id) => app.session.notify(
                            "info",
                            format!("레이어 가져옴: {id}"),
                            "LAYER_IMPORTED",
                        ),
                        Err(error) => {
                            app.session
                                .notify("error", error.message, "LAYER_IMPORT_FAILED")
                        }
                    },
                    Err(error) => {
                        app.session
                            .notify("error", error.to_string(), "LAYER_IMPORT_FAILED")
                    }
                }
            }
            ActionOutcome::Handled
        }
        ExportLayer => {
            if let Some(doc) = app.session.doc()
                && let Ok(bytes) =
                    crate::file_ops::export_layer_json_bytes(app, &doc.active_layer_id)
                && let Some(path) =
                    pixel_platform::pick_save_file(pixel_platform::FileKind::Json, "layer")
                && pixel_platform::write_bytes_atomic(&path, &bytes).is_ok()
            {
                app.session
                    .notify("info", "레이어 저장됨", "LAYER_EXPORTED");
            }
            ActionOutcome::Handled
        }
        Atlas => {
            ui_state.dialogs.active = Some(Dialog::atlas(app));
            ActionOutcome::Handled
        }
        Save => {
            if let Ok(bytes) = crate::file_ops::save_json_bytes(app)
                && let Some(path) =
                    pixel_platform::pick_save_file(pixel_platform::FileKind::Json, "untitled")
                && pixel_platform::write_bytes_atomic(&path, &bytes).is_ok()
            {
                app.session.history_mut().mark_saved();
                app.session.notify("info", "저장됨", "SAVE_OK");
            }
            ActionOutcome::Handled
        }
        SaveAs => {
            // Same as Save for now.
            ActionOutcome::Unsupported
        }
        ExportPng => {
            if let Ok(bytes) = crate::file_ops::export_png_bytes(app)
                && let Some(path) =
                    pixel_platform::pick_save_file(pixel_platform::FileKind::Png, "canvas")
            {
                let _ = pixel_platform::write_bytes_atomic(&path, &bytes);
            }
            ActionOutcome::Handled
        }
        Undo => {
            let _ = app.session.undo();
            ActionOutcome::Handled
        }
        Redo => {
            let _ = app.session.redo();
            ActionOutcome::Handled
        }
        ZoomIn => {
            app.view_store.zoom_in();
            ActionOutcome::Handled
        }
        ZoomOut => {
            app.view_store.zoom_out();
            ActionOutcome::Handled
        }
        ZoomFit => {
            app.view_store.fit();
            ActionOutcome::Handled
        }
        ZoomActual => {
            app.view_store.actual_size();
            ActionOutcome::Handled
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
            ActionOutcome::Handled
        }
        NewLayer => {
            let _ = app.session.add_layer(None);
            ActionOutcome::Handled
        }
        DuplicateLayer => {
            if let Some(id) = app.session.doc().map(|d| d.active_layer_id.clone()) {
                let _ = app.session.duplicate_layer(&id);
            }
            ActionOutcome::Handled
        }
        DeleteLayer => {
            if let Some(id) = app.session.doc().map(|d| d.active_layer_id.clone()) {
                let _ = app.session.remove_layer(&id);
            }
            ActionOutcome::Handled
        }
        MergeDown => {
            if let Some(id) = app.session.doc().map(|d| d.active_layer_id.clone()) {
                let _ = app.session.merge_down(&id);
            }
            ActionOutcome::Handled
        }
        Preferences => {
            ui_state.dialogs.active = Some(Dialog::preferences());
            ActionOutcome::Handled
        }
        ShowHelp => {
            ui_state.dialogs.active = Some(Dialog::new(super::dialogs::DialogKind::Help));
            ActionOutcome::Handled
        }
        ToolPen | ToolEraser | ToolFill | ToolEyedropper | ToolLine | ToolRect | ToolRrect
        | ToolEllipse | ToolPolygon | ToolArc | ToolChord | ToolRadialGradient | ToolText
        | ToolHand => {
            let tool_id = match action {
                ToolPen => "pen",
                ToolEraser => "eraser",
                ToolFill => "fill",
                ToolEyedropper => "eyedropper",
                ToolLine => "line",
                ToolRect => "rect",
                ToolRrect => "rrect",
                ToolEllipse => "ellipse",
                ToolPolygon => "polygon",
                ToolArc => "arc",
                ToolChord => "chord",
                ToolRadialGradient => "radial_gradient",
                ToolText => "text",
                ToolHand => "hand",
                _ => unreachable!(),
            };
            ui_state.toolbar.active_tool = tool_id.to_string();
            let _ = app.session.set_setting(
                "activeTool",
                pixel_engine::model::SettingInput::Str(tool_id),
            );
            ActionOutcome::Handled
        }
        _ => ActionOutcome::Unsupported,
    }
}
