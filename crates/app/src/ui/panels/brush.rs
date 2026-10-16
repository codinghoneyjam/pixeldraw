//! The brush section: the size row and the banded size presets.
//!
//! The size row mirrors the layer panel opacity row (label, slider with a
//! trailing fill, number box, unit) so both sides of the window speak one
//! control language. The presets below it are square cells half a palette
//! swatch on each side, laid out one row per size band, not a fixed 4x4.
use super::super::UiState;
use super::chrome::{EDGE, section_card, section_header, swatch_cell};
use crate::session_glue::AppState;
use egui::{Color32, DragValue, RichText, Slider, Ui};

pub const SIZE_PRESETS: [u32; 16] = [1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64];

/// Preset size bands: one band per row, so a 1px tip never shares a line with
/// a 4×4 grid; the native panel lays the
/// same sixteen presets out per size range instead, which is what makes the
/// rows ordered-by-magnitude rather than a shuffled square.
pub const SIZE_BANDS: [(&str, &[u32]); 3] = [
    ("1-8", &[1, 2, 3, 4, 5, 6, 8]),
    ("10-24", &[10, 12, 16, 20, 24]),
    ("32-64", &[32, 40, 48, 64]),
];

/// The preset cells are **half** a palette swatch on each side — a quarter of
/// its area — and square. A 24px swatch therefore gives a 12px cell.
pub fn preset_cell(swatch: f32) -> f32 {
    (swatch * 0.5).clamp(10.0, 16.0)
}

/// Font size that still fits two digits inside the cell.
fn preset_font(cell: f32) -> f32 {
    (cell * 0.62).clamp(9.0, 12.0)
}

/// One size preset: a square cell, hover-pale + press-pop, accent-filled while
/// it is the active size. The number labels the target, so a 1px tip occupies
/// exactly as much space as a 64px blob.
pub(crate) fn paint_size_preset(
    ui: &mut Ui,
    clicks: &mut std::collections::HashMap<String, f64>,
    key: &str,
    size: u32,
    cell: f32,
    active: bool,
) -> egui::Response {
    use crate::ui::juice::{click_pop, hover_fade, lerp_color};
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(cell, cell), egui::Sense::click());
    let hid = ui.id().with("pr").with(key);
    let f = hover_fade(ui.ctx(), hid, resp.hovered());
    let pop = click_pop(clicks, ui.ctx(), key, resp.clicked());
    let rect = rect.shrink((1.0 - pop) * cell * 0.14);
    let painter = ui.painter();
    let base = if active {
        Color32::from_rgb(0x00, 0x96, 0xff)
    } else {
        Color32::from_rgb(0x23, 0x24, 0x28)
    };
    painter.rect_filled(rect, 2.0, lerp_color(base, Color32::WHITE, f * 0.12));
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(
            1.0,
            if active {
                Color32::from_rgb(0x9a, 0xdc, 0xff)
            } else {
                EDGE
            },
        ),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{size}"),
        egui::FontId::proportional(preset_font(cell)),
        if active {
            Color32::WHITE
        } else {
            Color32::from_rgb(0xd0, 0xd4, 0xd9)
        },
    );
    resp.clone().on_hover_text(format!("굵기 {size}px"));
    resp
}

