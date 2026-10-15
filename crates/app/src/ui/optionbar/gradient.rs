//! Gradient option block: inner radius and the transparent/solid edge mode.
use super::tools::tool_mut;
use crate::session_glue::AppState;
use crate::tools::gradient::GradientTool;
use crate::ui::optionbar::switch::segmented_switch;
use egui::{DragValue, RichText, Ui};

pub fn gradient_options(
    ui: &mut Ui,
    juice_clicks: &mut std::collections::HashMap<String, f64>,
    app: &mut AppState,
) {
    let Some(gradient) = tool_mut::<GradientTool>(app, "radial_gradient") else {
        return;
    };
    ui.label(RichText::new("안쪽 반경:").size(12.0));
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
    // Same single-button segmented switch as the fill modes, so the bar's
    // geometry does not depend on which tool is active.
    if segmented_switch(
        ui,
        juice_clicks,
        "gradient:edge",
        true,
        "투명",
        "단색",
        solid,
    ) {
        gradient.set_edge_mode(if solid { "transparent" } else { "solid" });
    }
}
