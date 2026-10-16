//! Optionbar — routes the active tool to its own feature block.
//!
//! Each block lives beside its feature (`features/<tool>/optionbar.rs`), the
//! way the JS tree put every control inside the feature module that owns it. This
//! file only decides which block is visible, so a feature can be read — and
//! changed — in one place.

use crate::features::{arc, gradient, pen, shape, text};
use crate::session_glue::AppState;
use crate::ui::{ToolOptions, UiState};
use egui::{Color32, RichText, Ui, Vec2};

pub fn show(ui: &mut Ui, ui_state: &mut UiState, app: &mut AppState) {
    let tool = ui_state.toolbar.active_tool.clone();

    egui::Frame::new()
        .inner_margin(Vec2::new(12.0, 6.0))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(12.0, 8.0);
                // Split the borrow: the option block needs `juice_clicks` and
                // the pen size at once, and both are separate `UiState` fields.
                let UiState {
                    optionbar,
                    juice_clicks,
                    ..
                } = ui_state;

                match tool.as_str() {
                    "pen" | "eraser" => {
                        let options =
                            optionbar
                                .current_tool_options
                                .get_or_insert_with(|| ToolOptions {
                                    size: Some(app.session.settings().pen_size),
                                });
                        pen::optionbar::pen_options(ui, juice_clicks, options, app);
                    }
                    "line" | "rect" | "rrect" | "ellipse" | "polygon" => {
                        shape::optionbar::shape_options(ui, juice_clicks, app, &tool);
                    }
                    "arc" | "chord" => {
                        arc::optionbar::arc_options(ui, juice_clicks, app, &tool);
                    }
                    "radial_gradient" => {
                        gradient::optionbar::gradient_options(ui, juice_clicks, app);
                    }
                    "text" => text::optionbar::text_options(ui, app),
                    // fill / eyedropper / hand own no controls — the JS bar is
                    // empty for them too, so do not draw a placeholder.
                    _ => {}
                }
            });
        });
}

/// Kept for the panels that still fall back to a hint row.
#[allow(dead_code)]
pub(crate) fn no_options_note(ui: &mut Ui) {
    ui.label(RichText::new("이 도구에 대한 옵션이 없습니다").color(Color32::GRAY));
}
