//! Text option block: text input (applies per keystroke), font select, and
//! the stroke width.
use super::tools::tool_mut;
use crate::session_glue::AppState;
use crate::tools::text::TextTool;
use egui::{DragValue, RichText, Ui};

const FONTS: [(&str, &str); 2] = [
    ("default-42", "기본 42px"),
    ("monogram-240", "모노그램 240px"),
];

pub fn text_options(ui: &mut Ui, app: &mut AppState) {
    let Some(text_tool) = tool_mut::<TextTool>(app, "text") else {
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
                if ui
                    .selectable_value(&mut font, id.to_string(), label)
                    .clicked()
                {
                    text_tool.set_font(id);
                }
            }
        });
    ui.label(RichText::new("외곽선:").size(12.0));
    let mut width = text_tool.stroke_width();
    if ui
        .add(
            DragValue::new(&mut width)
                .range(0..=16)
                .speed(1.0)
                .suffix(" px"),
        )
        .changed()
    {
        text_tool.set_stroke_width(width);
    }
}
