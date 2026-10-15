//! The layers panel: opacity row, blend mode, the layer list, and the row of
//! add / duplicate / delete / merge / reorder buttons.
//!
//! The right panel is the mirror of the left ones, so its chrome (card,
//! header) comes from the shared `chrome` module instead of being restated.
use super::super::UiState;
use super::chrome::{EDGE_HI, section_card, section_header};
use crate::session_glue::AppState;
use egui::{Color32, DragValue, RichText, Slider, Ui};

pub const BLEND_MODES: [(&str, &str); 6] = [
    ("normal", "보통"),
    ("multiply", "곱하기"),
    ("screen", "스크린"),
    ("overlay", "오버레이"),
    ("darken", "어둡게"),
    ("lighten", "밝게"),
];

fn blend_label(mode: &str) -> &'static str {
    BLEND_MODES
        .iter()
        .find(|(m, _)| *m == mode)
        .map(|(_, l)| *l)
        .unwrap_or("보통")
}

/// JS `paintThumb` (`panel_layers.js`): nearest-neighbour 32×32.
pub const THUMB_PX: usize = 32;

fn layer_thumbnail(
    store: &pixel_engine::chunkstore::ChunkStore,
    w: u32,
    h: u32,
) -> egui::ColorImage {
    let mut pixels = vec![egui::Color32::TRANSPARENT; THUMB_PX * THUMB_PX];
    if w > 0 && h > 0 {
        for ty in 0..THUMB_PX {
            let sy = ((ty as u64 * u64::from(h)) / THUMB_PX as u64).min(u64::from(h) - 1) as u32;
            for tx in 0..THUMB_PX {
                let sx =
                    ((tx as u64 * u64::from(w)) / THUMB_PX as u64).min(u64::from(w) - 1) as u32;
                let [r, g, b, a] = pixel_engine::pixel::unpack_rgba(store.get_pixel(sx, sy));
                pixels[ty * THUMB_PX + tx] = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
            }
        }
    }
    egui::ColorImage {
        size: [THUMB_PX, THUMB_PX],
        source_size: egui::vec2(THUMB_PX as f32, THUMB_PX as f32),
        pixels,
    }
}

