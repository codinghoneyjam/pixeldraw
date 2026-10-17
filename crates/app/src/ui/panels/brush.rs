//! The brush section: the JS size controls and the size preset grid.
//!
//! `index.html` splits the section into a `굵기` number row and a
//! `.dt-brush-row` (slider + 1:1 footprint preview on its right); `panel_brush.js`
//! fills the sixteen presets into a grid whose column count derives from the
//! count (`presetColumns` → 4) and whose cells stretch to the panel width. The
//! fixed 7/5/4 size bands and the bottom-left preview this replaced were the
//! port's own layout, not the spec.
use super::super::UiState;
use super::chrome::{EDGE, section_card, section_header};
use super::color::to_color32;
use crate::session_glue::AppState;
use egui::{Color32, DragValue, RichText, Slider, Ui};

pub const SIZE_PRESETS: [u32; 16] = [1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64];

/// JS `panel_brush.js` L11-12: `PRESET_COLUMNS = 4`, `PRESET_MAX_COLUMNS = 8`.
pub const PRESET_COLUMNS: usize = 4;
pub const PRESET_MAX_COLUMNS: usize = 8;

/// JS `presetColumns(count)`: the first divisor of `count` in `4..=8`, else the
/// 4-column default. Sixteen presets therefore land on the JS 4×4 grid instead
/// of a hard-coded shape.
pub fn preset_columns(count: usize) -> usize {
    if count == 0 {
        return PRESET_COLUMNS;
    }
    let limit = PRESET_MAX_COLUMNS.min(count);
    (PRESET_COLUMNS..=limit)
        .find(|columns| count.is_multiple_of(*columns))
        .unwrap_or(PRESET_COLUMNS)
}

/// JS `presetRows(count, columns)`: enough rows to hold `count`, at least one,
/// so a trailing partial row is never dropped.
pub fn preset_rows(count: usize, columns: usize) -> usize {
    let columns = if columns > 0 { columns } else { PRESET_COLUMNS };
    if count == 0 {
        1
    } else {
        count.div_ceil(columns).max(1)
    }
}

/// JS `style_juicy.css` J5: `gap: 3px` between cells, every button
/// `min-height: 30px`; the width comes from the `1fr` grid column, so the cells
/// follow the panel the way the palette does.
const PRESET_GAP: f32 = 3.0;
const PRESET_MIN_HEIGHT: f32 = 30.0;
/// The preset buttons inherit the panel's body text in the JS (`1em`).
const PRESET_FONT: f32 = 12.0;
/// JS `#dt-brush-preview { width: 56px; height: 56px }` over the 64×64 canvas.
const PREVIEW_PX: f32 = 56.0;
const PREVIEW_GRID: f32 = 64.0;

/// One size preset: fills its grid cell, hover-pale + press-pop, accent-filled
/// while it is the active size. The number labels the target, so a 1px tip
/// occupies exactly as much space as a 64px blob.
pub(crate) fn paint_size_preset(
    ui: &mut Ui,
    clicks: &mut std::collections::HashMap<String, f64>,
    key: &str,
    size: u32,
    cell: egui::Vec2,
    active: bool,
) -> egui::Response {
    use crate::ui::juice::{click_pop, hover_fade, lerp_color};
    let (rect, resp) = ui.allocate_exact_size(cell, egui::Sense::click());
    let hid = ui.id().with("pr").with(key);
    let f = hover_fade(ui.ctx(), hid, resp.hovered());
    let pop = click_pop(clicks, ui.ctx(), key, resp.clicked());
    let rect = rect.shrink((1.0 - pop) * 2.5);
    // JS `--radius-pct: 10%` of the button box.
    let radius = (rect.height() * 0.1).max(2.0);
    let painter = ui.painter();
    let base = if active {
        Color32::from_rgb(0x00, 0x96, 0xff)
    } else {
        Color32::from_rgb(0x23, 0x24, 0x28)
    };
    painter.rect_filled(rect, radius, lerp_color(base, Color32::WHITE, f * 0.12));
    painter.rect_stroke(
        rect,
        radius,
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
        egui::FontId::proportional(PRESET_FONT),
        if active {
            Color32::WHITE
        } else {
            Color32::from_rgb(0xd0, 0xd4, 0xd9)
        },
    );
    resp.clone().on_hover_text(format!("굵기 {size}px"));
    resp
}

/// JS `paintPreview` (`panel_brush.js` L33-55): the 1:1 footprint mask in the
/// current primary colour on the white 64×64 preview canvas, scaled to the
/// 56px box so a 64px blob fills it and a 1px tip stays a single dot.
fn paint_preview(ui: &mut Ui, size: u32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(PREVIEW_PX), egui::Sense::hover());
    let painter = ui.painter();
    // JS `#dt-brush-preview { background: #fff; border: 1px solid }`.
    painter.rect_filled(rect, 2.0, Color32::WHITE);
    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, EDGE),
        egui::StrokeKind::Inside,
    );
    let Ok(footprint) = pixel_engine::raster::brush_footprint(size as usize) else {
        return;
    };
    let cell = rect.width() / PREVIEW_GRID;
    let origin = ((PREVIEW_GRID - footprint.n as f32) / 2.0).floor();
    for by in 0..footprint.n {
        for bx in 0..footprint.n {
            if !footprint.is_set(bx, by) {
                continue;
            }
            let min =
                rect.min + egui::vec2((origin + bx as f32) * cell, (origin + by as f32) * cell);
            painter.rect_filled(
                egui::Rect::from_min_size(min, egui::Vec2::splat(cell)),
                0.0,
                color,
            );
        }
    }
}

