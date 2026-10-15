//! The colour section: fg/bg slots, the HSV wheel, channel and hex fields.
//!
//! Facade over `color/**` so the panel stays about panels:
//!   color/hsv  ??the JS-pinned HSV maths
//!   color/hex  ??hex parsing/formatting
mod hex;
mod hsv;

pub(crate) use hex::{to_color32, to_hex_full};
pub(crate) use hsv::{angle_to_hue, hsv_to_rgb, on_ring, rgb_to_hsv, with_slot_alpha};

use super::super::UiState;
use super::chrome::{PanelsState, section_card, section_header};
use crate::session_glue::AppState;
use egui::{Color32, DragValue, RichText, Ui};

/// JS `panel_color_wheel.js` geometry, verbatim.
pub const WHEEL_PX: f32 = 132.0;
pub const RING_OUT: f32 = 66.0;
pub const RING_IN: f32 = 55.5;
pub const RING_MID: f32 = 60.75;
/// `floor(2 * (RING_IN - 3) / sqrt(2))` = 74, the largest square that fits
/// in the ring hole with 3px clearance on every side.
pub const SV_PX: f32 = 74.0;

/// Live wheel result: the picked colour plus whether the gesture ended
/// (JS `pick` fires continuously, `commit` fires exactly once - recent
/// colours are recorded only from the commit).
pub struct WheelPick {
    pub hex: String,
    pub committed: bool,
}

fn show_hsv_wheel(ui: &mut Ui, current_hex: &str, current_color: Color32) -> Option<WheelPick> {
    let size = WHEEL_PX;
    let (rect, response) =
        ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::click_and_drag());
    let center = rect.center();
    let r_out = RING_OUT;
    let r_in = RING_IN;
    let sv_size = SV_PX;
    let sv_rect = egui::Rect::from_center_size(center, egui::Vec2::splat(sv_size));

    let (mut h, mut s, mut v) = rgb_to_hsv(current_color.r(), current_color.g(), current_color.b());
    // JS `setHueSV`: hue only moves for saturated, bright, finite picks.
    let mut picked: Option<(f32, f32, f32)> = None;

    if (response.dragged() || response.clicked())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let dx = pos.x - center.x;
        let dy = pos.y - center.y;
        if on_ring(dx, dy) {
            // Hue ring drag (JS `pickHue`: off-ring offsets are ignored,
            // `onRing` uses RING_IN - 6).
            h = angle_to_hue(dx, dy);
            picked = Some((h, s, v));
        } else {
            // SV square drag (JS `pointerToSv`: clamped, corners reachable).
            let rel_x = ((pos.x - sv_rect.left()) / sv_size).clamp(0.0, 1.0);
            let rel_y = ((pos.y - sv_rect.top()) / sv_size).clamp(0.0, 1.0);
            s = rel_x;
            v = 1.0 - rel_y;
            picked = Some((h, s, v));
        }
    }

    // Build the entire wheel as a ColorImage (like JS paintPixels).
    // 1. Hue ring pixels (cached base in JS; rebuilt here under one texture id).
    let n = size as usize;
    let mut pixels = vec![egui::Color32::TRANSPARENT; n * n];
    for py in 0..n {
        for px in 0..n {
            let dx = px as f32 - size / 2.0 + 0.5;
            let dy = py as f32 - size / 2.0 + 0.5;
            let r = (dx * dx + dy * dy).sqrt();
            if r > r_out || r < r_in {
                continue;
            }
            // JS `hsvToHex` for the ring pixels.
            let hue_deg = angle_to_hue(dx, dy);
            let (cr, cg, cb) = hsv_to_rgb(hue_deg, 1.0, 1.0);
            pixels[py * n + px] = Color32::from_rgb(cr, cg, cb);
        }
    }

    // 2. SV square pixels
    let grid_n = 24usize;
    let sv_left = sv_rect.left() - rect.left();
    let sv_top = sv_rect.top() - rect.top();
    for gy in 0..grid_n {
        for gx in 0..grid_n {
            let s_mid = (gx as f32 + 0.5) / grid_n as f32;
            let v_mid = 1.0 - (gy as f32 + 0.5) / grid_n as f32;
            let (r, g, b) = hsv_to_rgb(h, s_mid, v_mid);
            let col = Color32::from_rgb(r, g, b);

            let x0 = ((sv_left + (gx as f32 / grid_n as f32) * sv_size) as usize).min(n - 1);
            let x1 = ((sv_left + ((gx + 1) as f32 / grid_n as f32) * sv_size) as usize).min(n - 1);
            let y0 = ((sv_top + (gy as f32 / grid_n as f32) * sv_size) as usize).min(n - 1);
            let y1 = ((sv_top + ((gy + 1) as f32 / grid_n as f32) * sv_size) as usize).min(n - 1);

            for py in y0..=y1 {
                for px in x0..=x1 {
                    if py < n && px < n {
                        pixels[py * n + px] = col;
                    }
                }
            }
        }
    }

    let image = egui::ColorImage {
        size: [n, n],
        source_size: egui::vec2(n as f32, n as f32),
        pixels,
    };
    let texture = ui
        .ctx()
        .load_texture("hsv_wheel", image, egui::TextureOptions::default());
    ui.painter().image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );

    // 3. Hue Marker
    let painter = ui.painter();
    let hue_rad = (h - 90.0).to_radians();
    let marker_pos =
        center + egui::Vec2::new(hue_rad.cos(), hue_rad.sin()) * ((r_in + r_out) * 0.5);
    painter.circle_stroke(marker_pos, 4.0, egui::Stroke::new(2.0, Color32::WHITE));
    painter.circle_stroke(marker_pos, 5.0, egui::Stroke::new(1.0, Color32::BLACK));

    // 4. SV Marker
    let sv_marker = egui::pos2(
        sv_rect.left() + s * sv_size,
        sv_rect.top() + (1.0 - v) * sv_size,
    );
    let marker_contrast = if v > 0.5 && s < 0.5 {
        Color32::BLACK
    } else {
        Color32::WHITE
    };
    painter.circle_stroke(sv_marker, 4.0, egui::Stroke::new(2.0, marker_contrast));

    // JS pick/commit: live colour on every drag (caller applies immediately),
    // `committed` only on release (caller records recent exactly once).
    match picked {
        Some((ph, ps, pv)) => {
            let (r, g, b) = hsv_to_rgb(ph, ps, pv);
            let hex = format!("#{r:02x}{g:02x}{b:02x}");
            let hex = with_slot_alpha(current_hex, hex);
            Some(WheelPick {
                hex,
                committed: response.drag_stopped() || response.clicked(),
            })
        }
        None => None,
    }
}

