//! Sheet grid, cell size, margin/padding controls, and the live sheet summary.

use egui::{Color32, RichText, Ui};
use pixel_engine::atlas::{ATLAS_DEFAULT_CELL_MAX, ATLAS_MAX_CELLS, default_cell_size};
use pixel_engine::model::Document;

use super::FOCUS_RED;
use super::state::AtlasState;

pub(super) fn layout_controls(ui: &mut Ui, atlas: &mut AtlasState, doc: &Document) {
    grid_controls(ui, atlas, doc);
    sheet_summary(ui, atlas, doc);
}

fn grid_controls(ui: &mut Ui, atlas: &mut AtlasState, doc: &Document) {
    let doc_w = doc.canvas.width_px;
    let doc_h = doc.canvas.height_px;
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("시트 그리드").strong());
        ui.horizontal(|ui| {
            ui.label("열 수");
            let mut cols = atlas.layout.columns.max(1);
            if ui
                .add(
                    egui::DragValue::new(&mut cols)
                        .range(1..=ATLAS_MAX_CELLS as u32)
                        .speed(1.0)
                        .min_decimals(0),
                )
                .changed()
            {
                atlas.layout.columns = cols.clamp(1, ATLAS_MAX_CELLS as u32);
            }
            ui.weak("열");
            ui.add_space(12.0);
            ui.label("행 수");
            let mut rows = atlas.layout.rows(doc);
            if ui
                .add(
                    egui::DragValue::new(&mut rows)
                        .range(1..=ATLAS_MAX_CELLS as u32)
                        .speed(1.0)
                        .min_decimals(0),
                )
                .changed()
            {
                atlas.layout.rows = Some(rows.clamp(1, ATLAS_MAX_CELLS as u32));
            }
            ui.weak("행");
            ui.weak(if atlas.layout.rows.is_some() {
                "고정"
            } else {
                "자동"
            });
            if atlas.layout.rows.is_some() && ui.small_button("자동").clicked() {
                atlas.layout.rows = None;
            }
            ui.weak(format!(
                "{}칸 · {}셀 사용",
                atlas.layout.columns.saturating_mul(atlas.layout.rows(doc)),
                atlas.layout.cells(doc).len()
            ));
        });
        ui.label(
            RichText::new(
                "자동 행은 셀 수에 맞춰 늘어납니다. 행을 고정하면 남는 칸은 투명하게 저장됩니다.",
            )
            .size(11.0)
            .color(Color32::from_gray(145)),
        );
    });
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("셀 크기").strong());
        ui.horizontal(|ui| {
            ui.label("너비");
            let mut cw = atlas.layout.cell_w;
            let mut ch = atlas.layout.cell_h;
            let w_changed = ui
                .add(
                    egui::DragValue::new(&mut cw)
                        .range(1..=4096)
                        .speed(1.0)
                        .min_decimals(0),
                )
                .changed();
            ui.weak("px");
            ui.label("×");
            ui.label("높이");
            let h_changed = ui
                .add(
                    egui::DragValue::new(&mut ch)
                        .range(1..=4096)
                        .speed(1.0)
                        .min_decimals(0),
                )
                .changed();
            ui.weak("px");
            if w_changed {
                atlas.layout.cell_w = cw.clamp(1, 4096);
            }
            if h_changed {
                atlas.layout.cell_h = ch.clamp(1, 4096);
            }
            if ui
                .button(format!("기본 {ATLAS_DEFAULT_CELL_MAX}"))
                .on_hover_text("문서 캔버스를 512px 상한으로 자른 기본 셀 크기")
                .clicked()
            {
                let (w, h) = default_cell_size(doc_w, doc_h);
                atlas.layout.cell_w = w;
                atlas.layout.cell_h = h;
            }
            if ui
                .button("원본 크기")
                .on_hover_text("문서 캔버스 크기를 그대로 셀로 사용")
                .clicked()
            {
                atlas.layout.cell_w = AtlasState::snap_cell(doc_w);
                atlas.layout.cell_h = AtlasState::snap_cell(doc_h);
            }
        });
        ui.label(
            RichText::new("가로·세로는 1px 단위 정수입니다. 문서 크기와 다르면 최근접 픽셀로 크기를 맞춥니다.")
                .size(11.0)
                .color(Color32::from_gray(145)),
        );
    });
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("내보내기 여백").strong());
        ui.horizontal(|ui| {
            ui.label("바깥 여백");
            ui.add(
                egui::DragValue::new(&mut atlas.layout.margin)
                    .range(0..=4096)
                    .speed(1.0)
                    .min_decimals(0),
            );
            ui.weak("px");
            ui.add_space(10.0);
            ui.label("셀 간격");
            ui.add(
                egui::DragValue::new(&mut atlas.layout.padding)
                    .range(0..=4096)
                    .speed(1.0)
                    .min_decimals(0),
            );
            ui.weak("px");
        });
        ui.label(
            RichText::new("투명 여백과 셀 사이 간격으로 텍스처 가장자리 샘플링을 줄입니다.")
                .size(11.0)
                .color(Color32::from_gray(145)),
        );
    });
}

fn sheet_summary(ui: &mut Ui, atlas: &AtlasState, doc: &Document) {
    let (sheet_w, sheet_h) = atlas.layout.sheet_size(doc);
    let sheet_px = u64::from(sheet_w) * u64::from(sheet_h);
    ui.horizontal(|ui| {
        ui.label(format!("시트 {sheet_w}×{sheet_h} px · 총 {sheet_px}픽셀"));
        match atlas.layout.validate(doc) {
            Ok(()) => {
                ui.colored_label(Color32::from_gray(140), "저장 가능");
            }
            Err(e) => {
                ui.colored_label(FOCUS_RED, &e.message);
            }
        }
    });
    if let Some(err) = &atlas.error {
        ui.colored_label(FOCUS_RED, err);
    }
}
