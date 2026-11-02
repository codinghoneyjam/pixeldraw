//! Composed-mode workspace: a spatial slot grid, the sheet preview, and the
//! selected slot's layer stack.
//!
//! The grid shows every slot in cell order. Dragging a tile onto another tile
//! (or directly onto a sheet cell) swaps their positions, so the arrangement is
//! edited where it is seen. Clicking a tile selects it and the right-hand panel
//! edits that slot's composition (`bottom → top`, shown top-first like a layer
//! palette). Layer rows can be dragged as well as moved with ▲▼.

use pixel_engine::atlas::{ATLAS_MAX_CELLS, ATLAS_MAX_LAYERS_PER_CELL, AtlasCellSpec, AtlasLayout};
use pixel_engine::model::Document;

use crate::session_glue::AppState;
use egui::{Color32, Id, RichText, Stroke, StrokeKind, Ui, Vec2};

use super::SELECT_BLUE;
use super::preview::preview;
use super::state::AtlasState;

/// A minimal wrapped tile size for the slot grid.
const TILE_GAP: f32 = 6.0;
const MAX_TILE_COLUMNS: usize = 8;

/// Drag payload for moving a composed slot to another cell.
#[derive(Clone, Copy, Debug)]
pub(super) struct SlotDrag(pub usize);

/// Drag payload for reordering one layer inside a slot. Distinct from
/// [`SlotDrag`] so a layer row can never be dropped onto the slot grid.
#[derive(Clone, Copy, Debug)]
pub(super) struct LayerDrag {
    pub slot: usize,
    pub layer: usize,
}

pub(super) fn composed_workspace(
    ui: &mut Ui,
    atlas: &mut AtlasState,
    app: &AppState,
    cells: &[AtlasCellSpec],
) {
    let Some(doc) = app.session.doc() else {
        return;
    };
    let max_slots = slot_capacity(atlas, doc);

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut atlas.layout.preset_all,
            "첫 슬롯: 모든 레이어 자동 합성",
        );
        ui.label(
            RichText::new("(끄면 사용자 슬롯만 저장)")
                .size(11.0)
                .color(Color32::from_gray(140)),
        );
    });
    ui.label(
        RichText::new(
            "타일을 다른 타일이나 시트 셀에 끌어 놓으면 슬롯 위치가 바뀝니다. 타일을 클릭하면 오른쪽에서 레이어 순서를 조정합니다(행 드래그 또는 ▲▼).",
        )
        .size(11.0)
        .color(Color32::from_gray(145)),
    );

    let total = ui.available_width();
    let gap = 12.0;
    let left_w = (total * 0.24).clamp(200.0, 260.0);
    let right_w = (total * 0.30).clamp(260.0, 360.0);
    let mid_w = (total - left_w - right_w - gap * 2.0).max(240.0);

    let mut action: Option<SlotAction> = None;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(left_w);
            action = slot_grid(ui, atlas, doc, max_slots);
        });
        ui.add_space(gap);
        ui.vertical(|ui| {
            ui.set_width(mid_w);
            if let Some(next) = preview(ui, atlas, app, cells, true) {
                action = Some(next);
            }
        });
        ui.add_space(gap);
        ui.vertical(|ui| {
            ui.set_width(right_w);
            if let Some(next) = slot_detail(ui, atlas, doc, max_slots) {
                action = Some(next);
            }
        });
    });

    let offset = usize::from(atlas.layout.preset_all);
    let focus_after = match &action {
        Some(SlotAction::MoveSlot(si, delta)) => {
            let target = *si as isize + delta;
            (target >= 0).then_some(target as usize + offset)
        }
        // The slot that was dropped on becomes the new selection.
        Some(SlotAction::SwapSlots(_, to)) => Some(to + offset),
        Some(SlotAction::PlaceAt(_, to)) => Some(to + offset),
        Some(SlotAction::DuplicateSlot(si)) => Some(si + 1 + offset),
        Some(SlotAction::DeleteSlot(si)) => Some(si.saturating_sub(1) + offset),
        _ => None,
    };
    if let Some(action) = action {
        action.apply(&mut atlas.layout.slots, max_slots);
        if let Some(focus) = focus_after {
            atlas.focus = Some(focus);
            atlas.selected.clear();
            atlas.selected.insert(focus);
        }
    }

    ui.separator();
    ui.horizontal(|ui| {
        let can_add = atlas.layout.slots.len() < max_slots;
        if ui
            .add_enabled(can_add, egui::Button::new("+ 슬롯 추가"))
            .on_hover_text("현재 선택한 셀의 구성을 복사한 새 슬롯")
            .clicked()
        {
            add_slot(atlas, doc, max_slots);
        }
        if ui.button("레이어에서 다시 만들기").clicked() {
            atlas.layout.slots = doc
                .layers
                .iter()
                .rev()
                .take(max_slots)
                .map(|l| AtlasCellSpec::single(&l.name, &l.id))
                .collect();
            atlas.focus = Some(offset.min(atlas.layout.slots.len()));
        }
        ui.label(
            RichText::new(format!(
                "사용자 슬롯 {}/{}",
                atlas.layout.slots.len(),
                max_slots
            ))
            .size(11.0)
            .color(Color32::from_gray(140)),
        );
    });
}

