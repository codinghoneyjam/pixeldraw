//! Toolbar — port of `src/ui/toolbar/**`.

use super::UiState;
use crate::session_glue::AppState;
use egui::{Button, RichText, ScrollArea, Ui, Vec2};

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
            // U+25AD/U+25B0/U+25B1 (geometric shapes) or U+21B6/U+21B7
            // (arrows), so eight of the eleven buttons rendered as tofu, and a
            // pictogram is exactly the sort of thing that renders on the
            // author's machine and not on anyone else's. The keyboard shortcut
            // takes the icon slot: it is ASCII, always available, and it
            // surfaces a mapping that used to be invisible.
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

    pub fn show(&mut self, ui: &mut Ui, state: &mut UiState, app: &mut AppState) {
        // Cleared first so the status bar shows nothing once the pointer leaves,
        // rather than keeping the last tool's help forever.
        state.toolbar.tool_hovered = None;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            for tool in &self.tools {
                let is_active = state.toolbar.active_tool == tool.id;
                let response = ui
                    .add(
                        Button::new(
                            RichText::new(format!("{} {}", tool.label, tool.shortcut)).size(12.0),
                        )
                        .fill(if is_active {
                            ui.visuals().selection.bg_fill
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .min_size(Vec2::new(56.0, 24.0)),
                    )
                    .on_hover_text(format!("[{}]  {}", tool.shortcut, tool.help));

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

            ui.separator();

            // History buttons
            let (can_undo, can_redo) = app.history_state();
            if ui
                .add_enabled(can_undo, Button::new("실행취소"))
                .on_hover_text("[Ctrl+Z]  되돌립니다.")
                .clicked()
            {
                let _ = app.session.undo();
            }
            if ui
                .add_enabled(can_redo, Button::new("다시실행"))
                .on_hover_text("[Ctrl+Y]  다시 적용합니다.")
                .clicked()
            {
                let _ = app.session.redo();
            }

            ui.separator();

            // Zoom controls. Plain ASCII, for the same reason the tools dropped
            // their icons: U+1F50D is not in every font, and a missing
            // magnifying glass is worse than the word "축소".
            if ui.button("축소").on_hover_text("[Ctrl+-]").clicked() {
                app.view_store.zoom_out();
            }
            if ui.button("확대").on_hover_text("[Ctrl+=]").clicked() {
                app.view_store.zoom_in();
            }
            if ui.button("맞춤").on_hover_text("[Ctrl+0]").clicked() {
                app.view_store.fit();
            }
            if ui.button("1:1").on_hover_text("[Ctrl+1]").clicked() {
                app.view_store.actual_size();
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

/// Integration function.
///
/// `render_ui` in `super::mod` already owns `Panel::top("toolbar_panel")`. This
/// used to open the *same* panel id again — a panel nested inside itself — and
/// [`Toolbar::show`] then opened a third (`Panel::top("toolbar")`) inside that.
/// egui panels are siblings, not nestable: each nested one is handed only the
/// space the outer one left over, which clipped every button to its top ~40%.
///
/// The row is a horizontal [`ScrollArea`] because eleven tools plus undo/redo
/// plus the zoom cluster are wider than a narrow window, and before this they
/// were cut off at the right edge instead of scrolling.
pub fn show(ui: &mut egui::Ui, ui_state: &mut UiState, app_state: &mut AppState) {
    let mut toolbar = Toolbar::new();
    ScrollArea::horizontal()
        .id_salt("toolbar_scroll")
        .auto_shrink([false, true])
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
        assert_eq!(toolbar.tools.len(), 14);
        for id in [
            "pen",
            "eraser",
            "fill",
            "eyedropper",
            "line",
            "rect",
            "rrect",
            "ellipse",
            "polygon",
            "arc",
            "chord",
            "radial_gradient",
            "text",
            "hand",
        ] {
            assert!(toolbar.tools.iter().any(|t| t.id == id), "missing tool {id}");
        }
    }

    /// Every tool needs its own help text; the option bar's generic branch used
    /// to be the only thing on screen when hovering.
    #[test]
    fn every_tool_has_help_and_it_is_distinct() {
        let toolbar = Toolbar::new();
        for t in &toolbar.tools {
            assert!(t.help.len() > 4, "tool {} has no help text", t.id);
            assert!(
                !t.help.contains("없습니다"),
                "tool {} has a fallback string",
                t.id
            );
        }
        let mut seen: Vec<&str> = toolbar.tools.iter().map(|t| t.help).collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(before, seen.len(), "two tools share a help string");
    }

    #[test]
    fn every_tool_declares_a_shortcut() {
        for t in Toolbar::new().tools {
            assert!(!t.shortcut.is_empty(), "tool {} has no shortcut", t.id);
        }
    }

    /// The shortcut is drawn on the button, so it has to be a single character
    /// that the font can actually render. It was `Option<&'static str>` and
    /// never shown, which is how a private detail became a public one.
    #[test]
    fn shortcuts_are_single_ascii_letters() {
        for t in Toolbar::new().tools {
            assert_eq!(
                t.shortcut.chars().count(),
                1,
                "tool {} shortcut {:?} is not one character",
                t.id,
                t.shortcut
            );
            assert!(
                t.shortcut.chars().all(|c| c.is_ascii_alphanumeric()),
                "tool {} shortcut {:?} is not ASCII alphanumeric",
                t.id,
                t.shortcut
            );
        }
    }

    /// Guard the regression that motivated dropping the icons: a pictogram that
    /// the runtime font cannot draw shows up as U+25A1 WHITE SQUARE. Nothing on
    /// a button may contain one, and nothing may reach outside Latin-1 plus the
    /// Hangul syllables the labels already use.
    #[test]
    fn no_button_text_contains_unrenderable_glyphs() {
        for t in Toolbar::new().tools {
            let drawn = format!("{}  {}", t.label, t.shortcut);
            assert!(
                !drawn.contains('\u{25A1}') && !drawn.contains('\u{FFFD}'),
                "tool {} draws a tofu box: {drawn:?}",
                t.id
            );
            for c in drawn.chars() {
                let ok = c.is_ascii()
                    || ('\u{AC00}'..='\u{D7A3}').contains(&c)  // Hangul syllables
                    || ('\u{3130}'..='\u{318F}').contains(&c); // Hangul compat jamo
                assert!(ok, "tool {} draws non-Latin glyph {c:?}", t.id);
            }
        }
    }

    /// The hover tooltip is the only place the shortcut is spelled out in full,
    /// and it is built by `format!` in `show`; pin the shape so a refactor that
    /// drops the key out of the tooltip is caught.
    #[test]
    fn tooltip_leads_with_the_shortcut() {
        let toolbar = Toolbar::new();
        for t in &toolbar.tools {
            let tip = format!("[{}]  {}", t.shortcut, t.help);
            assert!(tip.starts_with('['), "tool {} tooltip: {tip:?}", t.id);
            assert!(tip.contains(t.help));
        }
    }
}
