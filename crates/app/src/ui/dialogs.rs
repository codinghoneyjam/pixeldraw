//! Dialogs — port of `src/ui/dialogs/**`.

use super::UiState;
use crate::session_glue::AppState;
use egui::{RichText, Ui, Vec2};

/// Dialog kind enumeration.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogKind {
    NewDocument,
    ResizeCanvas,
    Preferences,
    Help,
    Confirm { title: String, message: String },
    ColorPicker,
    LayerProperties,
    ExportOptions,
}

/// Dialog result returned to caller.
#[derive(Debug, Clone)]
pub enum DialogResult {
    NewDocument {
        width: u32,
        height: u32,
        background: String,
    },
    ResizeCanvas {
        width: u32,
        height: u32,
    },
    Preferences,
    Confirmed(bool),
    Color([u8; 4]),
    None,
}

/// Active dialog state.
#[derive(Debug, Clone)]
pub struct Dialog {
    pub kind: DialogKind,
    pub open: bool,
    // Form state for new document
    pub new_doc_width: u32,
    pub new_doc_height: u32,
    pub new_doc_background: String,
    // Form state for resize
    pub resize_width: u32,
    pub resize_height: u32,
}

impl Dialog {
    pub fn new(kind: DialogKind) -> Self {
        Self {
            kind,
            open: true,
            new_doc_width: 512,
            new_doc_height: 512,
            new_doc_background: "transparent".to_string(),
            resize_width: 512,
            resize_height: 512,
        }
    }

    pub fn new_document() -> Self {
        Self::new(DialogKind::NewDocument)
    }

    pub fn resize_canvas(current_w: u32, current_h: u32) -> Self {
        let mut d = Self::new(DialogKind::ResizeCanvas);
        d.resize_width = current_w;
        d.resize_height = current_h;
        d
    }

    pub fn preferences() -> Self {
        Self::new(DialogKind::Preferences)
    }

    pub fn confirm(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DialogKind::Confirm {
            title: title.into(),
            message: message.into(),
        })
    }

    fn title(&self) -> &str {
        match &self.kind {
            DialogKind::NewDocument => "새 문서",
            DialogKind::ResizeCanvas => "캔버스 크기 변경",
            DialogKind::Preferences => "환경설정",
            DialogKind::Help => "도움말",
            DialogKind::Confirm { title, .. } => title.as_str(),
            DialogKind::ColorPicker => "색상 선택",
            DialogKind::LayerProperties => "레이어 속성",
            DialogKind::ExportOptions => "내보내기 옵션",
        }
    }
}

/// Dialogs state container.
#[derive(Default)]
pub struct DialogsState {
    pub active: Option<Dialog>,
    pub pending_result: Option<DialogResult>,
}

pub struct Dialogs;

impl Dialogs {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ui_state: &mut UiState, _app: &mut AppState) {
        if ui_state.dialogs.active.is_some() {
            // Extract dialog and process it
            if let Some(dialog) = ui_state.dialogs.active.as_mut() {
                let title = dialog.title().to_string();

                // Don't use .open() - instead handle the open state ourselves
                // by checking dialog.open before showing and setting it to false when cancelled
                if dialog.open {
                    egui::Window::new(title)
                        .resizable(false)
                        .collapsible(false)
                        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                        .show(ui.ctx(), |ui| {
                            ui.set_min_width(360.0);
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
                                DialogKind::ColorPicker => {
                                    // TODO: color picker
                                }
                                DialogKind::LayerProperties => {
                                    // TODO: layer properties
                                }
                                DialogKind::ExportOptions => {
                                    // TODO: export options
                                }
                            }

                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("취소").clicked() {
                                            dialog.open = false;
                                        }
                                    },
                                );
                            });
                        });
                }
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
}

impl Default for Dialogs {
    fn default() -> Self {
        Self::new()
    }
}

// Integration function
pub fn show(ui: &mut egui::Ui, ui_state: &mut UiState, app_state: &mut AppState) {
    let mut dialogs = Dialogs::new();
    dialogs.show(ui, ui_state, app_state);

    // Handle pending results - for now we'll handle results when dialog is closed
    // The actual result handling would need more sophisticated state management
    if let Some(dialog) = ui_state.dialogs.active.as_ref()
        && !dialog.open
    {
        // Dialog was closed, handle result if needed
        match &dialog.kind {
            DialogKind::NewDocument => {
                let _ = crate::file_ops::new_document_with_bg(
                    app_state,
                    dialog.new_doc_width,
                    dialog.new_doc_height,
                    &dialog.new_doc_background,
                );
            }
            DialogKind::ResizeCanvas => {
                let _ = app_state
                    .session
                    .resize_canvas(dialog.resize_width, dialog.resize_height);
            }
            _ => {}
        }
        ui_state.dialogs.active = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_creation() {
        let d = Dialog::new_document();
        assert_eq!(d.new_doc_width, 512);
        assert_eq!(d.new_doc_background, "transparent");
    }

    #[test]
    fn dialog_resize_uses_current_size() {
        let d = Dialog::resize_canvas(800, 600);
        assert_eq!(d.resize_width, 800);
        assert_eq!(d.resize_height, 600);
    }
}
