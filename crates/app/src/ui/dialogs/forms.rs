//! Per-dialog form bodies and the validation rules behind them.
//!
//! Split out of `dialogs.rs`: the facade owns the kinds, the state and the
//! integration function; this file owns what each dialog actually draws and the
//! "is this value legal" checks that gate its confirm button. The validation
//! helpers are pure, so their tests could move here with them.

use super::super::{Dialog, DialogKind, UiState};
use super::validate::{new_document_valid, resize_valid};
use crate::session_glue::AppState;
use egui::{RichText, Ui, Vec2};

pub fn show(ui: &mut egui::Ui, ui_state: &mut UiState, _app: &mut AppState) {
    if let Some(dialog) = ui_state.dialogs.active.as_mut() {
        if !dialog.open {
            return;
        }

        let title = dialog.title().to_string();
        let mut close_requested = false;
        let mut confirm_requested = false;

        egui::Window::new(title)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::window(ui.style())
                    .corner_radius(egui::CornerRadius::same(8))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgb(0x4c, 0x51, 0x59),
                    ))
                    .inner_margin(Vec2::new(12.0, 8.0)),
            )
            .show(ui.ctx(), |ui| {
                ui.set_min_width(360.0);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);

                match &mut dialog.kind {
                    DialogKind::NewDocument => {
                        new_document_form(ui, dialog);
                    }
                    DialogKind::ResizeCanvas => {
                        resize_canvas_form(ui, dialog);
                    }
                    DialogKind::Preferences => {
                        preferences_form(ui);
                    }
                    DialogKind::Help => {
                        help_form(ui);
                    }
                    DialogKind::Confirm { message, .. } => {
                        confirm_form(ui, message);
                    }
                    _ => {}
                }

                // Keyboard shortcuts for dialogs
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    close_requested = true;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    confirm_requested = true;
                }

                ui.separator();
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let has_confirm = matches!(
                            dialog.kind,
                            DialogKind::NewDocument
                                | DialogKind::ResizeCanvas
                                | DialogKind::Confirm { .. }
                        );

                        if has_confirm {
                            // JS disables the confirm button while the
                            // size is out of range / off the 32px grid.
                            let valid = match dialog.kind {
                                DialogKind::NewDocument => new_document_valid(dialog).0,
                                DialogKind::ResizeCanvas => resize_valid(dialog).0,
                                _ => true,
                            };
                            if ui.button("취소").clicked() {
                                close_requested = true;
                            }
                            if ui
                                .add_enabled(
                                    valid,
                                    egui::Button::new("확인")
                                        .fill(egui::Color32::from_rgb(0x00, 0x96, 0xff)),
                                )
                                .clicked()
                            {
                                confirm_requested = true;
                            }
                        } else if ui.button("닫기").clicked() {
                            close_requested = true;
                        }
                    });
                });
            });

        if confirm_requested {
            dialog.confirmed = true;
            dialog.open = false;
        } else if close_requested {
            dialog.confirmed = false;
            dialog.open = false;
        }
    }
}

pub(crate) fn new_document_form(ui: &mut Ui, dialog: &mut Dialog) {
    // JS presets: [label, wTiles, hTiles]; px = round(tiles * 64).
    ui.label(RichText::new("규격 프리셋 (타일 단위):").strong());
    ui.horizontal_wrapped(|ui| {
        for (label, wt, ht) in [
            ("1슬롯 128×128", 2.0, 2.0),
            ("16슬롯 512×512", 8.0, 8.0),
            ("64슬롯 1024×1024", 16.0, 16.0),
            ("풀HD 1920×1088", 30.0, 17.0),
        ] {
            if ui.button(label).clicked() {
                dialog.new_doc_w_tiles = wt;
                dialog.new_doc_h_tiles = ht;
            }
        }
    });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("너비(타일):");
        ui.add(
            egui::DragValue::new(&mut dialog.new_doc_w_tiles)
                .range(0.5..=32.0)
                .speed(0.5),
        );
        ui.label(format!(
            "({} × {} px)",
            (dialog.new_doc_w_tiles * 64.0).round() as u32,
            (dialog.new_doc_h_tiles * 64.0).round() as u32
        ));
    });

    ui.horizontal(|ui| {
        ui.label("높이(타일):");
        ui.add(
            egui::DragValue::new(&mut dialog.new_doc_h_tiles)
                .range(0.5..=17.0)
                .speed(0.5),
        );
    });

    ui.separator();
    ui.label("배경:");
    ui.horizontal(|ui| {
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
        ui.radio_value(
            &mut dialog.new_doc_background,
            "#000000".to_string(),
            "검정",
        );
    });
    ui.horizontal(|ui| {
        ui.label("문서 이름:");
        ui.text_edit_singleline(&mut dialog.new_doc_name);
    });
    if !new_document_valid(dialog).0 {
        ui.label(
            RichText::new(new_document_valid(dialog).1)
                .size(12.0)
                .color(egui::Color32::from_rgb(0xff, 0x80, 0x80)),
        );
    }
}

