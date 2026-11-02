//! Metadata-free PNG grid slicing: guess the grid, preview it, then replace
//! the document with one layer per cell.

use pixel_engine::atlas::{ATLAS_MAX_CELLS, AtlasLayout, GridSliceOptions, default_cell_size};
use pixel_engine::io::png::PngImage;

use crate::session_glue::AppState;
use egui::{
    Color32, Id, RichText, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, Vec2,
};

use super::state::{AtlasState, GridImportState};
use super::{FOCUS_RED, SELECT_BLUE, note};

/// Default grid options guessed from an unsliced PNG.
pub(super) fn grid_import_from_image(image: PngImage) -> GridImportState {
    static NEXT_CACHE_KEY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let gcd = |mut a: u32, mut b: u32| {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a.max(1)
    };
    let divisor = gcd(image.width, image.height);
    let (cell_w, cell_h) = if divisor >= 8 {
        (divisor, divisor)
    } else {
        default_cell_size(image.width, image.height)
    };
    let full_columns = (image.width / cell_w.max(1)).max(1);
    let full_rows = (image.height / cell_h.max(1)).max(1);
    let columns = full_columns.min(ATLAS_MAX_CELLS as u32);
    let rows = full_rows.min((ATLAS_MAX_CELLS as u32 / columns).max(1));
    GridImportState {
        image,
        options: GridSliceOptions {
            cell_w,
            cell_h,
            columns,
            rows,
            offset_x: 0,
            offset_y: 0,
            spacing_x: 0,
            spacing_y: 0,
            keep_empty: true,
        },
        cache_key: NEXT_CACHE_KEY.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        confirm_replace: false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GridImportAction {
    None,
    Apply,
    Cancel,
}

pub(super) fn grid_import_editor(
    ui: &mut Ui,
    import: &mut GridImportState,
    app: &AppState,
) -> GridImportAction {
    let mut action = GridImportAction::None;
    ui.separator();
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("PNG 그리드 가져오기").strong());
        ui.label(format!("원본 {}×{} px · 투명한 셀은 유지하거나 건너뜁니다.", import.image.width, import.image.height));
        ui.horizontal(|ui| {
            pixel_value(ui, "셀 너비", &mut import.options.cell_w, 1, import.image.width);
            ui.weak("px");
            pixel_value(ui, "셀 높이", &mut import.options.cell_h, 1, import.image.height);
            ui.weak("px");
        });
        ui.horizontal(|ui| {
            pixel_value(ui, "열", &mut import.options.columns, 1, ATLAS_MAX_CELLS as u32);
            pixel_value(ui, "행", &mut import.options.rows, 1, ATLAS_MAX_CELLS as u32);
            ui.label("시작 위치");
            pixel_value(ui, "X", &mut import.options.offset_x, 0, import.image.width);
            pixel_value(ui, "Y", &mut import.options.offset_y, 0, import.image.height);
        });
        ui.horizontal(|ui| {
            ui.label("셀 간격");
            pixel_value(ui, "가로", &mut import.options.spacing_x, 0, import.image.width);
            ui.weak("px");
            pixel_value(ui, "세로", &mut import.options.spacing_y, 0, import.image.height);
            ui.weak("px");
            ui.checkbox(&mut import.options.keep_empty, "빈 셀 유지");
        });
        let geometry_ok = validate_grid_import_geometry(&import.image, import.options);
        let capacity = u64::from(import.options.columns) * u64::from(import.options.rows);
        if !geometry_ok {
            ui.colored_label(FOCUS_RED, "셀 크기·위치·간격이 PNG 범위를 벗어납니다.");
        } else if capacity > ATLAS_MAX_CELLS as u64 {
            ui.colored_label(FOCUS_RED, format!("최대 {ATLAS_MAX_CELLS}셀까지 가져올 수 있습니다."));
        } else {
            ui.label(format!(
                "{}셀 선택 · 빈 셀 {}",
                capacity,
                if import.options.keep_empty { "유지" } else { "건너뜀" }
            ));
            let right = u64::from(import.options.offset_x)
                + u64::from(import.options.columns - 1)
                    * u64::from(import.options.cell_w + import.options.spacing_x)
                + u64::from(import.options.cell_w);
            let bottom = u64::from(import.options.offset_y)
                + u64::from(import.options.rows - 1)
                    * u64::from(import.options.cell_h + import.options.spacing_y)
                + u64::from(import.options.cell_h);
            if right < u64::from(import.image.width) || bottom < u64::from(import.image.height) {
                ui.weak(format!(
                    "시트의 선택 밖 영역: 오른쪽 {}px · 아래쪽 {}px",
                    u64::from(import.image.width).saturating_sub(right),
                    u64::from(import.image.height).saturating_sub(bottom)
                ));
            }
        }
        grid_import_preview(ui, import, geometry_ok);
        if !import.image.width.is_multiple_of(32) || !import.image.height.is_multiple_of(32) {
            ui.weak("편집기는 캔버스를 32px 단위로 올림합니다. 픽셀은 원래 좌표에 두고 나머지는 투명하게 채웁니다.");
        }
        ui.horizontal(|ui| {
            if import.confirm_replace {
                ui.colored_label(FOCUS_RED, "현재 문서의 저장되지 않은 변경을 버리고 새 문서로 바꿉니다.");
                if ui.button("새 문서로 교체").clicked() {
                    action = GridImportAction::Apply;
                }
                if ui.button("취소").clicked() {
                    import.confirm_replace = false;
                }
            } else if ui
                .add_enabled(geometry_ok && capacity <= ATLAS_MAX_CELLS as u64, egui::Button::new("셀 레이어로 가져오기"))
                .clicked()
            {
                if app.session.history().is_dirty() {
                    import.confirm_replace = true;
                } else {
                    action = GridImportAction::Apply;
                }
            }
            if ui.button("닫기").clicked() {
                action = GridImportAction::Cancel;
            }
        });
    });
    action
}