pub(crate) fn push_recent(recent: &mut Vec<String>, hex: String) {
    recent.retain(|c| c != &hex);
    recent.insert(0, hex);
    recent.truncate(8);
}

pub(crate) fn apply_color(
    app: &mut AppState,
    _panels: &mut PanelsState,
    secondary: bool,
    hex: &str,
) {
    let key = if secondary {
        "secondaryColor"
    } else {
        "primaryColor"
    };
    let _ = app
        .session
        .set_setting(key, pixel_engine::model::SettingInput::Str(hex));
}

/// JS `panel_color.js`: slots + wheel + RGBA/hex fields in one card.
pub fn show_color(ui: &mut Ui, ui_state: &mut UiState, app: &mut AppState) {
    // ?? Color section ??????????????????????????????????????????????????
    section_card(ui, |ui| {
        if !section_header(ui, &mut ui_state.panels, "color", "?됱긽") {
            return;
        }
        let s = app.session.settings();
        let primary = to_color32(&s.primary_color).unwrap_or(Color32::BLACK);
        let secondary = to_color32(&s.secondary_color).unwrap_or(Color32::WHITE);
        let is_secondary = ui_state.panels.secondary_active;
        let active_color = if is_secondary { secondary } else { primary };

        // JS `.dt-picker-head`: slots on the left, the 132px wheel on the
        // right (`display: flex`, single row).
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                let (slot_rect, slot_resp) =
                    ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::click());
                let painter = ui.painter();

                let bg_box = egui::Rect::from_min_size(
                    slot_rect.min + egui::vec2(16.0, 16.0),
                    egui::vec2(28.0, 28.0),
                );
                painter.rect_filled(bg_box, 3.0, secondary);
                painter.rect_stroke(
                    bg_box,
                    3.0,
                    egui::Stroke::new(
                        if is_secondary { 2.0 } else { 1.0 },
                        if is_secondary {
                            Color32::from_rgb(0x00, 0x96, 0xff)
                        } else {
                            Color32::from_rgb(0x45, 0x4a, 0x52)
                        },
                    ),
                    egui::StrokeKind::Inside,
                );

                let fg_box = egui::Rect::from_min_size(slot_rect.min, egui::vec2(28.0, 28.0));
                painter.rect_filled(fg_box, 3.0, primary);
                painter.rect_stroke(
                    fg_box,
                    3.0,
                    egui::Stroke::new(
                        if !is_secondary { 2.0 } else { 1.0 },
                        if !is_secondary {
                            Color32::from_rgb(0x00, 0x96, 0xff)
                        } else {
                            Color32::from_rgb(0x45, 0x4a, 0x52)
                        },
                    ),
                    egui::StrokeKind::Inside,
                );

                if slot_resp.clicked()
                    && let Some(pos) = slot_resp.interact_pointer_pos()
                {
                    if fg_box.contains(pos) {
                        ui_state.panels.secondary_active = false;
                    } else if bg_box.contains(pos) {
                        ui_state.panels.secondary_active = true;
                    } else {
                        ui_state.panels.secondary_active = !ui_state.panels.secondary_active;
                    }
                }

                ui.horizontal(|ui| {
                    if ui.button("??).on_hover_text("?꾧꼍쨌諛곌꼍 援먯껜 (X)").clicked() {
                        let s = app.session.settings();
                        let a = s.primary_color.clone();
                        let b = s.secondary_color.clone();
                        let _ = app.session.set_setting(
                            "primaryColor",
                            pixel_engine::model::SettingInput::Str(&b),
                        );
                        let _ = app.session.set_setting(
                            "secondaryColor",
                            pixel_engine::model::SettingInput::Str(&a),
                        );
                    }
                    if ui.button("??).on_hover_text("湲곕낯??珥덇린??(D)").clicked() {
                        let _ = app.session.set_setting(
                            "primaryColor",
                            pixel_engine::model::SettingInput::Str("#000000"),
                        );
                        let _ = app.session.set_setting(
                            "secondaryColor",
                            pixel_engine::model::SettingInput::Str("#ffffff"),
                        );
                    }
                });
            });

            ui.add_space(4.0);
            // JS `mountWheel`: onChange applies live (no recent), onCommit
            // records recent exactly once.
            let settings = app.session.settings();
            let active_hex = if is_secondary {
                settings.secondary_color.clone()
            } else {
                settings.primary_color.clone()
            };
            if let Some(pick) = show_hsv_wheel(ui, &active_hex, active_color) {
                apply_color(app, &mut ui_state.panels, is_secondary, &pick.hex);
                if pick.committed {
                    push_recent(&mut ui_state.panels.recent, pick.hex);
                }
            }
        });

        ui.add_space(6.0);
        // JS `panel_color_fields.js`: R/G/B (0..255) + A (1..255) channel
        // inputs plus HEX. Channel edits preserve the other channels and the
        {
            let settings = app.session.settings();
            let active_hex = if is_secondary {
                settings.secondary_color.clone()
            } else {
                settings.primary_color.clone()
            };
            let rgba = hex::parse_hex(&active_hex).unwrap_or([0, 0, 0, 255]);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
                for (label, ch) in [("R", 0), ("G", 1), ("B", 2), ("A", 3)] {
                    ui.label(RichText::new(label).size(11.0));
                    let mut v = u32::from(rgba[ch]);
                    let range = if ch == 3 { 1..=255 } else { 0..=255 };
                    if ui
                        .add(
                            DragValue::new(&mut v)
                                .range(range)
                                .speed(1.0)
                                .max_decimals(0),
                        )
                        .changed()
                    {
                        let mut next = rgba;
                        next[ch] = v as u8;
                        apply_color(app, &mut ui_state.panels, is_secondary, &to_hex_full(next));
                    }
                }
            });
        }

        ui.horizontal(|ui| {
            ui.label(RichText::new("HEX").size(11.0));
            // Canonical form keeps alpha when translucent (JS `normHex`).
            let settings = app.session.settings();
            let canonical = if is_secondary {
                settings.secondary_color.clone()
            } else {
                settings.primary_color.clone()
            };
            let mut hex_val = canonical;
            if ui.text_edit_singleline(&mut hex_val).changed()
                && let Some(rgba) = hex::parse_hex(&hex_val)
            {
                apply_color(app, &mut ui_state.panels, is_secondary, &to_hex_full(rgba));
            }
        });
    });
}

