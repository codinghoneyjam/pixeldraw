//! Statusbar — port of `src/ui/statusbar/statusbar.js`.

use super::UiState;
use crate::session_glue::AppState;
use egui::{Color32, RichText, Ui, Vec2};

pub struct Statusbar;

impl Statusbar {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut Ui, state: &mut UiState, app: &mut AppState) {
        // Update state from app
        if let Some(doc) = app.session.doc() {
            state.statusbar.document_size = Some((doc.canvas.width_px, doc.canvas.height_px));
            state.statusbar.layer_info = Some(format!(
                "레이어: {} / {}",
                doc.layers
                    .iter()
                    .position(|l| l.id == doc.active_layer_id)
                    .unwrap_or(0)
                    + 1,
                doc.layers.len()
            ));
        }
        state.statusbar.zoom = app.view_store.view().zoom() as f32;

        egui::Panel::bottom("statusbar")
            .frame(egui::Frame::new().inner_margin(Vec2::new(12.0, 4.0)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(16.0, 0.0);

                    // Left: status messages
                    if let Some(last) = state.statusbar.status_messages.last() {
                        ui.label(RichText::new(last).size(12.0).color(Color32::LIGHT_GRAY));
                    } else if let Some(doc) = app.session.doc() {
                        ui.label(
                            RichText::new(format!(
                                "{} × {} px",
                                doc.canvas.width_px, doc.canvas.height_px
                            ))
                            .size(12.0)
                            .color(Color32::LIGHT_GRAY),
                        );
                    }

                    ui.separator();

                    // Center: cursor position
                    if let Some((x, y)) = state.statusbar.cursor_pos {
                        ui.label(
                            RichText::new(format!("X: {}  Y: {}", x, y))
                                .size(12.0)
                                .monospace(),
                        );
                    } else {
                        ui.label(
                            RichText::new("X: —  Y: —")
                                .size(12.0)
                                .monospace()
                                .color(Color32::GRAY),
                        );
                    }

                    ui.separator();

                    // Right: zoom + layer info
                    ui.label(
                        RichText::new(format!("{:.0}%", state.statusbar.zoom * 100.0))
                            .size(12.0)
                            .monospace(),
                    );
                    ui.separator();
                    if let Some(layer_info) = &state.statusbar.layer_info {
                        ui.label(
                            RichText::new(layer_info)
                                .size(12.0)
                                .color(Color32::LIGHT_GRAY),
                        );
                    }
                });
            });
    }
}

impl Default for Statusbar {
    fn default() -> Self {
        Self::new()
    }
}

// Integration function
pub fn show(ui: &mut egui::Ui, ui_state: &mut UiState, app_state: &mut AppState) {
    let mut statusbar = Statusbar::new();
    egui::Panel::bottom("statusbar_panel")
        .frame(egui::Frame::new())
        .show(ui, |ui| {
            statusbar.show(ui, ui_state, app_state);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statusbar_creation() {
        let bar = Statusbar::new();
        let _ = bar;
    }
}