fn pixel_value(ui: &mut Ui, label: &str, value: &mut u32, min: u32, max: u32) {
    ui.label(label);
    ui.add(
        egui::DragValue::new(value)
            .range(min..=max.max(min))
            .speed(1.0)
            .min_decimals(0),
    );
}

fn validate_grid_import_geometry(image: &PngImage, options: GridSliceOptions) -> bool {
    if options.cell_w == 0 || options.cell_h == 0 || options.columns == 0 || options.rows == 0 {
        return false;
    }
    let stride_x = u64::from(options.cell_w) + u64::from(options.spacing_x);
    let stride_y = u64::from(options.cell_h) + u64::from(options.spacing_y);
    let right = u64::from(options.offset_x)
        + u64::from(options.columns - 1) * stride_x
        + u64::from(options.cell_w);
    let bottom = u64::from(options.offset_y)
        + u64::from(options.rows - 1) * stride_y
        + u64::from(options.cell_h);
    right <= u64::from(image.width) && bottom <= u64::from(image.height)
}

fn grid_import_preview(ui: &mut Ui, import: &GridImportState, geometry_ok: bool) {
    let id = Id::new(("atlas_grid_import_preview", import.cache_key));
    let texture = ui.ctx().data_mut(|data| data.get_temp::<TextureHandle>(id));
    let texture = texture.unwrap_or_else(|| {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [import.image.width as usize, import.image.height as usize],
            &import.image.rgba,
        );
        let texture = ui.ctx().load_texture(
            format!("atlas_grid_import_{}", import.cache_key),
            image,
            TextureOptions::NEAREST,
        );
        ui.ctx()
            .data_mut(|data| data.insert_temp(id, texture.clone()));
        texture
    });
    let max = Vec2::new(ui.available_width().min(680.0), 190.0);
    let factor = (max.x / import.image.width as f32).min(max.y / import.image.height as f32);
    let size = Vec2::new(
        import.image.width as f32 * factor,
        import.image.height as f32 * factor,
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    let stroke = Stroke::new(1.0, if geometry_ok { SELECT_BLUE } else { FOCUS_RED });
    let stride_x = import
        .options
        .cell_w
        .saturating_add(import.options.spacing_x);
    let stride_y = import
        .options
        .cell_h
        .saturating_add(import.options.spacing_y);
    for row in 0..import.options.rows.min(ATLAS_MAX_CELLS as u32) {
        for column in 0..import.options.columns.min(ATLAS_MAX_CELLS as u32) {
            let x = import
                .options
                .offset_x
                .saturating_add(column.saturating_mul(stride_x));
            let y = import
                .options
                .offset_y
                .saturating_add(row.saturating_mul(stride_y));
            let min = rect.min
                + Vec2::new(
                    x as f32 / import.image.width as f32 * rect.width(),
                    y as f32 / import.image.height as f32 * rect.height(),
                );
            let max = rect.min
                + Vec2::new(
                    x.saturating_add(import.options.cell_w) as f32 / import.image.width as f32
                        * rect.width(),
                    y.saturating_add(import.options.cell_h) as f32 / import.image.height as f32
                        * rect.height(),
                );
            ui.painter().rect_stroke(
                egui::Rect::from_min_max(min, max),
                0.0,
                stroke,
                StrokeKind::Inside,
            );
        }
    }
}

pub(super) fn apply_grid_import(
    import: &GridImportState,
    atlas: &mut AtlasState,
    app: &mut AppState,
    initial_layout: &mut AtlasLayout,
) {
    match crate::file_ops::import_png_grid(app, &import.image, import.options) {
        Ok(()) => {
            atlas.rebuild(app);
            *initial_layout = atlas.layout.clone();
            atlas.grid_import = None;
            atlas.error = None;
            note(app, "스프라이트시트를 셀 레이어로 가져왔습니다");
        }
        Err(error) => atlas.error = Some(format!("그리드 가져오기 실패: {}", error.message)),
    }
}
