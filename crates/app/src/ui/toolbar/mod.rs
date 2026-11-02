//! Toolbar — port of `src/ui/toolbar/**`.
//!
//! The tool strip is a **left** vertical column of hand-drawn vector icons
//! (`icons`), matching the JS tree where the tools sit in a narrow rail and the
//! wide top strip belongs to the option bar. `render_ui` in `super::mod` owns
//! the panel; this module only lays out into the `Ui` it is handed.

mod icons;

use super::UiState;
use crate::session_glue::AppState;
use egui::{ScrollArea, Ui, Vec2};

#[derive(Default)]
pub struct ToolbarState {
    pub active_tool: String,
    /// The help line for the tool under the pointer, or `None`.
    ///
    /// Cleared at the top of every frame, so a stale hover cannot outlive the
    /// pointer leaving the toolbar.
    pub tool_hovered: Option<String>,
}

pub struct Toolbar {
    tools: Vec<ToolButton>,
}

#[derive(Clone)]
struct ToolButton {
    id: &'static str,
    label: &'static str,
    /// Bare key, shown on the button. Every one of these is wired in
    /// `actions.rs` (`Shortcut::new(B, Modifiers::NONE, ToolPen)` and so on),
    /// so displaying it is stating a fact rather than a promise.
    shortcut: &'static str,
    /// One line shown on hover. The JS tree had these; the port dropped them,
    /// which is why hovering a tool said nothing and the option bar's generic
    /// branch was the only text on screen.
    help: &'static str,
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            // No pictographic icons. This font has no U+1F9FD (sponge), U+1FAA3
            // (bucket), U+1F4A7 (droplet), U+1D335 (double-struck T),
            tools: vec![
                ToolButton {
                    id: "pen",
                    label: "펜",
                    shortcut: "B",
                    help: "자유롭게 그립니다. 드래그하면 이어집니다.",
                },
                ToolButton {
                    id: "eraser",
                    label: "지우개",
                    shortcut: "E",
                    help: "지운 자리는 투명해집니다.",
                },
                ToolButton {
                    id: "fill",
                    label: "채우기",
                    shortcut: "G",
                    help: "누른 색으로 연속된 영역을 채웁니다.",
                },
                ToolButton {
                    id: "eyedropper",
                    label: "스포이드",
                    shortcut: "I",
                    help: "누른 곳의 색을 현재 색으로 가져옵니다.",
                },
                ToolButton {
                    id: "line",
                    label: "직선",
                    shortcut: "L",
                    help: "누른 곳에서 놓은 곳까지 한 줄을 긋습니다.",
                },
                ToolButton {
                    id: "rect",
                    label: "사각형",
                    shortcut: "R",
                    help: "누른 곳에서 놓은 곳까지 직사각형을 그립니다.",
                },
                ToolButton {
                    id: "rrect",
                    label: "둥근사각형",
                    shortcut: "U",
                    help: "모서리가 둥근 직사각형을 그립니다.",
                },
                ToolButton {
                    id: "ellipse",
                    label: "타원",
                    shortcut: "O",
                    help: "누른 곳에서 놓은 곳까지 타원을 그립니다.",
                },
                ToolButton {
                    id: "polygon",
                    label: "다각형",
                    shortcut: "P",
                    help: "클릭으로 정점을 놓고 Enter로 확정합니다.",
                },
                ToolButton {
                    id: "arc",
                    label: "호",
                    shortcut: "A",
                    help: "누른 곳에서 놓은 곳까지 호를 그립니다.",
                },
                ToolButton {
                    id: "chord",
                    label: "현",
                    shortcut: "C",
                    help: "호에 현을 더해 부채꼴을 그립니다.",
                },
                ToolButton {
                    id: "radial_gradient",
                    label: "그라디언트",
                    shortcut: "V",
                    help: "누른 곳에서 놓은 곳까지 방사형 그라디언트를 그립니다.",
                },
                ToolButton {
                    id: "text",
                    label: "텍스트",
                    shortcut: "T",
                    help: "눌러서 입력한 뒤 Esc로 확정합니다.",
                },
                ToolButton {
                    id: "hand",
                    label: "손",
                    shortcut: "H",
                    help: "화면을 끌어다 봅니다.",
                },
            ],
        }
    }

    /// The one place a tool's name is spelled out in full. The rail draws a
    /// vector glyph, so `label` only ever reaches the user here — which is why
    /// the test below calls this instead of rebuilding the string.
    fn tool_tip(tool: &ToolButton) -> String {
        format!("[{}]  {} — {}", tool.shortcut, tool.label, tool.help)
    }

    pub fn show(&mut self, ui: &mut Ui, state: &mut UiState, app: &mut AppState) {
        // Cleared first so the status bar shows nothing once the pointer leaves,
        // rather than keeping the last tool's help forever.
        state.toolbar.tool_hovered = None;

        ui.vertical(|ui| {
            // JS `#dt-toolbar { gap: 4px }` between 36px buttons.
            ui.spacing_mut().item_spacing.y = 4.0;

            for tool in &self.tools {
                let is_active = state.toolbar.active_tool == tool.id;
                let response = tool_icon(ui, tool, is_active, &mut state.juice_clicks)
                    .on_hover_text(Self::tool_tip(tool));

                if response.clicked() {
                    state.toolbar.active_tool = tool.id.to_string();
                    let _ = app.session.set_setting(
                        "activeTool",
                        pixel_engine::model::SettingInput::Str(tool.id),
                    );
                }

                if response.hovered() {
                    state.toolbar.tool_hovered = Some(tool.help.to_string());
                }
            }
        });
    }

    /// The help line for the tool under the pointer.
    pub fn help_for(state: &UiState) -> Option<&str> {
        state.toolbar.tool_hovered.as_deref()
    }
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::new()
    }
}

