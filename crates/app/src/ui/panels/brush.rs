//! The brush section: the JS size controls and the size preset grid.
//!
//! `index.html` splits the section into a `굵기` number row and a
//! `.dt-brush-row` (slider + 1:1 footprint preview on its right); `panel_brush.js`
//! fills the sixteen presets into a grid whose column count derives from the
//! count. Two user UI rounds then moved away from the JS numbers: the grid now
//! mirrors the palette (`#dt-palette`: eight square columns on a 2px gap)
//! instead of the JS four 30px-tall ones, and the row splits into a capped
//! slider plus an enlarged 96px preview instead of the JS flexing slider and
//! 56px box. The fixed 7/5/4 size bands and the bottom-left preview this
//! replaced were the port's own layout, not the spec.
use super::super::UiState;
use super::chrome::{EDGE, section_card, section_header};
use super::color::to_color32;
use crate::session_glue::AppState;
use egui::{Color32, DragValue, RichText, Slider, Ui};

pub const SIZE_PRESETS: [u32; 16] = [1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64];

/// JS `panel_brush.js` L11-12: `PRESET_COLUMNS = 4` (the fallback),
/// `PRESET_MAX_COLUMNS = 8` (the palette's column count).
pub const PRESET_COLUMNS: usize = 4;
pub const PRESET_MAX_COLUMNS: usize = 8;

