//! The palette and recent-colour sections.
//!
//! Both draw the same 8-column swatch grid; they differ only in the source
//! (the frozen 40-colour `DEFAULT_PALETTE` vs the recent list, whose unfilled
//! cells stay placeholders) and in which slot they write to.
use super::super::UiState;
use super::chrome::{
    paint_empty_swatch, paint_swatch, section_card, section_header, slot_mark, swatch_cell,
};
use super::color::apply_color;
use super::color::{push_recent, to_color32};
use crate::session_glue::AppState;
use egui::{Color32, Ui};

pub const DEFAULT_PALETTE: [&str; 40] = [
    "#ffffff", "#ffcdd2", "#ffe0b2", "#fff9c4", "#c8e6c9", "#b2ebf2", "#bbdefb", "#e1bee7",
    "#dedede", "#ef5350", "#ffb74d", "#fff176", "#81c784", "#4dd0e1", "#64b5f6", "#ba68c8",
    "#9e9e9e", "#e53935", "#fb8c00", "#fdd835", "#43a047", "#00acc1", "#1e88e5", "#8e24aa",
    "#616161", "#b71c1c", "#e65100", "#f9a825", "#1b5e20", "#006064", "#0d47a1", "#4a148c",
    "#000000", "#5f0a15", "#6d2f00", "#6e5b00", "#103516", "#0f3c45", "#08214d", "#230a32",
];

/// JS `mountPalette`: left click = active slot, right click = secondary slot.
pub fn show_palette(ui: &mut Ui, ui_state: &mut UiState, app: &mut AppState) {
    // ── Palette section ─────────────────────────────────────────────────
    // Right-click loads the secondary slot (JS `mountPalette` context menu).
    section_card(ui, |ui| {
        if !section_header(ui, &mut ui_state.panels, "palette", "팔레트") {
            return;
        }
        let settings = app.session.settings();
        let primary = settings.primary_color.clone();
        let secondary = settings.secondary_color.clone();
        let cell = swatch_cell(ui);
        egui::Grid::new("palette_grid")
            .num_columns(8)
            .spacing(egui::vec2(2.0, 2.0))
            .show(ui, |ui| {
                for (i, hex) in DEFAULT_PALETTE.iter().enumerate() {
                    let c = to_color32(hex).unwrap_or(Color32::GRAY);
                    // Primary blue brackets, secondary amber (`slot_mark`).
                    let mark = slot_mark(&primary, &secondary, hex);
                    let resp = paint_swatch(
                        ui,
                        &mut ui_state.juice_clicks,
                        &format!("pal:{i}"),
                        c,
                        cell,
                        mark,
                    );
                    if resp.clicked() {
                        let secondary = ui_state.panels.secondary_active;
                        apply_color(app, &mut ui_state.panels, secondary, hex);
                        push_recent(&mut ui_state.panels.recent, hex.to_string());
                    }
                    if resp.secondary_clicked() {
                        apply_color(app, &mut ui_state.panels, true, hex);
                        push_recent(&mut ui_state.panels.recent, hex.to_string());
                    }
                    if (i + 1) % 8 == 0 {
                        ui.end_row();
                    }
                }
            });
    });
}

/// JS `paintRecent`: fixed 8-cell row, unused slots stay placeholders.
pub fn show_recent(ui: &mut Ui, ui_state: &mut UiState, app: &mut AppState) {
    // ── Recent colors section ───────────────────────────────────────────
    // JS `paintRecent`: fixed 8-cell row, unused slots stay placeholders.
    section_card(ui, |ui| {
        if !section_header(ui, &mut ui_state.panels, "recent", "최근 색") {
            return;
        }
        let settings = app.session.settings();
        let primary = settings.primary_color.clone();
        let secondary = settings.secondary_color.clone();
        let cell = swatch_cell(ui);
        egui::Grid::new("recent_grid")
            .num_columns(8)
            .spacing(egui::vec2(2.0, 2.0))
            .show(ui, |ui| {
                for i in 0..8 {
                    let hex = ui_state.panels.recent.get(i).cloned().unwrap_or_default();
                    match to_color32(&hex) {
                        Some(c) if !hex.is_empty() => {
                            let mark = slot_mark(&primary, &secondary, &hex);
                            let resp = paint_swatch(
                                ui,
                                &mut ui_state.juice_clicks,
                                &format!("rec:{i}"),
                                c,
                                cell,
                                mark,
                            );
                            if resp.clicked() {
                                let secondary = ui_state.panels.secondary_active;
                                apply_color(app, &mut ui_state.panels, secondary, &hex);
                                push_recent(&mut ui_state.panels.recent, hex.to_string());
                            }
                            if resp.secondary_clicked() {
                                apply_color(app, &mut ui_state.panels, true, &hex);
                            }
                        }
                        // Empty placeholder cell (JS `.dt-swatch.is-empty`).
                        _ => paint_empty_swatch(ui, cell),
                    }
                    if (i + 1) % 8 == 0 {
                        ui.end_row();
                    }
                }
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_has_40_colors() {
        assert_eq!(DEFAULT_PALETTE.len(), 40);
    }
}