/// Grid window: show occupied positions and gray cells that can receive a slot.
pub(super) fn grid_window(ui: &mut Ui, atlas: &mut AtlasState, doc: &Document) {
    let max_slots = ATLAS_MAX_CELLS - usize::from(atlas.layout.preset_all);
    if let Some(action) = slot_grid(ui, atlas, doc, max_slots) {
        apply_slot_action(atlas, doc, action, max_slots);
    }
    ui.separator();
    ui.label(
        RichText::new("회색 칸을 누르거나 슬롯을 끌어 놓으면 해당 위치까지 그리드가 확장됩니다.")
            .size(11.0)
            .color(Color32::from_gray(150)),
    );
}

/// Layer window: edit only the selected slot's top-first composition list.
pub(super) fn layer_window(ui: &mut Ui, atlas: &mut AtlasState, doc: &Document) {
    let max_slots = ATLAS_MAX_CELLS - usize::from(atlas.layout.preset_all);
    if let Some(action) = slot_detail(ui, atlas, doc, max_slots) {
        apply_slot_action(atlas, doc, action, max_slots);
    }
}

fn apply_slot_action(atlas: &mut AtlasState, doc: &Document, action: SlotAction, max_slots: usize) {
    let offset = usize::from(atlas.layout.preset_all);
    let focus_after = match &action {
        SlotAction::MoveSlot(si, delta) => {
            let target = *si as isize + delta;
            (target >= 0).then_some(target as usize + offset)
        }
        SlotAction::SwapSlots(_, to) => Some(to + offset),
        SlotAction::PlaceAt(_, to) => Some(to + offset),
        SlotAction::DuplicateSlot(si) => Some(si + 1 + offset),
        SlotAction::DeleteSlot(si) => Some(si.saturating_sub(1) + offset),
        _ => None,
    };
    action.apply(&mut atlas.layout.slots, max_slots);
    if let Some(focus) = focus_after {
        atlas.focus = Some(focus);
        atlas.selected.clear();
        atlas.selected.insert(focus);
    }
    atlas.layout.rows = None;
    atlas.clamp_focus(atlas.layout.cells(doc).len());
}

/// How many user slots fit in the current sheet grid.
pub(super) fn slot_capacity(atlas: &AtlasState, doc: &Document) -> usize {
    let _ = doc;
    ATLAS_MAX_CELLS - usize::from(atlas.layout.preset_all)
}

enum TileClick {
    Focus(usize),
    AddAt(usize),
}