/// JS `panel_brush.js`: size row + size presets.
pub fn show_brush(ui: &mut Ui, ui_state: &mut UiState, app: &mut AppState) {
    // —— Brush section ——————————————————————————————————————————————————
    // JS `panel_brush.js`: number input + range slider + preview + size presets.
    section_card(ui, |ui| {
        if !section_header(ui, &mut ui_state.panels, "brush", "브러시") {
            return;
        }
        let size = app.session.settings().pen_size;
        let mut v = size;

        ui.horizontal(|ui| {
            ui.label(RichText::new("굵기:").size(12.0));
            // Trailing fill + hidden value: the number box is the readout, the
            // slider is the gesture —— exactly the layer panel's opacity row.
            let slider = ui.add(
                Slider::new(&mut v, 1..=64)
                    .show_value(false)
                    .trailing_fill(true),
            );
            let num = ui.add(DragValue::new(&mut v).range(1..=64).speed(1.0));
            ui.label("px");
            if num.changed() {
                let _ = app.session.set_setting(
                    "penSize",
                    pixel_engine::model::SettingInput::Num(f64::from(v)),
                );
            } else {
                if slider.changed() {
                    let _ = app.session.set_setting(
                        "penSize",
                        pixel_engine::model::SettingInput::Num(f64::from(v)),
                    );
                }
                if slider.drag_stopped() {
                    let _ = app.session.set_setting(
                        "penSize",
                        pixel_engine::model::SettingInput::Num(f64::from(v)),
                    );
                }
            }

            // Row 1 —— the same shape as the layers' opacity row: label, slider,
            // number box, unit. No preview here: it used to squeeze the slider
        });

        ui.add_space(4.0);
        // Footprint preview on the left, the size bands stacked beside it —— the
        // extra ragged space on the short lower rows would otherwise be dead.
        ui.horizontal(|ui| {
            let preview = (v as f32 * 0.5).clamp(1.0, 14.0);
            let (preview_rect, _) =
                ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(preview_rect, 3.0, Color32::from_rgb(0x23, 0x24, 0x28));
            painter.rect_stroke(
                preview_rect,
                3.0,
                egui::Stroke::new(1.0, EDGE),
                egui::StrokeKind::Inside,
            );
            painter.circle_filled(preview_rect.center(), preview, Color32::WHITE);

            // Preset grid, one row per size band, cells half a swatch on each
            // side (a quarter of its area), so the numbers never compete with
            let cell = preset_cell(swatch_cell(ui));
            let current = app.session.settings().pen_size;
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                for (i, (band, sizes)) in SIZE_BANDS.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
                        ui.add_space(4.0);
                        let (label_rect, _) =
                            ui.allocate_exact_size(egui::vec2(38.0, cell), egui::Sense::empty());
                        ui.painter().text(
                            label_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            band,
                            egui::FontId::proportional(preset_font(cell)),
                            Color32::from_rgb(0x7a, 0x7f, 0x87),
                        );
                        for size in sizes.iter() {
                            let size = *size;
                            let resp = paint_size_preset(
                                ui,
                                &mut ui_state.juice_clicks,
                                &format!("preset:{size}"),
                                size,
                                cell,
                                size == current,
                            );
                            if resp.clicked() {
                                let _ = app.session.set_setting(
                                    "penSize",
                                    pixel_engine::model::SettingInput::Num(f64::from(size)),
                                );
                            }
                        }
                    });
                    let _ = i;
                }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::super::chrome::swatch_cell_width;
    use super::*;

    #[test]
    fn presets_cover_js_range() {
        assert_eq!(SIZE_PRESETS.len(), 16);
        assert_eq!(SIZE_PRESETS[0], 1);
        assert_eq!(SIZE_PRESETS[15], 64);
    }

    /// The banded layout is only valid if the bands really are the frozen
    /// preset list: every size exactly once, ascending, no gap between bands.
    #[test]
    fn preset_bands_are_exactly_the_preset_list() {
        let flat: Vec<u32> = SIZE_BANDS
            .iter()
            .flat_map(|(_, sizes)| sizes.iter().copied())
            .collect();
        assert_eq!(flat, SIZE_PRESETS.to_vec());
        // Three rows of 7 / 5 / 4, not the 4x4 the JS port used.
        assert_eq!(SIZE_BANDS.len(), 3);
        assert_eq!(SIZE_BANDS[0].1.len(), 7);
        assert_eq!(SIZE_BANDS[2].1.len(), 4);
    }

    /// A preset cell is half a swatch on each side (a quarter of its area), so
    /// the number grid stays subordinate to the palette it sits under.
    #[test]
    fn preset_cell_is_a_square_quarter_of_a_swatch() {
        // 24px swatch -> 12px cell: 144 = 576/4.
        let swatch = swatch_cell_width(206.0);
        assert_eq!(swatch, 24.0);
        assert_eq!(preset_cell(swatch), swatch / 2.0);
        assert_eq!(preset_cell(24.0), 12.0);
        // The floor keeps a 2px label legible on a shrunk panel.
        assert_eq!(preset_cell(16.0), 10.0);
        // The cap keeps the cells from turning back into full-size buttons.
        assert_eq!(preset_cell(200.0), 16.0);
    }
}
