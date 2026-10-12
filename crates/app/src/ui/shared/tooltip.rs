//! Tooltip display.

use crate::ui::UiState;
use egui::{Color32, CornerRadius, RichText, Vec2};

pub fn show(ui: &mut egui::Ui, ui_state: &mut UiState) {
    if let Some(tooltip) = &mut ui_state.tooltip
        && tooltip.visible
        && let Some(pos) = tooltip.position
    {
        egui::Area::new("tooltip".into())
            .fixed_pos(pos + Vec2::new(16.0, 16.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(40, 40, 40))
                    .corner_radius(CornerRadius::same(4))
                    .inner_margin(Vec2::new(8.0, 4.0))
                    .stroke(egui::Stroke::new(1.0, Color32::from_gray(80)))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&tooltip.text)
                                .size(11.0)
                                .color(Color32::WHITE),
                        );
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::Tooltip;

    #[test]
    fn tooltip_creation() {
        let t = Tooltip::new("Test tooltip");
        assert_eq!(t.text, "Test tooltip");
        assert!(!t.visible);
    }
}