/// N×N slot tiles in cell order. Dragging swaps positions; the sheet preview is
/// temporarily replaced by plain indices while a drag is in flight.
pub(super) fn slot_grid(
    ui: &mut Ui,
    atlas: &mut AtlasState,
    doc: &Document,
    max_slots: usize,
) -> Option<SlotAction> {
    ui.label(RichText::new("슬롯 배치").strong());
    let preset = atlas.layout.preset_all;
    let offset = usize::from(preset);
    // Leave room for the scrollbar so the tiles never spill into the preview.
    let available = (ui.available_width() - 12.0).max(48.0);
    let mut columns = (atlas.layout.columns as usize).clamp(1, MAX_TILE_COLUMNS);
    while columns > 1 && (available - TILE_GAP * (columns as f32 - 1.0)) / (columns as f32) < 24.0 {
        columns -= 1;
    }
    let col_gap = TILE_GAP * (columns.saturating_sub(1)) as f32;
    let tile_w = ((available - col_gap) / columns as f32).clamp(24.0, 92.0);
    let tile_h = (tile_w * 0.78).clamp(30.0, 66.0);

    let mut clicked = None;
    let mut action = None;
    let mut dropped_on: Option<(usize, egui::Rect)> = None;

    egui::ScrollArea::vertical()
        .max_height(340.0)
        .id_salt("atlas_slot_tile_grid_scroll")
        .show(ui, |ui| {
            egui::Grid::new("atlas_slot_tile_grid")
                .num_columns(columns)
                .spacing(Vec2::splat(TILE_GAP))
                .show(ui, |ui| {
                    let slot_count = atlas.layout.slots.len();
                    let mut tiles = 0;
                    if preset {
                        let selected = atlas.focus == Some(0);
                        let response = ui.add_sized(
                            Vec2::new(tile_w, tile_h),
                            egui::Button::new(
                                RichText::new("ALL\n모든 레이어")
                                    .size(if tile_w >= 46.0 { 11.0 } else { 9.0 })
                                    .strong(),
                            )
                            .selected(selected),
                        );
                        if response.clicked() {
                            clicked = Some(TileClick::Focus(0));
                        }
                        response.on_hover_text("자동 합성 슬롯입니다. 위치를 옮길 수 없습니다.");
                        tiles += 1;
                        if tiles % columns == 0 {
                            ui.end_row();
                        }
                    }
                    // Keep one row of gray targets visible beyond the current
                    // occupied range so the destination can grow naturally.
                    let occupied_cells = slot_count + usize::from(preset);
                    let to_finish_row = (columns - occupied_cells % columns) % columns;
                    let display_count = slot_count
                        .saturating_add(to_finish_row)
                        .saturating_add(columns)
                        .min(max_slots);
                    for si in 0..display_count {
                        let index = si + offset;
                        let selected = atlas.focus == Some(index);
                        let is_empty = atlas
                            .layout
                            .slots
                            .get(si)
                            .is_none_or(|slot| slot.layer_ids.is_empty());
                        if is_empty {
                            let response = ui.add_sized(
                                Vec2::new(tile_w, tile_h),
                                egui::Button::new(
                                    RichText::new(format!("{}\n+", index + 1))
                                        .size(11.0)
                                        .color(Color32::from_gray(145)),
                                )
                                .fill(Color32::from_gray(48))
                                .sense(egui::Sense::click_and_drag()),
                            );
                            if response.clicked() {
                                clicked = Some(if si < slot_count {
                                    TileClick::Focus(index)
                                } else {
                                    TileClick::AddAt(si)
                                });
                            }
                            if let Some(from) = response.dnd_hover_payload::<SlotDrag>()
                                && from.0 != si
                            {
                                dropped_on = Some((from.0, response.rect));
                            }
                            if let Some(from) = response.dnd_release_payload::<SlotDrag>()
                                && from.0 != si
                            {
                                action = Some(SlotAction::PlaceAt(from.0, si));
                            }
                            response.on_hover_text(if si < slot_count {
                                format!("빈 슬롯 {} · 다른 슬롯을 끌어 놓을 수 있습니다", index + 1)
                            } else {
                                format!("빈 위치 {} · 눌러 슬롯 추가", index + 1)
                            });
                        } else {
                            let name = atlas.layout.slots[si].name.clone();
                            let text = if tile_w >= 46.0 {
                                format!("{}\n{}", index + 1, short_name(&name, 8))
                            } else {
                                format!("{}", index + 1)
                            };
                            let response = ui
                                .add_sized(
                                    Vec2::new(tile_w, tile_h),
                                    egui::Button::new(RichText::new(&text).size(11.0))
                                        .selected(selected)
                                        .sense(egui::Sense::click_and_drag()),
                                )
                                .on_hover_text(format!("{name}\n끌어서 위치 변경"));
                            response.dnd_set_drag_payload(SlotDrag(si));
                            if response.clicked() || response.drag_started() {
                                clicked = Some(TileClick::Focus(index));
                            }
                            if let Some(from) = response.dnd_hover_payload::<SlotDrag>()
                                && from.0 != si
                            {
                                dropped_on = Some((from.0, response.rect));
                            }
                            if let Some(from) = response.dnd_release_payload::<SlotDrag>()
                                && from.0 != si
                            {
                                action = Some(if si >= slot_count {
                                    SlotAction::PlaceAt(from.0, si)
                                } else {
                                    SlotAction::SwapSlots(from.0, si)
                                });
                            }
                        }
                        tiles += 1;
                        if tiles % columns == 0 {
                            ui.end_row();
                        }
                    }
                    if tiles == 0 {
                        ui.label("슬롯이 없습니다.");
                    }
                });
        });

    if let Some((from, rect)) = dropped_on {
        let painter = ui.painter();
        painter.rect_stroke(rect, 2.0, Stroke::new(2.0, SELECT_BLUE), StrokeKind::Inside);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{}↔", from + 1 + offset),
            egui::FontId::proportional(12.0),
            SELECT_BLUE,
        );
    }

    // A small ghost follows the pointer so the dragged slot stays legible even
    // though the sheet preview is showing indices only.
    if let Some(from) = egui::DragAndDrop::payload::<SlotDrag>(ui.ctx())
        && let Some(pos) = ui.ctx().pointer_interact_pos()
    {
        let name = atlas
            .layout
            .slots
            .get(from.0)
            .map(|slot| short_name(&slot.name, 10))
            .unwrap_or_else(|| "슬롯".to_string());
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            Id::new("atlas_slot_drag_ghost"),
        ));
        let rect = egui::Rect::from_center_size(pos + Vec2::new(0.0, 16.0), Vec2::new(104.0, 24.0));
        painter.rect_filled(rect, 4.0, Color32::from_black_alpha(210));
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{}. {name}", from.0 + 1 + offset),
            egui::FontId::proportional(11.0),
            Color32::from_gray(235),
        );
    }

    match clicked {
        Some(TileClick::Focus(index)) => {
            atlas.focus = Some(index);
            atlas.selected.clear();
            atlas.selected.insert(index);
        }
        Some(TileClick::AddAt(si)) => ensure_slot_at(atlas, doc, si, max_slots),
        None => {}
    }
    action
}

