//! Text option block: content/font/stroke, then the shared geometry actions.
use crate::features::geom_options::geometry_and_actions;
use crate::session_glue::AppState;
use egui::{DragValue, RichText, Ui};

const FONTS: [(&str, &str); 2] = [
    ("default-42", "기본 42px"),
    ("monogram-240", "모노그램 240px"),
];

pub fn text_options(
    ui: &mut Ui,
    juice_clicks: &mut std::collections::HashMap<String, f64>,
    app: &mut AppState,
) {
    {
        let Some(text_tool) = app.tools.get_mut("text").and_then(|t| t.text_tool_mut()) else {
            return;
        };
        ui.label(RichText::new("텍스트:").size(12.0));
        let mut text = text_tool.text().to_string();
        if ui.text_edit_singleline(&mut text).changed() {
            text_tool.set_text(text);
        }
        ui.label(RichText::new("글꼴:").size(12.0));
        let mut font = text_tool.font().to_string();
        let selected_label: String = FONTS
            .iter()
            .find(|(id, _)| *id == font)
            .map(|(_, label)| label.to_string())
            .unwrap_or_else(|| font.clone());
        egui::ComboBox::from_id_salt("text_font")
            .selected_text(selected_label)
            .show_ui(ui, |ui| {
                for (id, label) in FONTS {
                    ui.selectable_value(&mut font, id.to_string(), label);
                }
            });
        if font != text_tool.font() {
            text_tool.set_font(font);
        }
        ui.label(RichText::new("외곽선:").size(12.0));
        let mut stroke = text_tool.stroke_width();
        if ui
            .add(
                DragValue::new(&mut stroke)
                    .range(0..=16)
                    .speed(1.0)
                    .suffix(" px"),
            )
            .changed()
        {
            text_tool.set_stroke_width(stroke);
        }
    }
    ui.separator();
    geometry_and_actions(ui, juice_clicks, app, "text");
}