#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_geometry_matches_js() {
        assert_eq!(WHEEL_PX, 132.0);
        assert_eq!(RING_OUT, 66.0);
        assert_eq!(RING_IN, 55.5);
        assert_eq!(RING_MID, 60.75);
        // floor(2 * (55.5 - 3) / sqrt(2)) = 74
        assert_eq!(SV_PX, 74.0);
    }

    #[test]
    fn hsv_round_trips_like_js() {
        // Pure red / green / blue hit exact sector boundaries.
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), (255, 0, 0));
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), (0, 255, 0));
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), (0, 0, 255));
        let (h, s, v) = rgb_to_hsv(255, 0, 0);
        assert!((h - 0.0).abs() < 1e-4 && (s - 1.0).abs() < 1e-5 && (v - 1.0).abs() < 1e-5);
        // 360 wraps to sector 0, not 6.
        assert_eq!(hsv_to_rgb(360.0, 1.0, 1.0), (255, 0, 0));
    }

    #[test]
    fn angle_and_ring_match_js() {
        assert!((angle_to_hue(1.0, 0.0) - 90.0).abs() < 1e-4);
        assert!((angle_to_hue(0.0, -1.0) - 0.0).abs() < 1e-4);
        assert!(on_ring(60.0, 0.0));
        assert!(!on_ring(10.0, 0.0));
    }

    #[test]
    fn slot_alpha_is_preserved() {
        assert_eq!(
            with_slot_alpha("#ff000080", "#00ff00".to_string()),
            "#00ff0080"
        );
        assert_eq!(with_slot_alpha("#ff0000", "#00ff00".to_string()), "#00ff00");
    }
}