pub(crate) fn resize_canvas_form(ui: &mut Ui, dialog: &mut Dialog) {
    ui.horizontal(|ui| {
        ui.label("너비(px, 32px 배수):");
        ui.add(
            egui::DragValue::new(&mut dialog.resize_width)
                .range(32..=2048)
                .speed(32.0),
        );
        ui.label(format!("({} 타일)", dialog.resize_width / 64));
    });
    ui.horizontal(|ui| {
        ui.label("높이(px, 32px 배수):");
        ui.add(
            egui::DragValue::new(&mut dialog.resize_height)
                .range(32..=1088)
                .speed(32.0),
        );
        ui.label(format!("({} 타일)", dialog.resize_height / 64));
    });
    if !resize_valid(dialog).0 {
        ui.label(
            RichText::new(resize_valid(dialog).1)
                .size(12.0)
                .color(egui::Color32::from_rgb(0xff, 0x80, 0x80)),
        );
    }
}

pub(crate) fn preferences_form(ui: &mut Ui) {
    ui.label("Pixeldraw 설정");
    ui.separator();
    ui.label("• 테마: 챠콜 다크 (Charcoal Dark)");
    ui.label("• 언어: 한국어 (기본)");
    ui.label("• 최대 Undo 버퍼: 200 스텝 / 96MB");
    ui.label("• 타일 규격: 64×64 px (타일), 32×32 px (단위/청크)");
}

pub(crate) fn help_form(ui: &mut Ui) {
    ui.label(RichText::new("Pixeldraw 단축키 가이드").size(15.0).strong());
    ui.separator();
    ui.label("도구 선택:");
    ui.label("  B - 펜        E - 지우개     G - 페인트통(채우기)");
    ui.label("  I - 스포이드   L - 직선       R - 사각형");
    ui.label("  U - 둥근 사각  O - 타원       P - 다각형");
    ui.label("  A - 호        C - 현         V - 방사형 그래디언트");
    ui.label("  T - 텍스트     H - 화면 이동(손)");
    ui.separator();
    ui.label("단축키 명령:");
    ui.label("  Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y : 실행 취소 / 다시 실행");
    ui.label("  Ctrl+N / Ctrl+O : 새 문서 / 열기");
    ui.label("  Ctrl+S : 저장");
    ui.label("  Ctrl+E : PNG 내보내기");
    ui.label("  [ / ] (Shift: 5씩) : 브러시 크기");
    ui.label("  X : 전경색 / 배경색 스왑");
    ui.label("  D : 기본 색상(검정/흰색) 초기화");
    ui.label("  Ctrl+' : 그리드 모드 순환");
    ui.label("  + / - / 0 / 1 : 확대 / 축소 / 맞추기 / 100%");
    ui.label("  Ctrl+Shift+M : 아래로 병합");
    ui.label("  Ctrl+] / Ctrl+[ : 레이어 위로 / 아래로");
    ui.separator();
    ui.label("네이티브 v0.1.0 (단일 실행 .exe)");
}

pub(crate) fn confirm_form(ui: &mut Ui, message: &str) {
    ui.label(RichText::new(message).size(13.0));
}
