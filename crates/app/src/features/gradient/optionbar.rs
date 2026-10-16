//! Gradient option block.
use crate::session_glue::AppState;
use egui::{DragValue, RichText, Ui};

pub fn gradient_options(
    ui: &mut Ui,
    _juice_clicks: &mut std::collections::HashMap<String, f64>,
    app: &mut AppState,
) {
    let Some(gradient) = app
        .tools
        .get_mut("radial_gradient")
        .and_then(|t| t.gradient_tool_mut())
    else {
        return;
    };
    ui.label(RichText::new("Inner radius:").size(12.0));
    let mut radius = gradient.inner_radius() as u32;
    if ui
        .add(
            DragValue::new(&mut radius)
                .range(0..=2048)
                .speed(1.0)
                .suffix(" px"),
        )
        .changed()
    {
        gradient.set_inner_radius(f64::from(radius));
    }
    let solid = gradient.edge_mode_str() == "solid";
    if ui
        .button(if solid { "transparent" } else { "solid" })
        .clicked()
    {
        gradient.set_edge_mode(if solid { "transparent" } else { "solid" });
    }
}