/// One square tool button, painted rather than built from [`Button`] so the
/// hand-drawn vector glyph can sit on it. Same juice as every other control:
/// hover fade, press pop, accent fill for the active tool.
fn tool_icon(
    ui: &mut Ui,
    tool: &ToolButton,
    is_active: bool,
    clicks: &mut std::collections::HashMap<String, f64>,
) -> egui::Response {
    use crate::ui::juice::{click_pop, hover_fade, lerp_color};
    // JS `#dt-toolbar button { width/height: 36px }` inside the 44px rail
    // column; the 4px `item_spacing.y` above is the JS `gap: 4px`, so the
    // pitch is the JS 40px.
    const S: f32 = 36.0;
    let key = format!("tool:{}", tool.id);
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(S), egui::Sense::click());
    let hid = ui.id().with("tb").with(tool.id);
    let fade = hover_fade(ui.ctx(), hid, resp.hovered());
    let pop = click_pop(clicks, ui.ctx(), &key, resp.clicked());
    // The resting plate is the full 36px button (JS); only a press shrinks it.
    let rect = rect.shrink((1.0 - pop) * 3.0);

    let painter = ui.painter();
    if is_active {
        painter.rect_filled(
            rect,
            5.0,
            lerp_color(
                egui::Color32::from_rgb(0x00, 0x63, 0x9e),
                egui::Color32::from_rgb(0x1e, 0x8f, 0xe0),
                fade,
            ),
        );
        painter.rect_stroke(
            rect,
            5.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(0x4c, 0xc2, 0xff)),
            egui::StrokeKind::Inside,
        );
    } else if fade > 0.0 {
        painter.rect_filled(
            rect,
            5.0,
            egui::Color32::from_white_alpha((28.0 * fade) as u8),
        );
    }

    // The glyph keeps the same colour in both states so the rail reads as one
    // set; only the plate behind it changes.
    let glyph = if is_active {
        egui::Color32::WHITE
    } else {
        lerp_color(
            egui::Color32::from_rgb(0xb6, 0xbb, 0xc4),
            egui::Color32::WHITE,
            fade,
        )
    };
    icons::draw_tool_vector_icon(painter, rect, tool.id, glyph);
    resp
}

/// Integration function.
///
/// `render_ui` in `super::mod` owns the rail's panel (`Panel::left`). This used
/// to open the *same* panel id again — a panel nested inside itself — which
/// handed the inner one only the sliver the outer one left over and clipped
/// every button to its top ~40%.
///
/// The tool rail scrolls vertically rather than clipping its tail in a short
/// window.
pub fn show(ui: &mut egui::Ui, ui_state: &mut UiState, app_state: &mut AppState) {
    let mut toolbar = Toolbar::new();
    ScrollArea::vertical()
        .id_salt("toolbar_scroll")
        .auto_shrink([true, false])
        .show(ui, |ui| {
            toolbar.show(ui, ui_state, app_state);
        });
}
#[cfg(test)]
mod tests;
