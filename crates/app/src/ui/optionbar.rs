//! Optionbar — port of `src/ui/optionbar/panel_options.js`.

use super::{ToolOptions, UiState};
use crate::session_glue::AppState;
use egui::{Color32, ComboBox, DragValue, RichText, Slider, Ui, Vec2};

pub struct Optionbar;

impl Optionbar {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut Ui, state: &mut UiState, app: &mut AppState) {
        let active_tool = &state.toolbar.active_tool;
        let options = state
            .optionbar
            .current_tool_options
            .get_or_insert_with(|| self.default_options(active_tool));

        egui::Frame::new()
            .inner_margin(Vec2::new(12.0, 6.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(12.0, 8.0);

                    // Tool-specific options
                    match active_tool.as_str() {
                        "pen" | "eraser" | "line" => self.pen_options(ui, options),
                        "rect" | "ellipse" => self.shape_options(ui, options),
                        "fill" => self.fill_options(ui, options, app),
                        "text" => self.text_options(ui, options),
                        "gradient" => self.gradient_options(ui, options),
                        "shape" => self.shape_tool_options(ui, options),
                        _ => self.generic_options(ui, options),
                    }

                    ui.separator();

                    // Common: opacity
                    ui.label("불투명도:");
                    if let Some(op) = &mut options.opacity {
                        let mut val = *op;
                        if ui.add(Slider::new(&mut val, 0.0..=1.0).text("")).changed() {
                            options.opacity = Some(val);
                        }
                    }

                    // Common: blend mode (for brush tools)
                    if matches!(active_tool.as_str(), "pen" | "eraser" | "fill") {
                        ui.label("블렌드:");
                        let modes = ["Normal", "Multiply", "Screen", "Overlay"];
                        ComboBox::from_id_salt("blend_mode")
                            .selected_text("Normal")
                            .show_ui(ui, |ui| {
                                for m in modes {
                                    ui.selectable_value(&mut options.opacity, Some(1.0), m);
                                }
                            });
                    }
                });
            });
    }

    fn default_options(&self, tool: &str) -> ToolOptions {
        ToolOptions {
            size: Some(match tool {
                "pen" | "eraser" | "line" => 4,
                "fill" => 1,
                _ => 2,
            }),
            opacity: Some(1.0),
            hardness: if tool == "pen" { Some(1.0) } else { None },
            fill_color: None,
            stroke_color: None,
        }
    }

    fn pen_options(&self, ui: &mut Ui, options: &mut ToolOptions) {
        ui.label("크기:");
        if let Some(size) = &mut options.size {
            ui.add(DragValue::new(size).range(1..=256).speed(1.0).suffix(" px"));
        }
        ui.label("경도:");
        if let Some(hardness) = &mut options.hardness {
            let mut val = *hardness;
            if ui.add(Slider::new(&mut val, 0.0..=1.0).text("")).changed() {
                options.hardness = Some(val);
            }
        }
    }

    fn shape_options(&self, ui: &mut Ui, options: &mut ToolOptions) {
        ui.label("두께:");
        if let Some(size) = &mut options.size {
            ui.add(DragValue::new(size).range(1..=50).speed(1.0).suffix(" px"));
        }
        ui.label("모서리 반경:");
        if let Some(radius) = &mut options.hardness {
            ui.add(
                DragValue::new(radius)
                    .range(0..=100)
                    .speed(1.0)
                    .suffix(" px"),
            );
        }
    }

    fn fill_options(&self, ui: &mut Ui, options: &mut ToolOptions, _app: &mut AppState) {
        ui.label("모드:");
        ComboBox::from_id_salt("fill_mode")
            .selected_text("연속")
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut options.hardness, Some(0.0), "연속");
                ui.selectable_value(&mut options.hardness, Some(1.0), "비연속");
            });

        ui.label("허용 오차:");
        if let Some(tol) = &mut options.opacity {
            let mut val = *tol * 255.0;
            if ui
                .add(Slider::new(&mut val, 0.0..=255.0).text(""))
                .changed()
            {
                options.opacity = Some(val / 255.0);
            }
        }
    }

    fn text_options(&self, ui: &mut Ui, options: &mut ToolOptions) {
        ui.label("폰트 크기:");
        if let Some(size) = &mut options.size {
            ui.add(DragValue::new(size).range(6..=200).speed(1.0).suffix(" pt"));
        }
        ui.label("색상:");
        if let Some(fill) = &mut options.fill_color {
            let mut color = [
                fill[0] as f32 / 255.0,
                fill[1] as f32 / 255.0,
                fill[2] as f32 / 255.0,
                fill[3] as f32 / 255.0,
            ];
            if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                *fill = [
                    (color[0] * 255.0) as u8,
                    (color[1] * 255.0) as u8,
                    (color[2] * 255.0) as u8,
                    (color[3] * 255.0) as u8,
                ];
            }
        }
    }

    fn gradient_options(&self, ui: &mut Ui, options: &mut ToolOptions) {
        ui.label("타입:");
        ComboBox::from_id_salt("gradient_type")
            .selected_text("선형")
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut options.size, Some(0), "선형");
                ui.selectable_value(&mut options.size, Some(1), "방사형");
            });
    }

    fn shape_tool_options(&self, ui: &mut Ui, options: &mut ToolOptions) {
        ui.label("도형:");
        ComboBox::from_id_salt("shape_kind")
            .selected_text("사각형")
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut options.size, Some(0), "사각형");
                ui.selectable_value(&mut options.size, Some(1), "타원");
                ui.selectable_value(&mut options.size, Some(2), "둥근 사각형");
                ui.selectable_value(&mut options.size, Some(3), "다각형");
                ui.selectable_value(&mut options.size, Some(4), "선");
            });
    }

    fn generic_options(&self, ui: &mut Ui, _options: &mut ToolOptions) {
        ui.label(RichText::new("이 도구에 대한 옵션이 없습니다").color(Color32::GRAY));
    }
}

impl Default for Optionbar {
    fn default() -> Self {
        Self::new()
    }
}

// Integration function.
//
// `render_ui` in `super::mod` already owns `Panel::top("optionbar_panel")`.
// This used to open the *same* panel id again, nesting a panel inside itself,
// and `Optionbar::show` then opened a third (`Panel::top("optionbar")`) inside
// that. egui panels are siblings: each nested one is handed only the space the
// outer one left over, which clipped the option row. Now the layout happens
// directly in the caller's `Ui`.
pub fn show(ui: &mut egui::Ui, ui_state: &mut UiState, app_state: &mut AppState) {
    let mut optionbar = Optionbar::new();
    optionbar.show(ui, ui_state, app_state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optionbar_default_options() {
        let bar = Optionbar::new();
        let opts = bar.default_options("pen");
        assert_eq!(opts.size, Some(4));
        assert_eq!(opts.opacity, Some(1.0));
        assert_eq!(opts.hardness, Some(1.0));
    }
}