/// The widest divisor of `count` in `4..=8`, else the 4-column default. The JS
/// `presetColumns` scanned the *narrowest* divisor, which put the sixteen
/// presets on a 4×4 grid; the user asked for the palette's eight columns, so
/// the scan runs the other way and the sixteen land on 8×2. The count still
/// picks the shape — no hard-coded rows.
pub fn preset_columns(count: usize) -> usize {
    if count == 0 {
        return PRESET_COLUMNS;
    }
    let limit = PRESET_MAX_COLUMNS.min(count);
    (PRESET_COLUMNS..=limit)
        .rev()
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

/// The preset grid mirrors the palette (`style_base.css` `#dt-palette`:
/// `repeat(8, 1fr)` with `gap: 2px` and `.dt-swatch { aspect-ratio: 1 }`), so
/// the cells are square and follow the panel width the way the swatches do.
/// The JS `#dt-size-presets` instead uses a 3px gap and 30px-tall cells.
const PRESET_GAP: f32 = 2.0;
/// The preset buttons inherit the panel's body text in the JS (`1em`).
const PRESET_FONT: f32 = 12.0;
/// The 1:1 footprint preview. The JS `#dt-brush-preview` is a 56px box the user
/// found too small, so it grows to 96 while the slider stops filling the row.
const PREVIEW_PX: f32 = 96.0;
/// Narrow-panel floor: the preview gives width back before it stops fitting.
const PREVIEW_MIN_PX: f32 = 48.0;
const PREVIEW_GRID: f32 = 64.0;
/// JS `style_juicy.css` J9 `.dt-brush-row { gap: 10px }`.
const SLIDER_GAP: f32 = 10.0;
/// The capped slider: room to drag a 1..=64 range without running the whole
/// panel width (`brush_row_geometry`); `SLIDER_MIN` is the narrow-panel floor.
const SLIDER_MAX: f32 = 180.0;
const SLIDER_MIN: f32 = 80.0;

/// Row-2 geometry: `(slider width, preview box)`. The preview keeps the row's
/// right edge (the JS row puts it after the flexing slider), the slider is
/// capped, and both shrink together once the panel cannot hold them.
fn brush_row_geometry(available: f32) -> (f32, f32) {
    let preview = PREVIEW_PX.min((available - SLIDER_MIN - SLIDER_GAP).max(PREVIEW_MIN_PX));
    let slider = (available - preview - SLIDER_GAP).clamp(SLIDER_MIN, SLIDER_MAX);
    (slider, preview)
}

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
/// current primary colour on the white 64×64 preview canvas, scaled into the
/// preview box so a 64px blob fills it and a 1px tip stays a single dot. The
/// caller places the box in the row (`brush_row_geometry`).
fn paint_preview(ui: &Ui, rect: egui::Rect, size: u32, color: Color32) {
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

        // Row 2 (JS `.dt-brush-row`, `style_juicy.css` J9): the 1:1 footprint
        // preview keeps the row's right edge, but the slider is capped and no
        // longer runs into it. Both rects are placed by hand so the thin slider
        // sits centered on the tall preview (the JS row's `align-items: center`).
        let available = ui.available_width();
        let (slider_w, preview) = brush_row_geometry(available);
        let (_, row) = ui.allocate_space(egui::vec2(available, preview));
        let slider_h = ui
            .text_style_height(&egui::TextStyle::Body)
            .max(ui.spacing().interact_size.y);
        let slider_rect = egui::Rect::from_min_size(
            egui::pos2(row.left(), row.center().y - slider_h * 0.5),
            egui::vec2(slider_w, slider_h),
        );
        ui.spacing_mut().slider_width = slider_w;
        let slider = ui.place(
            slider_rect,
            Slider::new(&mut v, 1..=64)
                .show_value(false)
                .trailing_fill(true),
        );
        paint_preview(
            ui,
            egui::Rect::from_min_size(
                egui::pos2(row.right() - preview, row.top()),
                egui::Vec2::splat(preview),
            ),
            v,
            primary,
        );
        if slider.changed() || slider.drag_stopped() {
            set_pen_size(app, v);
        }

        ui.add_space(6.0);

        // Presets (JS `#dt-size-presets`): `cols` palette-width square cells,
        // emitted row-major like `renderPresets`.
        let current = app.session.settings().pen_size;
        let cols = preset_columns(SIZE_PRESETS.len());
        let rows = preset_rows(SIZE_PRESETS.len(), cols);
        let cell_w =
            ((ui.available_width() - (cols as f32 - 1.0) * PRESET_GAP) / cols as f32).max(18.0);
        let cell = egui::Vec2::splat(cell_w);
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

    /// The widest divisor of the count in `4..=8` wins, so the sixteen presets
    /// land on the palette's eight columns (the JS rule picked the narrowest
    /// divisor and produced 4×4).
    #[test]
    fn preset_columns_prefer_the_widest_divisor() {
        assert_eq!(preset_columns(16), 8);
        assert_eq!(preset_columns(8), 8);
        assert_eq!(preset_columns(20), 5);
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
        assert_eq!(preset_rows(16, 8), 2);
        assert_eq!(preset_rows(16, 4), 4);
        assert_eq!(preset_rows(5, 4), 2);
        assert_eq!(preset_rows(0, 4), 1);
        // A zero column count falls back to the JS default.
        assert_eq!(preset_rows(16, 0), 4);
    }

    /// The sixteen presets multiply out to the palette's 8×2 exactly: every
    /// cell filled, no ragged trailing row.
    #[test]
    fn the_sixteen_presets_form_the_palette_8x2() {
        let cols = preset_columns(SIZE_PRESETS.len());
        let rows = preset_rows(SIZE_PRESETS.len(), cols);
        assert_eq!((cols, rows), (8, 2));
        assert_eq!(cols * rows, SIZE_PRESETS.len());
    }

    /// Row 2: the slider is capped (it used to fill the whole row) and the
    /// preview grows from the JS 56px to 96px, both giving way on a narrow
    /// panel instead of overflowing.
    #[test]
    fn the_brush_row_caps_the_slider_and_grows_the_preview() {
        // The default panel (~355pt of card content).
        assert_eq!(brush_row_geometry(355.0), (SLIDER_MAX, PREVIEW_PX));
        // Mid width: the slider absorbs the slack up to the cap.
        assert_eq!(brush_row_geometry(260.0), (154.0, PREVIEW_PX));
        // Narrowest panel: both shrink, neither overflows the row.
        let (slider, preview) = brush_row_geometry(156.0);
        assert_eq!((slider, preview), (SLIDER_MIN, 66.0));
        assert!(slider + SLIDER_GAP + preview <= 156.0);
    }
}
