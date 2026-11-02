//! Dialog window shell, per-kind forms, and the atlas modal embedding.

use super::super::UiState;
use super::{Dialog, DialogKind, DialogResult, Dialogs};
use crate::session_glue::AppState;
use egui::{RichText, Ui, Vec2};

impl Dialogs {
    pub fn show(&mut self, ui: &mut egui::Ui, ui_state: &mut UiState, app: &mut AppState) {
        if let Some(dialog) = ui_state.dialogs.active.as_mut()
            && matches!(&dialog.kind, DialogKind::Atlas)
        {
            if dialog.open {
                crate::ui::atlas::show_windows(ui.ctx(), &mut dialog.atlas, app);
                if dialog.atlas.close_requested {
                    dialog.atlas.close_requested = false;
                    match dialog.atlas.commit_layout(app) {
                        Ok(()) => {
                            dialog.open = false;
                            dialog.confirmed = true;
                        }
                        Err(error) => dialog.atlas.error = Some(error),
                    }
                }
            }
            return;
        }
        if let Some(dialog) = ui_state.dialogs.active.as_mut() {
            let title = dialog.title().to_string();
            // Results produced by the 3.10b dialogs (restore prompt, close
            // guard). They resolve through `pending_result`; the older kinds
            // keep their post-close path in `apply`.
            let mut pending: Option<DialogResult> = None;

            // Don't use .open() - instead handle the open state ourselves
            // by checking dialog.open before showing and setting it to false when cancelled
            if dialog.open {
                egui::Window::new(title)
                    .resizable(matches!(&dialog.kind, DialogKind::Atlas))
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ui.ctx(), |ui| {
                        ui.set_min_width(if matches!(&dialog.kind, DialogKind::Atlas) {
                            720.0
                        } else {
                            360.0
                        });
                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 12.0);

                        match &mut dialog.kind {
                            DialogKind::NewDocument => {
                                Self::new_document_form(ui, dialog);
                            }
                            DialogKind::ResizeCanvas => {
                                Self::resize_canvas_form(ui, dialog);
                            }
                            DialogKind::Preferences => {
                                Self::preferences_form(ui);
                            }
                            DialogKind::Help => {
                                Self::help_form(ui);
                            }
                            DialogKind::Confirm { message, .. } => {
                                Self::confirm_form(ui, message);
                            }
                            DialogKind::RestoreAutosave { name, when } => {
                                Self::restore_autosave_form(ui, name, when);
                            }
                            DialogKind::ConfirmExit => {
                                Self::confirm_exit_form(ui);
                            }
                            DialogKind::Atlas => unreachable!("atlas uses its own viewports"),
                            DialogKind::ColorPicker
                            | DialogKind::LayerProperties
                            | DialogKind::ExportOptions => {
                                // TODO: color picker / layer properties / export
                            }
                        }

                        // 3.10b: Esc is the JS backdrop path — 버리기 for the
                        // restore prompt (D8), stay for the close guard.
                        if matches!(
                            &dialog.kind,
                            DialogKind::RestoreAutosave { .. } | DialogKind::ConfirmExit
                        ) && ui.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            pending = Some(match &dialog.kind {
                                DialogKind::ConfirmExit => DialogResult::ConfirmExit(false),
                                _ => DialogResult::AutosaveRestore(false),
                            });
                            dialog.open = false;
                        }

                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| match &dialog.kind {
                                    DialogKind::RestoreAutosave { .. } => {
                                        if ui.button("복원").clicked() {
                                            pending = Some(DialogResult::AutosaveRestore(true));
                                            dialog.open = false;
                                        }
                                        if ui.button("버리기").clicked() {
                                            pending = Some(DialogResult::AutosaveRestore(false));
                                            dialog.open = false;
                                        }
                                    }
                                    DialogKind::ConfirmExit => {
                                        if ui.button("버리기").clicked() {
                                            pending = Some(DialogResult::ConfirmExit(true));
                                            dialog.open = false;
                                        }
                                        if ui.button("취소").clicked() {
                                            pending = Some(DialogResult::ConfirmExit(false));
                                            dialog.open = false;
                                        }
                                    }
                                    _ => {
                                        if ui.button("확인").clicked() {
                                            if matches!(&dialog.kind, DialogKind::Atlas) {
                                                match dialog.atlas.commit_layout(app) {
                                                    Ok(()) => {
                                                        dialog.confirmed = true;
                                                        dialog.open = false;
                                                    }
                                                    Err(error) => {
                                                        dialog.atlas.error = Some(error);
                                                    }
                                                }
                                            } else {
                                                dialog.confirmed = true;
                                                dialog.open = false;
                                            }
                                        }
                                        if ui.button("취소").clicked() {
                                            dialog.confirmed = false;
                                            dialog.open = false;
                                        }
                                    }
                                },
                            );
                        });
                    });
            }

            if let Some(result) = pending {
                ui_state.dialogs.pending_result = Some(result);
                ui_state.dialogs.active = None;
            }
        }
    }

    fn new_document_form(ui: &mut Ui, dialog: &mut Dialog) {
        ui.label("너비 (px):");
        ui.add(
            egui::DragValue::new(&mut dialog.new_doc_width)
                .range(1..=8192)
                .speed(1.0),
        );
        ui.label("높이 (px):");
        ui.add(
            egui::DragValue::new(&mut dialog.new_doc_height)
                .range(1..=8192)
                .speed(1.0),
        );
        ui.separator();
        ui.label("배경:");
        ui.radio_value(
            &mut dialog.new_doc_background,
            "transparent".to_string(),
            "투명",
        );
        ui.radio_value(
            &mut dialog.new_doc_background,
            "#ffffff".to_string(),
            "흰색",
        );
        // The result is handled when the user clicks OK in the window
        // We'll handle the result in the main show loop
    }

    fn resize_canvas_form(ui: &mut Ui, dialog: &mut Dialog) {
        ui.label("새 너비 (px):");
        ui.add(
            egui::DragValue::new(&mut dialog.resize_width)
                .range(1..=8192)
                .speed(1.0),
        );
        ui.label("새 높이 (px):");
        ui.add(
            egui::DragValue::new(&mut dialog.resize_height)
                .range(1..=8192)
                .speed(1.0),
        );
    }

    fn preferences_form(ui: &mut Ui) {
        ui.label("환경설정은 추후 구현 예정입니다.");
        ui.separator();
        ui.label("• 테마: 시스템/라이트/다크");
        ui.label("• 언어: 한국어/영어");
        ui.label("• 자동 저장 간격");
        ui.label("• 단축키 커스터마이징");
    }

    fn help_form(ui: &mut Ui) {
        ui.label(RichText::new("Pixeldraw 도움말").size(16.0).strong());
        ui.separator();
        ui.label("단축키:");
        ui.label("  B - 펜   E - 지우개   L - 직선");
        ui.label("  R - 사각형   U - 타원   F - 채우기");
        ui.label("  I - 스포이드   H - 손   T - 텍스트");
        ui.separator();
        ui.label("  Ctrl+Z - 실행취소   Ctrl+Y - 다시실행");
        ui.label("  Ctrl+N - 새 문서   Ctrl+O - 열기");
        ui.label("  Ctrl+S - 저장   Ctrl+Shift+S - 다른이름으로 저장");
        ui.separator();
        ui.label("버전: 0.1.0 (Rust 포팅 중)");
    }

    fn confirm_form(ui: &mut Ui, message: &str) {
        ui.label(RichText::new(message).size(13.0));
    }

    /// 3.7d copy: `자동저장된 문서 "name" (when)가 있습니다. 복원하시겠습니까?`
    fn restore_autosave_form(ui: &mut Ui, name: &str, when: &str) {
        ui.label(
            RichText::new(format!(
                "자동저장된 문서 \"{name}\" ({when})가 있습니다. 복원하시겠습니까?"
            ))
            .size(13.0),
        );
    }

    /// 3.7c copy for the close guard (the JS used the browser `beforeunload`
    /// prompt, so no portable string exists).
    fn confirm_exit_form(ui: &mut Ui) {
        ui.label(RichText::new("저장하지 않은 변경이 있습니다. 버리시겠습니까?").size(13.0));
    }
}
