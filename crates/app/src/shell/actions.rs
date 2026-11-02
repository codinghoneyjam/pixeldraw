//! File, edit and view action handling for the native shell.

use super::*;
use crate::file_ops::{export_png_bytes, save_json_bytes};

impl PixeldrawApp {
    /// Dialog outcomes the shell owns: the boot autosave prompt (3.10b) and
    /// the close guard. The older dialog kinds keep their existing post-close
    /// path inside `ui/dialogs.rs`.
    pub(super) fn handle_dialog_results(&mut self, ctx: &egui::Context) {
        use crate::ui::dialogs::DialogResult;
        let Some(result) = self.ui_state.dialogs.pending_result.take() else {
            return;
        };
        match result {
            DialogResult::AutosaveRestore(true) => {
                self.state.restore_autosave();
            }
            DialogResult::AutosaveRestore(false) => {
                self.state.discard_autosave();
            }
            DialogResult::ConfirmExit(true) => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            DialogResult::ConfirmExit(false) => {}
            _ => {}
        }
    }

    pub(super) fn dispatch_action(&mut self, action: crate::ui::actions::Action) {
        use crate::ui::actions::Action::*;
        match action {
            NewDocument => {
                self.ui_state.dialogs.active = Some(crate::ui::dialogs::Dialog::new_document());
            }
            OpenFile => {
                if let Some(path) = pixel_platform::pick_open_file(pixel_platform::FileKind::Any) {
                    self.open_any(&path);
                }
            }
            Save => {
                if let Ok(bytes) = save_json_bytes(&self.state) {
                    if let Some(path) =
                        pixel_platform::pick_save_file(pixel_platform::FileKind::Json, "untitled")
                    {
                        match pixel_platform::write_bytes_atomic(&path, &bytes) {
                            Ok(()) => {
                                // JS `doSave`: `history.markSaved()` only on a
                                // successful write — the statusbar dirty dot
                                // and the close guard read this.
                                self.state.session.history_mut().mark_saved();
                                self.say("저장됨");
                            }
                            Err(e) => self.say(format!("저장 실패: {e}")),
                        }
                    }
                } else {
                    self.say("저장 실패");
                }
            }
            SaveAs => {
                // Same as Save for now
            }
            ImportPng | ImportLayer | ExportLayer | Atlas => {
                // New actions go through the single action router; the older
                // arms below are migrated as they are touched (M0-1).
                let _ =
                    crate::ui::action_router::dispatch(action, &mut self.state, &mut self.ui_state);
            }
            ExportPng => {
                if let Ok(bytes) = export_png_bytes(&self.state) {
                    if let Some(path) =
                        pixel_platform::pick_save_file(pixel_platform::FileKind::Png, "canvas")
                    {
                        let _ = pixel_platform::write_bytes_atomic(&path, &bytes);
                        self.say("PNG 저장됨");
                    }
                } else {
                    self.say("PNG 실패");
                }
            }
            Undo => {
                let _ = self.state.session.undo();
            }
            Redo => {
                let _ = self.state.session.redo();
            }
            ZoomIn => {
                self.state.view_store.zoom_in();
            }
            ZoomOut => {
                self.state.view_store.zoom_out();
            }
            ZoomFit => {
                self.state.view_store.fit();
            }
            ZoomActual => {
                self.state.view_store.actual_size();
            }
            ToggleGrid => {
                let current = self.state.session.settings().grid_mode;
                let next = match current {
                    "off" => "unit",
                    "unit" => "tile",
                    "tile" => "pixel",
                    _ => "off",
                };
                let _ = self
                    .state
                    .session
                    .set_setting("gridMode", pixel_engine::model::SettingInput::Str(next));
            }
            NewLayer => {
                let _ = self.state.session.add_layer(None);
            }
            DuplicateLayer => {
                if let Some(id) = self.state.session.doc().map(|d| d.active_layer_id.clone()) {
                    let _ = self.state.session.duplicate_layer(&id);
                }
            }
            DeleteLayer => {
                if let Some(id) = self.state.session.doc().map(|d| d.active_layer_id.clone()) {
                    let _ = self.state.session.remove_layer(&id);
                }
            }
            MergeDown => {
                if let Some(id) = self.state.session.doc().map(|d| d.active_layer_id.clone()) {
                    let _ = self.state.session.merge_down(&id);
                }
            }
            Preferences => {
                self.ui_state.dialogs.active = Some(crate::ui::dialogs::Dialog::preferences());
            }
            ShowHelp => {
                self.ui_state.dialogs.active = Some(crate::ui::dialogs::Dialog::new(
                    crate::ui::dialogs::DialogKind::Help,
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
                self.ui_state.toolbar.active_tool = tool_id.to_string();
                let _ = self.state.session.set_setting(
                    "activeTool",
                    pixel_engine::model::SettingInput::Str(tool_id),
                );
            }
            _ => {}
        }
    }

    fn open_any(&mut self, path: &std::path::Path) {
        let bytes = match pixel_platform::read_bytes(path) {
            Ok(b) => b,
            Err(e) => {
                self.say(format!("읽기 실패: {e}"));
                return;
            }
        };
        let is_png = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
        let result = if is_png {
            open_png_bytes(&mut self.state, &bytes)
        } else {
            open_json_bytes(&mut self.state, &bytes)
        };
        match result {
            Ok(()) => self.say("열기 완료"),
            Err(e) => self.say(format!("열기 실패: {e}")),
        }
    }
}
