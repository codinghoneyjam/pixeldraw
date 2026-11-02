//! Arc / chord option block: mode + angles + width, then the shared geometry.
use crate::features::geom_options::geometry_and_actions;
use crate::session_glue::AppState;
use egui::{DragValue, RichText, Ui};

pub fn arc_options(
    ui: &mut Ui,
    juice_clicks: &mut std::collections::HashMap<String, f64>,
    app: &mut AppState,
    tool: &str,
) {
    let tool_id = if tool == "chord" { "chord" } else { "arc" };
    {
        let Some(arc) = app.tools.get_mut(tool_id).and_then(|t| t.arc_tool_mut()) else {
            return;
        };
        // `deg` is a unit label, not part of the number the user edits.
        // Keeping it outside also leaves the text field free for 0..=360.
        ui.spacing_mut().interact_size.x = 56.0;
        ui.label(RichText::new("시작 각도:").size(12.0));
        let mut start = arc.start();
        if ui
            .add(
                DragValue::new(&mut start)
                    .range(0.0..=360.0)
                    .speed(1.0)
                    .max_decimals(1),
            )
            .changed()
        {
            arc.set_start(start);
        }
        ui.label(RichText::new("deg").size(11.0));
        ui.label(RichText::new("끝 각도:").size(12.0));
        let mut end = arc.end();
        if ui
            .add(
                DragValue::new(&mut end)
                    .range(0.0..=360.0)
                    .speed(1.0)
                    .max_decimals(1),
            )
            .changed()
        {
            arc.set_end(end);
        }
        ui.label(RichText::new("deg").size(11.0));
        ui.label(RichText::new("두께:").size(12.0));
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
            if ui.button(if fill { "윤곽선" } else { "채우기" }).clicked() {
                arc.set_fill(!fill);
            }
        }
    }
    ui.separator();
    geometry_and_actions(ui, juice_clicks, app, tool_id);
}
