//! Menu/toolbar action dispatch, and the "open any file kind" helper.
//!
//! Split out of `shell.rs` when it passed 540 lines: one pure match from
//! `ui::actions::Action` to a session/UI effect, plus the file loader that the
//! dialog path shares. Keeping the match out of the eframe impl means the
//! window code stays about the window.

use super::super::file_ops::{export_png_bytes, open_json_bytes, open_png_bytes, save_json_bytes};
use super::super::session_glue::AppState;
use super::super::ui::UiState;
use super::super::ui::actions::Action;
use pixel_platform;

pub fn dispatch_action(action: Action, app: &mut AppState, ui_state: &mut UiState) {
    use Action::*;
    match action {
        NewDocument => {
            ui_state.dialogs.active = Some(super::super::ui::dialogs::Dialog::new_document());
        }
        OpenFile => {
            if let Some(path) = pixel_platform::pick_open_file(pixel_platform::FileKind::Any) {
                open_any(app, &path);
            }
        }
        Save => {
            if let Ok(bytes) = save_json_bytes(app) {
                if let Some(path) =
                    pixel_platform::pick_save_file(pixel_platform::FileKind::Json, "untitled")
                {
                    let _ = pixel_platform::write_bytes_atomic(&path, &bytes);
                    let _ = "저장됨";
                }
            } else {
                let _ = "저장 실패";
            }
        }
        SaveAs => {
            if let Ok(bytes) = save_json_bytes(app) {
                if let Some(path) =
                    pixel_platform::pick_save_file(pixel_platform::FileKind::Json, "untitled")
                {
                    let _ = pixel_platform::write_bytes_atomic(&path, &bytes);
                    let _ = "저장됨";
                }
            } else {
                let _ = "저장 실패";
            }
        }
        ExportPng => {
            if let Ok(bytes) = export_png_bytes(app) {
                if let Some(path) =
                    pixel_platform::pick_save_file(pixel_platform::FileKind::Png, "canvas")
                {
                    let _ = pixel_platform::write_bytes_atomic(&path, &bytes);
                    let _ = "PNG 저장됨";
                }
            } else {
                let _ = "PNG 실패";
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
            ui_state.dialogs.active = Some(super::super::ui::dialogs::Dialog::preferences());
        }
        ShowHelp => {
            ui_state.dialogs.active = Some(super::super::ui::dialogs::Dialog::new(
                super::super::ui::dialogs::DialogKind::Help,
            ));
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
        }
        BrushSizeDec => {
            let cur = app.session.settings().pen_size;
            let next = (cur.saturating_sub(1)).max(1);
            let _ = app.session.set_setting(
                "penSize",
                pixel_engine::model::SettingInput::Num(next as f64),
            );
            if let Some(opts) = &mut ui_state.optionbar.current_tool_options {
                opts.size = Some(next);
            }
        }
        BrushSizeDecFast => {
            let cur = app.session.settings().pen_size;
            let next = (cur.saturating_sub(5)).max(1);
            let _ = app.session.set_setting(
                "penSize",
                pixel_engine::model::SettingInput::Num(next as f64),
            );
            if let Some(opts) = &mut ui_state.optionbar.current_tool_options {
                opts.size = Some(next);
            }
        }
        BrushSizeInc => {
            let cur = app.session.settings().pen_size;
            let next = (cur + 1).min(64);
            let _ = app.session.set_setting(
                "penSize",
                pixel_engine::model::SettingInput::Num(next as f64),
            );
            if let Some(opts) = &mut ui_state.optionbar.current_tool_options {
                opts.size = Some(next);
            }
        }
        BrushSizeIncFast => {
            let cur = app.session.settings().pen_size;
            let next = (cur + 5).min(64);
            let _ = app.session.set_setting(
                "penSize",
                pixel_engine::model::SettingInput::Num(next as f64),
            );
            if let Some(opts) = &mut ui_state.optionbar.current_tool_options {
                opts.size = Some(next);
            }
        }
        SwapColors => {
            let s = app.session.settings();
            let a = s.primary_color.clone();
            let b = s.secondary_color.clone();
            let _ = app
                .session
                .set_setting("primaryColor", pixel_engine::model::SettingInput::Str(&b));
            let _ = app
                .session
                .set_setting("secondaryColor", pixel_engine::model::SettingInput::Str(&a));
        }
        ResetColors => {
            let _ = app.session.set_setting(
                "primaryColor",
                pixel_engine::model::SettingInput::Str("#000000"),
            );
            let _ = app.session.set_setting(
                "secondaryColor",
                pixel_engine::model::SettingInput::Str("#ffffff"),
            );
        }
        _ => {}
    }
}

/// Open whatever the user picked: a PNG goes through the raster decoder, anything
/// else through the JSON document reader. Shared by the menubar dialogs.
pub fn open_any(app: &mut AppState, path: &std::path::Path) {
    let Ok(bytes) = pixel_platform::read_bytes(path) else {
        return;
    };
    let is_png = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
    let _ = if is_png {
        open_png_bytes(app, &bytes)
    } else {
        open_json_bytes(app, &bytes)
    };
}