fn set_pen_size(app: &mut AppState, size: u32) {
    let _ = app.session.set_setting(
        "penSize",
        pixel_engine::model::SettingInput::Num(f64::from(size)),
    );
}

/// JS `panel_brush.js`: number row, slider row with the 1:1 preview, presets.
pub fn show_brush(ui: &mut Ui, ui_state: &mut UiState, app: &mut AppState) {
    section_card(ui, |ui| {
        if !section_header(ui, &mut ui_state.panels, "brush", "브러시") {
            return;
        }
        let mut v = app.session.settings().pen_size;
        let primary = to_color32(&app.session.settings().primary_color).unwrap_or(Color32::BLACK);

        // Row 1 (JS `label` + `#dt-pen-size`): the size box sits right of 굵기.
        ui.horizontal(|ui| {
            ui.label(RichText::new("굵기:").size(12.0));
            let num = ui.add(DragValue::new(&mut v).range(1..=64).speed(1.0));
            ui.label("px");
            if num.changed() {
                set_pen_size(app, v);
            }
        });

        ui.add_space(6.0);

        // Row 2 (JS `.dt-brush-row`, `style_juicy.css` J9): the slider flexes
        // and the 1:1 footprint preview stays on its right.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            ui.spacing_mut().slider_width = (ui.available_width() - PREVIEW_PX - 10.0).max(80.0);
            let slider = ui.add(
                Slider::new(&mut v, 1..=64)
                    .show_value(false)
                    .trailing_fill(true),
            );
            paint_preview(ui, v, primary);
            if slider.changed() || slider.drag_stopped() {
                set_pen_size(app, v);
            }
        });

        ui.add_space(6.0);

        // Presets (JS `#dt-size-presets`): `cols` columns of width-filling
        // cells, 30px tall, emitted row-major like `renderPresets`.
        let current = app.session.settings().pen_size;
        let cols = preset_columns(SIZE_PRESETS.len());
        let rows = preset_rows(SIZE_PRESETS.len(), cols);
        let cell_w =
            ((ui.available_width() - (cols as f32 - 1.0) * PRESET_GAP) / cols as f32).max(18.0);
        let cell = egui::vec2(cell_w, PRESET_MIN_HEIGHT);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(PRESET_GAP, PRESET_GAP);
            for row in 0..rows {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = PRESET_GAP;
                    for col in 0..cols {
                        let Some(&size) = SIZE_PRESETS.get(row * cols + col) else {
                            continue;
                        };
                        let resp = paint_size_preset(
                            ui,
                            &mut ui_state.juice_clicks,
                            &format!("preset:{size}"),
                            size,
                            cell,
                            size == current,
                        );
                        if resp.clicked() {
                            set_pen_size(app, size);
                        }
                    }
                });
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_cover_js_range() {
        assert_eq!(SIZE_PRESETS.len(), 16);
        assert_eq!(SIZE_PRESETS[0], 1);
        assert_eq!(SIZE_PRESETS[15], 64);
    }

    /// JS `presetColumns`: the first divisor of the count in `4..=8`, else the
    /// 4-column default. The JS rule is what makes the shape dynamic — the
    /// count picks it, no hard-coded rows.
    #[test]
    fn preset_columns_follow_the_js_divisor_rule() {
        assert_eq!(preset_columns(16), 4);
        assert_eq!(preset_columns(8), 4);
        assert_eq!(preset_columns(20), 4);
        assert_eq!(preset_columns(5), 5);
        assert_eq!(preset_columns(6), 6);
        assert_eq!(preset_columns(7), 7);
        // No divisor inside 4..=8: the default wins, the count is never lost.
        assert_eq!(preset_columns(9), 4);
        assert_eq!(preset_columns(1), 4);
        assert_eq!(preset_columns(0), 4);
    }

    /// JS `presetRows`: ceil(count / cols), never zero, trailing buttons kept.
    #[test]
    fn preset_rows_never_drop_a_trailing_button() {
        assert_eq!(preset_rows(16, 4), 4);
        assert_eq!(preset_rows(16, 8), 2);
        assert_eq!(preset_rows(5, 4), 2);
        assert_eq!(preset_rows(0, 4), 1);
        // A zero column count falls back to the JS default.
        assert_eq!(preset_rows(16, 0), 4);
    }

    /// The sixteen presets multiply out to the JS 4×4 exactly: every cell
    /// filled, no ragged trailing row.
    #[test]
    fn the_sixteen_presets_form_the_js_4x4() {
        let cols = preset_columns(SIZE_PRESETS.len());
        let rows = preset_rows(SIZE_PRESETS.len(), cols);
        assert_eq!((cols, rows), (4, 4));
        assert_eq!(cols * rows, SIZE_PRESETS.len());
    }
}