/// The selected slot: name, position controls, and its layer stack top-first.
pub(super) fn slot_detail(
    ui: &mut Ui,
    atlas: &mut AtlasState,
    doc: &Document,
    max_slots: usize,
) -> Option<SlotAction> {
    ui.label(RichText::new("선택한 슬롯").strong());
    let offset = usize::from(atlas.layout.preset_all);
    let Some(focus) = atlas.focus else {
        ui.label(hint_text("타일이나 시트 셀을 선택하세요."));
        return None;
    };

    if focus < offset {
        return preset_detail(ui, atlas, doc, max_slots);
    }
    let si = focus - offset;
    if si >= atlas.layout.slots.len() {
        ui.label(hint_text("타일이나 시트 셀을 선택하세요."));
        return None;
    }

    let mut action = None;
    let layer_count = atlas.layout.slots[si].layer_ids.len();
    let slot_count = atlas.layout.slots.len();

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}.", focus + 1)).strong());
        ui.add(
            egui::TextEdit::singleline(&mut atlas.layout.slots[si].name)
                .desired_width(f32::INFINITY)
                .hint_text("슬롯 이름"),
        );
    });

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("위치")
                .size(11.0)
                .color(Color32::from_gray(145)),
        );
        if ui
            .add_enabled(si > 0, egui::Button::new("◀").small())
            .on_hover_text("앞 슬롯과 순서 교환")
            .clicked()
        {
            action = Some(SlotAction::MoveSlot(si, -1));
        }
        ui.label(format!("{}", focus + 1));
        if ui
            .add_enabled(si + 1 < slot_count, egui::Button::new("▶").small())
            .on_hover_text("뒤 슬롯과 순서 교환")
            .clicked()
        {
            action = Some(SlotAction::MoveSlot(si, 1));
        }
        ui.label(
            RichText::new(format!("레이어 {layer_count}/{ATLAS_MAX_LAYERS_PER_CELL}"))
                .size(11.0)
                .color(Color32::from_gray(145)),
        );
    });
    ui.label(
        RichText::new("합성 순서 · 위에 있을수록 나중에 그려집니다")
            .size(11.0)
            .color(Color32::from_gray(145)),
    );

    egui::ScrollArea::vertical()
        .max_height(214.0)
        .id_salt(("atlas_slot_layer_stack", si))
        .show(ui, |ui| {
            if layer_count == 0 {
                ui.label(hint_text("빈 슬롯 (투명)"));
            }
            // `layer_ids` is bottom → top; show top-first so ▲ means "draw later".
            for li in (0..layer_count).rev() {
                let id = &atlas.layout.slots[si].layer_ids[li];
                let name = doc
                    .find_layer(id)
                    .map(|l| l.name.as_str())
                    .unwrap_or("(없는 레이어)");
                let row = egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let handle = ui
                            .add(
                                egui::Label::new(RichText::new("⠿").color(Color32::from_gray(150)))
                                    .sense(egui::Sense::click_and_drag()),
                            )
                            .on_hover_text("끌어서 이 슬롯 안에서 순서 변경");
                        handle.dnd_set_drag_payload(LayerDrag {
                            slot: si,
                            layer: li,
                        });
                        if ui
                            .add_enabled(li + 1 < layer_count, egui::Button::new("▲").small())
                            .on_hover_text("합성에서 한 단계 위로")
                            .clicked()
                        {
                            action = Some(SlotAction::MoveLayer(si, li, li + 1));
                        }
                        if ui
                            .add_enabled(li > 0, egui::Button::new("▼").small())
                            .on_hover_text("합성에서 한 단계 아래로")
                            .clicked()
                        {
                            action = Some(SlotAction::MoveLayer(si, li, li - 1));
                        }
                        ui.label(
                            RichText::new(format!("{}", li + 1))
                                .size(10.0)
                                .color(Color32::from_gray(130)),
                        );
                        ui.label(RichText::new(short_name(name, 16)).size(12.0))
                            .on_hover_text(name);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("×")
                                .on_hover_text("이 슬롯에서 빼기")
                                .clicked()
                            {
                                action = Some(SlotAction::RemoveLayer(si, li));
                            }
                        });
                    });
                });

                // The whole row accepts a layer drop. Dropping on the upper
                // half inserts above (draws later), the lower half below.
                let rect = row.response.rect;
                if let Some(from) = row.response.dnd_hover_payload::<LayerDrag>()
                    && from.slot == si
                    && from.layer != li
                {
                    let above = drop_is_above(ui, rect);
                    let y = if above { rect.top() } else { rect.bottom() };
                    ui.painter().line_segment(
                        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                        Stroke::new(2.0, SELECT_BLUE),
                    );
                }
                if let Some(from) = row.response.dnd_release_payload::<LayerDrag>()
                    && from.slot == si
                    && from.layer != li
                {
                    let target = li + usize::from(drop_is_above(ui, rect));
                    action = Some(SlotAction::MoveLayerTo(si, from.layer, target));
                }
            }
        });

    if layer_count < ATLAS_MAX_LAYERS_PER_CELL {
        egui::ComboBox::from_id_salt(("atlas_slot_detail_add", si))
            .selected_text("+ 레이어 추가")
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for layer in doc.layers.iter().rev() {
                    if ui.selectable_label(false, &layer.name).clicked() {
                        action = Some(SlotAction::AddLayer(si, layer.id.clone()));
                    }
                }
            });
    }

    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                atlas.layout.slots.len() < max_slots,
                egui::Button::new("슬롯 복제"),
            )
            .on_hover_text("이름과 레이어 구성을 복사해 바로 뒤에 넣습니다")
            .clicked()
        {
            action = Some(SlotAction::DuplicateSlot(si));
        }
        if ui
            .button("슬롯 삭제")
            .on_hover_text("이 슬롯만 삭제합니다")
            .clicked()
        {
            action = Some(SlotAction::DeleteSlot(si));
        }
    });

    action
}