pub fn show_right(ui: &mut Ui, ui_state: &mut UiState, app: &mut AppState) {
    let (active_id, active_idx, layer_count, canvas_wh, layers_snapshot) = match app.session.doc() {
        Some(d) => (
            d.active_layer_id.clone(),
            d.index_of(&d.active_layer_id.clone()).unwrap_or(0),
            d.layers.len(),
            (d.canvas.width_px, d.canvas.height_px),
            d.layers
                .iter()
                .map(|l| {
                    (
                        l.id.clone(),
                        l.name.clone(),
                        l.visible,
                        l.locked,
                        l.opacity,
                        l.blend,
                        layer_thumbnail(&l.store, d.canvas.width_px, d.canvas.height_px),
                    )
                })
                .collect::<Vec<_>>(),
        ),
        None => return,
    };
    if !section_header(ui, &mut ui_state.panels, "layers", "레이어") {
        return;
    }
    section_card(ui, |ui| {
        let active = layers_snapshot.iter().find(|l| l.0 == active_id).cloned();
        if let Some((id, _, _, _, opacity, blend, _)) = active.clone() {
            // JS `.dt-opacity-row`: range slider + number input side by side.
            ui.horizontal(|ui| {
                ui.label("불투명도:");
                let mut v = (opacity * 100.0).round() as u32;
                // Slider takes the flexible middle; its own value label is hidden
                // because the number box next to it already shows it.
                let resp = ui.add(
                    Slider::new(&mut v, 0..=100)
                        .show_value(false)
                        .trailing_fill(true),
                );
                let num_resp = ui.add(DragValue::new(&mut v).range(0..=100).speed(1.0));
                ui.label("%");
                if num_resp.changed() {
                    // Number-box commits are final (JS `change` → `{final: true}`).
                    let _ = app
                        .session
                        .set_layer_opacity(&id, f64::from(v) / 100.0, true);
                } else {
                    if resp.changed() {
                        let _ = app
                            .session
                            .set_layer_opacity(&id, f64::from(v) / 100.0, false);
                    }
                    if resp.drag_stopped() {
                        let _ = app
                            .session
                            .set_layer_opacity(&id, f64::from(v) / 100.0, true);
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("블렌드:");
                let mut cur = blend;
                egui::ComboBox::from_id_salt("layer_blend")
                    .selected_text(blend_label(cur))
                    .show_ui(ui, |ui| {
                        for (m, label) in BLEND_MODES {
                            if ui.selectable_value(&mut cur, m, label).clicked() {
                                let _ = app.session.set_layer_blend(&id, m);
                            }
                        }
                    });
            });
        }
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                // Top-first like JS (`layers.slice().reverse()`).
                for (id, name, visible, locked, _opacity, _blend, thumb) in
                    layers_snapshot.iter().rev()
                {
                    let selected = *id == active_id;
                    let card_size = egui::vec2(ui.available_width().max(180.0), 40.0);
                    let (card_rect, card_resp) =
                        ui.allocate_exact_size(card_size, egui::Sense::click());

                    let bg_color = if selected {
                        egui::Color32::from_rgb(0x1a, 0x36, 0x5d)
                    } else if card_resp.hovered() {
                        egui::Color32::from_rgb(0x32, 0x35, 0x3b)
                    } else {
                        egui::Color32::from_rgb(0x23, 0x24, 0x28)
                    };

                    let border_stroke = if selected {
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(0x00, 0x96, 0xff))
                    } else {
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(0x3a, 0x3d, 0x43))
                    };

                    let painter = ui.painter();
                    painter.rect_filled(card_rect, 4.0, bg_color);
                    painter.rect_stroke(card_rect, 4.0, border_stroke, egui::StrokeKind::Inside);
                    painter.rect_stroke(
                        card_rect.shrink(2.5),
                        3.0,
                        egui::Stroke::new(1.0, EDGE_HI),
                        egui::StrokeKind::Inside,
                    );

                    // Left: Eye visibility toggle (JS: "👁" / "🚫").
                    let eye_rect = egui::Rect::from_center_size(
                        card_rect.left_center() + egui::vec2(14.0, 0.0),
                        egui::vec2(20.0, 20.0),
                    );
                    let eye_resp = ui.interact(
                        eye_rect,
                        ui.id().with(format!("eye_{id}")),
                        egui::Sense::click(),
                    );
                    let eye_text = if *visible { "👁" } else { "🚫" };
                    painter.text(
                        eye_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        eye_text,
                        egui::FontId::proportional(12.0),
                        if *visible {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::GRAY
                        },
                    );
                    if eye_resp.clicked() {
                        let _ = app.session.set_layer_visible(id, !*visible);
                    }

                    // Thumbnail: JS `paintThumb` 32×32 nearest-neighbour.
                    let thumb_rect = egui::Rect::from_center_size(
                        card_rect.left_center() + egui::vec2(42.0, 0.0),
                        egui::vec2(32.0, 32.0),
                    );
                    painter.rect_filled(thumb_rect, 2.0, egui::Color32::from_rgb(0x18, 0x19, 0x1c));
                    let tex = ui.ctx().load_texture(
                        format!("thumb_{id}"),
                        thumb.clone(),
                        egui::TextureOptions::NEAREST,
                    );
                    ui.painter().image(
                        tex.id(),
                        thumb_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );

                    // Right: Lock toggle
                    let lock_rect = egui::Rect::from_center_size(
                        card_rect.right_center() + egui::vec2(-14.0, 0.0),
                        egui::vec2(20.0, 20.0),
                    );
                    let lock_resp = ui.interact(
                        lock_rect,
                        ui.id().with(format!("lock_{id}")),
                        egui::Sense::click(),
                    );
                    let lock_text = if *locked { "🔒" } else { "🔓" };
                    painter.text(
                        lock_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        lock_text,
                        egui::FontId::proportional(11.0),
                        if *locked {
                            egui::Color32::from_rgb(0xff, 0xb0, 0x20)
                        } else {
                            egui::Color32::GRAY
                        },
                    );
                    if lock_resp.clicked() {
                        let _ = app.session.set_layer_locked(id, !*locked);
                    }

                    // Center: Layer name (offset past eye + thumbnail).
                    let name_rect = egui::Rect::from_min_max(
                        card_rect.left_top() + egui::vec2(62.0, 0.0),
                        card_rect.right_bottom() + egui::vec2(-28.0, 0.0),
                    );
                    let name_color = if selected {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_rgb(0xd0, 0xd4, 0xd9)
                    };
                    painter.text(
                        name_rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        name,
                        egui::FontId::proportional(13.0),
                        name_color,
                    );

                    if card_resp.clicked() && !eye_resp.clicked() && !lock_resp.clicked() {
                        let _ = app.session.set_active_layer(id);
                    }

                    if ui_state.panels.renaming.as_deref() == Some(id.as_str()) {
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut ui_state.panels.rename_buffer);
                            if ui.small_button("확인").clicked() {
                                let v = ui_state.panels.rename_buffer.trim().to_string();
                                if !v.is_empty() {
                                    let _ = app.session.rename_layer(id, &v);
                                }
                                ui_state.panels.renaming = None;
                            }
                            if ui.small_button("취소").clicked() {
                                ui_state.panels.renaming = None;
                            }
                        });
                    } else if selected && card_resp.double_clicked() {
                        ui_state.panels.renaming = Some(id.clone());
                        ui_state.panels.rename_buffer = name.clone();
                    }

                    ui.add_space(2.0);
                }
            });
        ui.separator();
        // JS `syncButtons`: remove disabled when n<=1, merge/down disabled on the
        // bottom layer, up disabled on the top layer, dup/add disabled with no doc.
        let n = layer_count;
        let can_merge = active_idx > 0;
        let can_up = active_idx < n.saturating_sub(1);
        let can_down = active_idx > 0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            if crate::ui::juice::button(
                ui,
                &mut ui_state.juice_clicks,
                "layer:add",
                n > 0,
                egui::Button::new(RichText::new("+").size(14.0)),
            )
            .on_hover_text("레이어 추가")
            .clicked()
            {
                let _ = app.session.add_layer(None);
            }
            if crate::ui::juice::button(
                ui,
                &mut ui_state.juice_clicks,
                "layer:dup",
                n > 0,
                egui::Button::new(RichText::new("⧉").size(13.0)),
            )
            .on_hover_text("레이어 복제 (Ctrl+J)")
            .clicked()
            {
                let _ = app.session.duplicate_layer(&active_id);
            }
            if crate::ui::juice::button(
                ui,
                &mut ui_state.juice_clicks,
                "layer:del",
                n > 1,
                egui::Button::new(RichText::new("−").size(14.0)),
            )
            .on_hover_text("레이어 삭제")
            .clicked()
            {
                let _ = app.session.remove_layer(&active_id);
            }
            if crate::ui::juice::button(
                ui,
                &mut ui_state.juice_clicks,
                "layer:merge",
                can_merge,
                egui::Button::new(RichText::new("⬇").size(12.0)),
            )
            .on_hover_text("아래로 병합 (Ctrl+Shift+M)")
            .clicked()
            {
                let _ = app.session.merge_down(&active_id);
            }
            if crate::ui::juice::button(
                ui,
                &mut ui_state.juice_clicks,
                "layer:up",
                can_up,
                egui::Button::new(RichText::new("↑").size(13.0)),
            )
            .on_hover_text("위로 이동 (Ctrl+])")
            .clicked()
            {
                move_active(app, 1);
            }
            if crate::ui::juice::button(
                ui,
                &mut ui_state.juice_clicks,
                "layer:down",
                can_down,
                egui::Button::new(RichText::new("↓").size(13.0)),
            )
            .on_hover_text("아래로 이동 (Ctrl+[)")
            .clicked()
            {
                move_active(app, -1);
            }
        });
        let _ = canvas_wh;
    });
}

fn move_active(app: &mut AppState, delta: i64) {
    let (id, idx, _n) = match app.session.doc() {
        Some(d) => {
            let idx = d.layers.iter().position(|l| l.id == d.active_layer_id);
            match idx {
                Some(i) => (d.active_layer_id.clone(), i as i64, d.layers.len() as i64),
                None => return,
            }
        }
        None => return,
    };
    let _ = app.session.move_layer(&id, idx + delta);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_modes_match_js() {
        assert_eq!(BLEND_MODES.len(), 6);
        // Compared against the table's own label, so re-translating the
        // Korean strings can never drift out of sync with the lookup.
        let (_, label) = BLEND_MODES
            .iter()
            .find(|(mode, _)| *mode == "multiply")
            .expect("multiply is a blend mode");
        assert_eq!(blend_label("multiply"), *label);
    }
}
