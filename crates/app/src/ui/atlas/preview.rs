//! Lazy cell previews and the sheet canvas with focus/selection outlines.
//! In composed mode every existing cell is also a slot drop target.

use std::hash::{Hash, Hasher};

use pixel_engine::atlas::{AtlasCellSpec, AtlasLayout, AtlasMode, build_cell_rgba};
use pixel_engine::model::Document;

use crate::session_glue::AppState;
use egui::{
    Color32, FontId, Id, RichText, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui,
    Vec2,
};

use super::slots::{SlotAction, SlotDrag};
use super::state::AtlasState;
use super::{FOCUS_RED, GRID_GRAY, PREVIEWS_PER_FRAME, SELECT_BLUE};

#[derive(Clone, Default)]
struct PreviewCache {
    salt: u64,
    previews: Vec<Option<TextureHandle>>,
}

fn preview_cache_id() -> Id {
    Id::new("pixeldraw_atlas_cell_previews")
}

fn preview_salt(doc: &Document, layout: &AtlasLayout, cells: &[AtlasCellSpec]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    doc.id.hash(&mut h);
    doc.canvas.width_px.hash(&mut h);
    doc.canvas.height_px.hash(&mut h);
    layout.mode.hash(&mut h);
    layout.columns.hash(&mut h);
    layout.rows.hash(&mut h);
    layout.cell_w.hash(&mut h);
    layout.cell_h.hash(&mut h);
    layout.margin.hash(&mut h);
    layout.padding.hash(&mut h);
    layout.preset_all.hash(&mut h);
    for cell in cells {
        cell.name.hash(&mut h);
        cell.merged.hash(&mut h);
        for id in &cell.layer_ids {
            id.hash(&mut h);
            if let Some(layer) = doc.find_layer(id) {
                layer.visible.hash(&mut h);
                layer.opacity.to_bits().hash(&mut h);
                layer.blend.hash(&mut h);
            }
        }
    }
    h.finish()
}

fn ensure_previews(
    ctx: &egui::Context,
    doc: &Document,
    layout: &AtlasLayout,
    cells: &[AtlasCellSpec],
    salt: u64,
) -> Vec<Option<TextureHandle>> {
    let mut handles = ctx.data_mut(|d| {
        let cache = d.get_temp_mut_or_default::<PreviewCache>(preview_cache_id());
        if cache.salt != salt || cache.previews.len() != cells.len() {
            cache.salt = salt;
            cache.previews.clear();
            cache.previews.resize(cells.len(), None);
        }
        cache.previews.clone()
    });
    let mut built = 0usize;
    for (i, cell) in cells.iter().enumerate() {
        if handles[i].is_some() || built >= PREVIEWS_PER_FRAME {
            continue;
        }
        match build_cell_rgba(doc, cell, layout.cell_w, layout.cell_h) {
            Ok(rgba) => {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [rgba.width as usize, rgba.height as usize],
                    &rgba.rgba,
                );
                let tex = ctx.load_texture(
                    format!("atlas_cell_{salt}_{i}"),
                    image,
                    TextureOptions::NEAREST,
                );
                handles[i] = Some(tex.clone());
                ctx.data_mut(|d| {
                    let cache = d.get_temp_mut_or_default::<PreviewCache>(preview_cache_id());
                    if cache.salt == salt && i < cache.previews.len() && cache.previews[i].is_none()
                    {
                        cache.previews[i] = Some(tex);
                    }
                });
                built += 1;
            }
            Err(e) => {
                // Draw a placeholder and let the status line explain it.
                let _ = e;
                break;
            }
        }
    }
    if built > 0 && handles.iter().any(Option::is_none) {
        ctx.request_repaint();
    }
    handles
}