fn preset_detail(
    ui: &mut Ui,
    atlas: &mut AtlasState,
    doc: &Document,
    max_slots: usize,
) -> Option<SlotAction> {
    let all = AtlasLayout::all_layers_spec(doc);
    ui.label(
        RichText::new("모든 레이어 자동 합성")
            .strong()
            .color(Color32::from_gray(200)),
    );
    ui.label(hint_text(
        "이 슬롯은 문서 레이어 순서를 그대로 따릅니다. 순서를 직접 바꾸려면 새 슬롯으로 복사하세요.",
    ));
    egui::ScrollArea::vertical()
        .max_height(190.0)
        .id_salt("atlas_preset_layer_stack")
        .show(ui, |ui| {
            for (i, id) in all.layer_ids.iter().enumerate().rev() {
                let name = doc
                    .find_layer(id)
                    .map(|l| l.name.as_str())
                    .unwrap_or("(없는 레이어)");
                ui.label(
                    RichText::new(format!("{}. {name}", i + 1))
                        .size(12.0)
                        .color(Color32::from_gray(185)),
                );
            }
        });
    if ui
        .add_enabled(
            atlas.layout.slots.len() < max_slots,
            egui::Button::new("새 슬롯으로 복사"),
        )
        .on_hover_text("자동 합성 구성을 편집 가능한 사용자 슬롯으로 복사합니다")
        .clicked()
    {
        let offset = usize::from(atlas.layout.preset_all);
        let name = unique_slot_name(&atlas.layout.slots, "Slot", atlas.layout.slots.len() + 1);
        atlas.layout.slots.push(AtlasCellSpec { name, ..all });
        let focus = atlas.layout.slots.len() - 1 + offset;
        atlas.focus = Some(focus);
        atlas.selected.clear();
        atlas.selected.insert(focus);
    }
    None
}

