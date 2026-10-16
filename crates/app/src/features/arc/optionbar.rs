//! Arc / chord option block.
use crate::session_glue::AppState;
use egui::{DragValue, RichText, Ui};

pub fn arc_options(
    ui: &mut Ui,
    _juice_clicks: &mut std::collections::HashMap<String, f64>,
    app: &mut AppState,
    tool: &str,
) {
    let tool_id = if tool == "chord" { "chord" } else { "arc" };
    let Some(arc) = app.tools.get_mut(tool_id).and_then(|t| t.arc_tool_mut()) else {
        return;
    };
    ui.label(RichText::new("Start angle:").size(12.0));
    let mut start = arc.start();
    if ui
        .add(DragValue::new(&mut start).speed(1.0).suffix(" deg"))
        .changed()
    {
        arc.set_start(start);
    }
    ui.label(RichText::new("End angle:").size(12.0));
    let mut end = arc.end();
    if ui
        .add(DragValue::new(&mut end).speed(1.0).suffix(" deg"))
        .changed()
    {
        arc.set_end(end);
    }
    ui.label(RichText::new("Width:").size(12.0));
    let mut width = arc.width();
    if ui
        .add(
            DragValue::new(&mut width)
                .range(1..=64)
                .speed(1.0)
                .suffix(" px"),
        )
        .changed()
    {
        arc.set_width(width);
    }
    if tool == "chord" {
        let fill = arc.fill();
        if ui.button(if fill { "fill" } else { "outline" }).clicked() {
            arc.set_fill(!fill);
        }
    }
}
