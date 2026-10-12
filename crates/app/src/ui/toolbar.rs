//! Toolbar — port of `src/ui/toolbar/**`.

use super::UiState;
use crate::session_glue::AppState;
use egui::{Button, RichText, Ui, Vec2};

#[derive(Default)]
pub struct ToolbarState {
    pub active_tool: String,
    pub tool_hovered: Option<String>,
}

pub struct Toolbar {
    tools: Vec<ToolButton>,
}

#[derive(Clone)]
struct ToolButton {
    id: &'static str,
    label: &'static str,
    #[allow(dead_code)]
    shortcut: Option<&'static str>,
    icon: &'static str,
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            tools: vec![
                ToolButton {
                    id: "pen",
                    label: "펜",
                    shortcut: Some("B"),
                    icon: "✏",
                },
                ToolButton {
                    id: "eraser",
                    label: "지우개",
                    shortcut: Some("E"),
                    icon: "🧽",
                },
                ToolButton {
                    id: "line",
                    label: "직선",
                    shortcut: Some("L"),
                    icon: "━",
                },
                ToolButton {
                    id: "rect",
                    label: "사각형",
                    shortcut: Some("R"),
                    icon: "▭",
                },
                ToolButton {
                    id: "ellipse",
                    label: "타원",
                    shortcut: Some("U"),
                    icon: "⭘",
                },
                ToolButton {
                    id: "fill",
                    label: "채우기",
                    shortcut: Some("F"),
                    icon: "🪣",
                },
                ToolButton {
                    id: "eyedropper",
                    label: "스포이드",
                    shortcut: Some("I"),
                    icon: "💧",
                },
                ToolButton {
                    id: "hand",
                    label: "손",
                    shortcut: Some("H"),
                    icon: "✋",
                },
                ToolButton {
                    id: "text",
                    label: "텍스트",
                    shortcut: Some("T"),
                    icon: "𝌵",
                },
                ToolButton {
                    id: "gradient",
                    label: "그라디언트",
                    shortcut: Some("G"),
                    icon: "▰",
                },
                ToolButton {
                    id: "shape",
                    label: "도형",
                    shortcut: Some("M"),
                    icon: "▱",
                },
            ],
        }
    }

    pub fn show(&mut self, ui: &mut Ui, state: &mut UiState, app: &mut AppState) {
        egui::Panel::top("toolbar")
            .frame(egui::Frame::new().inner_margin(Vec2::new(8.0, 4.0)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    for tool in &self.tools {
                        let is_active = state.toolbar.active_tool == tool.id;
                        let response = ui.add(
                            Button::new(
                                RichText::new(format!("{} {}", tool.icon, tool.label)).size(13.0),
                            )
                            .fill(if is_active {
                                ui.visuals().selection.bg_fill
                            } else {
                                egui::Color32::TRANSPARENT
                            })
                            .min_size(Vec2::new(72.0, 28.0)),
                        );

                        if response.clicked() {
                            state.toolbar.active_tool = tool.id.to_string();
                            let _ = app.session.set_setting(
                                "activeTool",
                                pixel_engine::model::SettingInput::Str(tool.id),
                            );
                        }

                        if response.hovered() {
                            state.toolbar.tool_hovered = Some(tool.id.to_string());
                        }
                    }

                    ui.separator();

                    // History buttons
                    let (can_undo, can_redo) = app.history_state();
                    if ui
                        .add_enabled(can_undo, Button::new("↶ 실행취소"))
                        .clicked()
                    {
                        let _ = app.session.undo();
                    }
                    if ui
                        .add_enabled(can_redo, Button::new("↷ 다시실행"))
                        .clicked()
                    {
                        let _ = app.session.redo();
                    }

                    ui.separator();

                    // Zoom controls
                    if ui.button("🔍−").clicked() {
                        app.view_store.zoom_out();
                    }
                    if ui.button("🔍+").clicked() {
                        app.view_store.zoom_in();
                    }
                    if ui.button("맞춤").clicked() {
                        app.view_store.fit();
                    }
                    if ui.button("1:1").clicked() {
                        app.view_store.actual_size();
                    }
                });
            });
    }
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::new()
    }
}

// Integration function
pub fn show(ui: &mut egui::Ui, ui_state: &mut UiState, app_state: &mut AppState) {
    let mut toolbar = Toolbar::new();
    egui::Panel::top("toolbar_panel")
        .frame(egui::Frame::new())
        .show(ui, |ui| {
            toolbar.show(ui, ui_state, app_state);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_has_all_tools() {
        let toolbar = Toolbar::new();
        assert_eq!(toolbar.tools.len(), 11);
        assert!(toolbar.tools.iter().any(|t| t.id == "pen"));
        assert!(toolbar.tools.iter().any(|t| t.id == "eraser"));
    }
}