fn add_slot(atlas: &mut AtlasState, doc: &Document, max_slots: usize) {
    if atlas.layout.slots.len() >= max_slots {
        return;
    }
    let base = atlas
        .focus
        .and_then(|focus| atlas.layout.cells(doc).get(focus).cloned())
        .unwrap_or_else(|| AtlasLayout::all_layers_spec(doc));
    let name = unique_slot_name(&atlas.layout.slots, "Slot", atlas.layout.slots.len() + 1);
    atlas.layout.slots.push(AtlasCellSpec { name, ..base });
    let focus = atlas.layout.slots.len() - 1 + usize::from(atlas.layout.preset_all);
    atlas.focus = Some(focus);
    atlas.selected.clear();
    atlas.selected.insert(focus);
}

fn ensure_slot_at(atlas: &mut AtlasState, doc: &Document, index: usize, max_slots: usize) {
    if index >= max_slots {
        return;
    }
    while atlas.layout.slots.len() <= index {
        let name = unique_slot_name(&atlas.layout.slots, "Slot", atlas.layout.slots.len() + 1);
        atlas.layout.slots.push(AtlasCellSpec {
            name,
            layer_ids: Vec::new(),
            merged: false,
        });
    }
    atlas.layout.rows = None;
    let focus = index + usize::from(atlas.layout.preset_all);
    atlas.focus = Some(focus);
    atlas.selected.clear();
    atlas.selected.insert(focus);
    atlas.clamp_focus(atlas.layout.cells(doc).len());
}

fn unique_slot_name(slots: &[AtlasCellSpec], base: &str, start: usize) -> String {
    let mut n = start.max(1);
    loop {
        let name = format!("{base} {n}");
        if !slots.iter().any(|slot| slot.name == name) {
            return name;
        }
        n += 1;
    }
}

fn hint_text(text: &str) -> RichText {
    RichText::new(text)
        .size(11.0)
        .color(Color32::from_gray(135))
}

/// True when the pointer is on the upper half of a layer row. Used for
/// "insert above/below" drop semantics.
fn drop_is_above(ui: &Ui, rect: egui::Rect) -> bool {
    ui.ctx()
        .pointer_interact_pos()
        .map(|pos| pos.y < rect.center().y)
        .unwrap_or(false)
}

