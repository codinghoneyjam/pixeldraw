//! Arc / chord option block: start/end angles, line width, and the chord
//! fill-vs-outline toggle (chord only).
use super::tools::tool_mut;
use crate::session_glue::AppState;
use crate::tools::arc::ArcTool;
use crate::ui::optionbar::switch::segmented_switch;
use egui::{DragValue, RichText, Ui};

pub fn arc_options(
    ui: &mut Ui,
    juice_clicks: &mut std::collections::HashMap<String, f64>,
    app: &mut AppState,
    tool: &str,
) {
    let tool_id = if tool == "chord" { "chord" } else { "arc" };
    let Some(arc) = tool_mut::<ArcTool>(app, tool_id) else {
        return;
    };
    ui.label(RichText::new("시작 각도:").size(12.0));
    let mut start = arc.start();
    if ui
        .add(DragValue::new(&mut start).speed(1.0).suffix("°"))
        .changed()
    {
        arc.set_start(start);
    }
    ui.label(RichText::new("끝 각도:").size(12.0));
    let mut end = arc.end();
    if ui
        .add(DragValue::new(&mut end).speed(1.0).suffix("°"))
        .changed()
    {
        arc.set_end(end);
    }
    ui.label(RichText::new("선폭:").size(12.0));
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
        // One toggle button, same control as the shape mode above — the two
        // half labels are the two states, not two clickable radios.
        let fill = arc.fill();
        if segmented_switch(ui, juice_clicks, "chord:mode", true, "테두리", "채움", fill) {
            arc.set_fill(!fill);
        }
    }
}
