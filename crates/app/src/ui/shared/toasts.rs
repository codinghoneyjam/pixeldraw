//! Toast notification display.

use crate::ui::{Toast, ToastKind, UiState};
use egui::{Color32, CornerRadius, RichText, Vec2};

pub fn show(ui: &mut egui::Ui, ui_state: &mut UiState) {
    let ctx = ui.ctx();
    let now = ctx.input(|i| i.time);

    // Update and filter toasts
    ui_state.toasts.retain_mut(|toast| {
        if toast.created == 0.0 {
            toast.created = now;
        }
        now - toast.created < toast.duration as f64
    });

    // Render toasts in top-right
    if !ui_state.toasts.is_empty() {
        egui::Area::new("toasts".into())
            .anchor(egui::Align2::RIGHT_TOP, Vec2::new(-16.0, 16.0))
            .show(ui.ctx(), |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    for toast in &ui_state.toasts {
                        let (bg, text_color) = match toast.kind {
                            ToastKind::Info => (Color32::from_rgb(30, 60, 120), Color32::WHITE),
                            ToastKind::Success => (Color32::from_rgb(30, 100, 30), Color32::WHITE),
                            ToastKind::Warning => (Color32::from_rgb(120, 90, 0), Color32::WHITE),
                            ToastKind::Error => (Color32::from_rgb(120, 30, 30), Color32::WHITE),
                        };
                        let remaining = (toast.duration as f64 - (now - toast.created)).max(0.0);
                        let alpha = (remaining / toast.duration as f64 * 255.0) as u8;

                        egui::Frame::new()
                            .fill(Color32::from_rgba_premultiplied(
                                bg.r(),
                                bg.g(),
                                bg.b(),
                                alpha,
                            ))
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(Vec2::new(12.0, 8.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new(&toast.message).size(12.0).color(
                                    Color32::from_rgba_premultiplied(
                                        text_color.r(),
                                        text_color.g(),
                                        text_color.b(),
                                        alpha,
                                    ),
                                ));
                            });
                    }
                });
            });
    }
}

/// Helper to add a toast to the state.
pub fn add_toast(ui_state: &mut UiState, toast: Toast) {
    ui_state.toasts.push(toast);
}

/// Convenience functions.
pub fn info(ui_state: &mut UiState, message: impl Into<String>) {
    add_toast(ui_state, Toast::info(message));
}
pub fn success(ui_state: &mut UiState, message: impl Into<String>) {
    add_toast(ui_state, Toast::success(message));
}
pub fn warning(ui_state: &mut UiState, message: impl Into<String>) {
    add_toast(ui_state, Toast::warning(message));
}
pub fn error(ui_state: &mut UiState, message: impl Into<String>) {
    add_toast(ui_state, Toast::error(message));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::UiState;

    #[test]
    fn add_toast_increments_count() {
        let mut state = UiState::default();
        add_toast(&mut state, Toast::info("test"));
        assert_eq!(state.toasts.len(), 1);
    }
}