pub(super) fn short_name(name: &str, max_chars: usize) -> String {
    let mut chars = name.chars();
    let mut out = String::new();
    for _ in 0..max_chars {
        let Some(ch) = chars.next() else {
            return out;
        };
        out.push(ch);
    }
    if chars.next().is_some() {
        out.push('…');
    }
    out
}

pub(super) enum SlotAction {
    MoveSlot(usize, isize),
    SwapSlots(usize, usize),
    PlaceAt(usize, usize),
    DuplicateSlot(usize),
    DeleteSlot(usize),
    MoveLayer(usize, usize, usize),
    /// `to` is the final index in `layer_ids` (after removal). `to == len`
    /// means "above the current top layer".
    MoveLayerTo(usize, usize, usize),
    RemoveLayer(usize, usize),
    AddLayer(usize, String),
}

impl SlotAction {
    pub(super) fn apply(self, slots: &mut Vec<AtlasCellSpec>, max_slots: usize) {
        match self {
            Self::MoveSlot(i, delta) => {
                let j = i as isize + delta;
                if j >= 0 && (j as usize) < slots.len() {
                    slots.swap(i, j as usize);
                    while slots.last().is_some_and(|slot| slot.layer_ids.is_empty()) {
                        slots.pop();
                    }
                }
            }
            Self::SwapSlots(a, b) => {
                if a < slots.len() && b < slots.len() && a != b {
                    slots.swap(a, b);
                    while slots.last().is_some_and(|slot| slot.layer_ids.is_empty()) {
                        slots.pop();
                    }
                }
            }
            Self::PlaceAt(from, to) => {
                if from < slots.len() && from != to && to < max_slots {
                    while slots.len() <= to {
                        let n = slots.len() + 1;
                        let name = unique_slot_name(slots, "Empty Slot", n);
                        slots.push(AtlasCellSpec {
                            name,
                            layer_ids: Vec::new(),
                            merged: false,
                        });
                    }
                    slots.swap(from, to);
                    while slots.last().is_some_and(|slot| slot.layer_ids.is_empty()) {
                        slots.pop();
                    }
                }
            }
            Self::DeleteSlot(i) => {
                if i < slots.len() {
                    slots[i].layer_ids.clear();
                    slots[i].merged = false;
                    while slots.last().is_some_and(|slot| slot.layer_ids.is_empty()) {
                        slots.pop();
                    }
                }
            }
            Self::DuplicateSlot(i) => {
                if slots.len() < max_slots
                    && let Some(source) = slots.get(i).cloned()
                {
                    let mut duplicate = source;
                    let desired = format!("{} copy", duplicate.name);
                    duplicate.name = if slots.iter().any(|slot| slot.name == desired) {
                        unique_slot_name(slots, &desired, 2)
                    } else {
                        desired
                    };
                    slots.insert(i + 1, duplicate);
                }
            }
            Self::MoveLayer(si, from, to) => {
                if let Some(slot) = slots.get_mut(si)
                    && from < slot.layer_ids.len()
                    && to < slot.layer_ids.len()
                {
                    slot.layer_ids.swap(from, to);
                    slot.merged = slot.layer_ids.len() > 1;
                }
            }
            Self::MoveLayerTo(si, from, to) => {
                if let Some(slot) = slots.get_mut(si)
                    && from < slot.layer_ids.len()
                {
                    let layer = slot.layer_ids.remove(from);
                    let target = to.min(slot.layer_ids.len());
                    slot.layer_ids.insert(target, layer);
                    slot.merged = slot.layer_ids.len() > 1;
                }
            }
            Self::RemoveLayer(si, li) => {
                if let Some(slot) = slots.get_mut(si)
                    && li < slot.layer_ids.len()
                {
                    slot.layer_ids.remove(li);
                    slot.merged = slot.layer_ids.len() > 1;
                }
            }
            Self::AddLayer(si, id) => {
                if let Some(slot) = slots.get_mut(si)
                    && slot.layer_ids.len() < ATLAS_MAX_LAYERS_PER_CELL
                {
                    slot.layer_ids.push(id);
                    slot.merged = slot.layer_ids.len() > 1;
                }
            }
        }
    }
}