pub(super) fn preview(
    ui: &mut Ui,
    atlas: &mut AtlasState,
    app: &AppState,
    cells: &[AtlasCellSpec],
    interactive: bool,
) -> Option<SlotAction> {
    let doc = app.session.doc()?;
    let (sheet_w, sheet_h) = atlas.layout.sheet_size(doc);
    if cells.is_empty() || sheet_w == 0 || sheet_h == 0 {
        ui.label("미리보기할 셀이 없습니다.");
        return None;
    }
    if atlas.layout.validate(doc).is_err() {
        ui.label("그리드 용량과 셀 수를 맞추면 미리보기가 표시됩니다.");
        return None;
    }
    // A slot drag is a lightweight rearrangement operation: skip texture
    // uploads and paint plain cell indices until the pointer is released, so
    // the sheet never competes with the drag for frame time.
    let dragging = interactive && egui::DragAndDrop::has_payload_of_type::<SlotDrag>(ui.ctx());
    let salt = preview_salt(doc, &atlas.layout, cells);
    let handles = if dragging {
        Vec::new()
    } else {
        ensure_previews(ui.ctx(), doc, &atlas.layout, cells, salt)
    };
    let cols = atlas.layout.columns.max(1);
    let rows = atlas.layout.rows(doc);

    let avail_w = ui.available_width().clamp(240.0, 760.0);
    let max_h = 300.0;
    let margin = 6.0;
    let scale = ((avail_w - margin * 2.0) / sheet_w as f32)
        .min((max_h - margin * 2.0) / sheet_h as f32)
        .max(0.001);
    let size = Vec2::new(sheet_w as f32 * scale, sheet_h as f32 * scale);
    let mut preview_widget = None;
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - size.x - margin * 2.0).max(0.0) * 0.5);
        preview_widget = Some(ui.allocate_exact_size(
            size + Vec2::splat(margin * 2.0),
            if interactive {
                Sense::click()
            } else {
                Sense::hover()
            },
        ));
    });
    let (rect, response) = preview_widget.expect("preview widget allocated");
    let sheet_rect = egui::Rect::from_min_size(rect.min + Vec2::splat(margin), size);
    let cell_size = Vec2::new(
        atlas.layout.cell_w as f32 * scale,
        atlas.layout.cell_h as f32 * scale,
    );
    // Cloned so the per-cell drop targets can call `ui.interact` while the
    // painter is still in use.
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 0.0, Color32::from_gray(22));
    if dragging {
        painter.text(
            sheet_rect.left_top() + Vec2::new(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            "드래그 중 · 미리보기 숨김",
            FontId::proportional(10.0),
            Color32::from_gray(170),
        );
    }

    let capacity = (cols * rows) as usize;
    let mut bodies = Vec::with_capacity(cells.len());
    let mut drop_target = None;
    let mut drop_action: Option<SlotAction> = None;
    for i in 0..capacity {
        let cx = i as u32 % cols;
        let cy = i as u32 / cols;
        let cell_rect = egui::Rect::from_min_size(
            sheet_rect.min
                + Vec2::new(
                    (atlas.layout.margin + cx * (atlas.layout.cell_w + atlas.layout.padding))
                        as f32
                        * scale,
                    (atlas.layout.margin + cy * (atlas.layout.cell_h + atlas.layout.padding))
                        as f32
                        * scale,
                ),
            cell_size,
        );
        let body = cell_rect.shrink(1.0);
        if body.width() > 2.0 && body.height() > 2.0 {
            paint_checker(&painter, body, 12.0);
            let Some(cell) = cells.get(i) else {
                painter.text(
                    body.center(),
                    egui::Align2::CENTER_CENTER,
                    if atlas.layout.mode == AtlasMode::Composed {
                        if interactive {
                            "빈 슬롯 · 클릭해 추가"
                        } else {
                            "빈 슬롯"
                        }
                    } else {
                        "빈 칸"
                    },
                    FontId::proportional(if body.width() > 100.0 { 13.0 } else { 9.0 }),
                    Color32::from_gray(130),
                );
                painter.rect_stroke(body, 0.0, Stroke::new(1.0, GRID_GRAY), StrokeKind::Inside);
                continue;
            };
            bodies.push((i, body));

            // Slot tiles can also be dropped straight onto the sheet. Only
            // existing user-slot cells are targets; empty cells keep their
            // click-to-create behaviour.
            if interactive && atlas.layout.mode == AtlasMode::Composed {
                let offset = usize::from(atlas.layout.preset_all);
                let target_slot = i.saturating_sub(offset);
                let user_slots = cells.len().saturating_sub(offset);
                let drop_response = ui.interact(
                    body,
                    Id::new(("atlas_preview_cell_drop", i)),
                    Sense::hover(),
                );
                if i >= offset
                    && let Some(from) = drop_response.dnd_hover_payload::<SlotDrag>()
                    && from.0 != target_slot
                    && from.0 + offset < cells.len()
                {
                    drop_target = Some(i);
                }
                if let Some(from) = drop_response.dnd_release_payload::<SlotDrag>()
                    && i >= offset
                    && from.0 != target_slot
                    && target_slot < user_slots
                {
                    drop_action = Some(SlotAction::SwapSlots(from.0, target_slot));
                }
            }

            if let Some(tex) = handles.get(i).and_then(Option::as_ref) {
                painter.image(
                    tex.id(),
                    body,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                painter.text(
                    body.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    FontId::proportional(if body.width() > 40.0 { 15.0 } else { 10.0 }),
                    Color32::from_gray(150),
                );
            }
            painter.rect_stroke(body, 0.0, Stroke::new(1.0, GRID_GRAY), StrokeKind::Inside);
            if cell.merged {
                let badge = egui::Rect::from_min_size(
                    body.min + Vec2::new(2.0, 2.0),
                    Vec2::new(16.0, 13.0),
                );
                painter.rect_filled(badge, 2.0, Color32::from_black_alpha(180));
                painter.text(
                    badge.center(),
                    egui::Align2::CENTER_CENTER,
                    "M",
                    FontId::monospace(9.0),
                    Color32::from_rgb(255, 210, 205),
                );
            }
        }
    }

    // Selected cells: blue inside stroke, never on top of the image content
    // beyond a 1px rim.
    for (i, body) in &bodies {
        if atlas.selected.contains(i) && atlas.focus != Some(*i) {
            painter.rect_stroke(
                *body,
                0.0,
                Stroke::new(2.0, SELECT_BLUE),
                StrokeKind::Inside,
            );
        }
    }

    // Focus: red stroke entirely outside the cell. Clipping to the sheet (plus
    // the margin) keeps it off neighbours' pixels and inside the widget.
    if let Some(focus) = atlas.focus
        && let Some(body) = bodies
            .iter()
            .find_map(|(i, body)| (*i == focus).then_some(body))
    {
        let clip = sheet_rect.expand(margin);
        let focus_painter = painter.with_clip_rect(clip);
        focus_painter.rect_stroke(
            body.expand(2.0),
            0.0,
            Stroke::new(2.0, FOCUS_RED),
            StrokeKind::Outside,
        );
    }

    // Drop target: highlight the cell that would receive the dragged slot.
    if let Some(target) = drop_target
        && let Some((_, body)) = bodies.iter().find(|(index, _)| *index == target)
    {
        painter.rect_stroke(
            *body,
            0.0,
            Stroke::new(3.0, SELECT_BLUE),
            StrokeKind::Inside,
        );
        painter.text(
            body.center(),
            egui::Align2::CENTER_CENTER,
            "여기에 놓기",
            FontId::proportional(if body.width() > 90.0 { 12.0 } else { 9.0 }),
            SELECT_BLUE,
        );
    }

    // Grid outline + click target.
    painter.rect_stroke(
        sheet_rect,
        0.0,
        Stroke::new(1.0, Color32::from_gray(120)),
        StrokeKind::Outside,
    );
    if interactive
        && response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
        && sheet_rect.contains(pos)
    {
        let local = pos - sheet_rect.min;
        let margin = atlas.layout.margin as f32 * scale;
        let stride_x = (atlas.layout.cell_w + atlas.layout.padding) as f32 * scale;
        let stride_y = (atlas.layout.cell_h + atlas.layout.padding) as f32 * scale;
        let inner_x = local.x - margin;
        let inner_y = local.y - margin;
        if inner_x >= 0.0 && inner_y >= 0.0 && stride_x > 0.0 && stride_y > 0.0 {
            let cx = (inner_x / stride_x).floor() as u32;
            let cy = (inner_y / stride_y).floor() as u32;
            let in_cell_x = inner_x - cx as f32 * stride_x < cell_size.x;
            let in_cell_y = inner_y - cy as f32 * stride_y < cell_size.y;
            if cx >= cols || cy >= rows || !in_cell_x || !in_cell_y {
                return drop_action;
            }
            let i = (cy * cols + cx) as usize;
            if i < cells.len() {
                atlas.focus = Some(i);
                let multi = ui.input(|input| input.modifiers.shift || input.modifiers.command);
                if multi {
                    if !atlas.selected.remove(&i) {
                        atlas.selected.insert(i);
                    }
                } else {
                    atlas.selected.clear();
                    atlas.selected.insert(i);
                }
                ui.ctx().request_repaint();
            } else if i >= cells.len() && atlas.layout.mode == AtlasMode::Composed {
                while atlas.layout.slots.len() + usize::from(atlas.layout.preset_all) <= i {
                    atlas.layout.slots.push(AtlasCellSpec {
                        name: format!("Slot {}", atlas.layout.slots.len() + 1),
                        layer_ids: Vec::new(),
                        merged: false,
                    });
                }
                atlas.focus = Some(i);
                atlas.selected.clear();
                atlas.selected.insert(i);
                ui.ctx().request_repaint();
            }
        }
    }

    if let Some(focus) = atlas.focus
        && let Some(cell) = cells.get(focus)
    {
        let names = cell
            .layer_ids
            .iter()
            .filter_map(|id| doc.find_layer(id).map(|l| l.name.as_str()))
            .collect::<Vec<_>>()
            .join(" → ");
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(format!("포커스: {}. {}", focus + 1, cell.name))
                    .size(12.0)
                    .color(FOCUS_RED),
            );
            if !names.is_empty() {
                ui.label(
                    RichText::new(names)
                        .size(11.0)
                        .color(Color32::from_gray(150)),
                );
            }
        });
    }

    drop_action
}

fn paint_checker(painter: &egui::Painter, rect: egui::Rect, square: f32) {
    let a = Color32::from_gray(42);
    let b = Color32::from_gray(30);
    let nx = (rect.width() / square).ceil() as i32;
    let ny = (rect.height() / square).ceil() as i32;
    for y in 0..ny {
        for x in 0..nx {
            let min = rect.min + Vec2::new(x as f32 * square, y as f32 * square);
            let cell = egui::Rect::from_min_size(min, Vec2::splat(square)).intersect(rect);
            if cell.width() <= 0.0 || cell.height() <= 0.0 {
                continue;
            }
            painter.rect_filled(cell, 0.0, if (x + y) % 2 == 0 { a } else { b });
        }
    }
}
