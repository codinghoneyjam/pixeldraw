//! The colour section: fg/bg slots, the HSV wheel, channel and hex fields.
//!
//! Facade over `color/**` so the panel stays about panels:
//!   color/hsv  — the JS-pinned HSV maths
//!   color/hex  — hex parsing/formatting
mod hex;
mod hsv;

pub(crate) use hex::{to_color32, to_hex_full};
pub(crate) use hsv::{angle_to_hue, hsv_to_rgb, on_ring, rgb_to_hsv, with_slot_alpha};

mod wheel;
pub use wheel::RING_IN;
use wheel::show_hsv_wheel;
#[cfg(test)]
use wheel::{
    RING_MID, RING_OUT, SV_PX, WHEEL_PX, WheelTarget, build_ring, wheel_hue_key, wheel_target_at,
};

use super::super::UiState;
use super::chrome::{PanelsState, section_card, section_header};
use crate::session_glue::AppState;
use egui::{Color32, DragValue, RichText, Ui};

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
    // ── Colour section ──────────────────────────────────────────────
    section_card(ui, |ui| {
        if !section_header(ui, &mut ui_state.panels, "color", "색상") {
            return;
        }
        let s = app.session.settings();
        let primary = to_color32(&s.primary_color).unwrap_or(Color32::BLACK);
        let secondary = to_color32(&s.secondary_color).unwrap_or(Color32::WHITE);
        let is_secondary = ui_state.panels.secondary_active;
        let active_color = if is_secondary { secondary } else { primary };

        // Keep the wheel beside the slots when possible; wrap it below them in
        // narrow sidebars so neither control is clipped.
        ui.horizontal_wrapped(|ui| {
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
                    if ui
                        .button("교체")
                        .on_hover_text("전경·배경 교체 (X)")
                        .clicked()
                    {
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
                    if ui.button("초기화").on_hover_text("기본샇 (D)").clicked() {
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
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                for (label, ch) in [("R", 0), ("G", 1), ("B", 2), ("A", 3)] {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
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
                            apply_color(
                                app,
                                &mut ui_state.panels,
                                is_secondary,
                                &to_hex_full(next),
                            );
                        }
                    });
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

    /// The cache key quantises the hue to whole degrees: enough for a 24-block
    /// SV square, coarse enough to reuse the texture between small changes.
    #[test]
    fn wheel_hue_key_quantises_to_whole_degrees() {
        assert_eq!(wheel_hue_key(10.0), 10);
        assert_eq!(wheel_hue_key(10.4), 10);
        assert_eq!(wheel_hue_key(10.49), 10);
        assert_eq!(wheel_hue_key(10.6), 11);
        assert_ne!(wheel_hue_key(10.0), wheel_hue_key(11.0));
    }

    /// The ring is hue-independent by construction — that is what makes
    /// caching it safe. Two builds are identical, the annulus is painted, and
    /// the hole stays clear for the SV square.
    #[test]
    fn the_cached_ring_is_hue_independent() {
        let n = WHEEL_PX as usize;
        let a = build_ring(n, RING_OUT, RING_IN);
        let b = build_ring(n, RING_OUT, RING_IN);
        assert_eq!(a, b);
        let centre = n / 2;
        // Corner: outside the ring entirely.
        assert_eq!(a[0], Color32::TRANSPARENT);
        // Mid-annulus at +62px from the centre: painted.
        assert_ne!(a[centre * n + centre + 62], Color32::TRANSPARENT);
        // The hole the SV square sits in: unpainted.
        assert_eq!(a[centre * n + centre], Color32::TRANSPARENT);
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
    fn wheel_gesture_target_is_locked_to_the_press_area() {
        let center = egui::pos2(66.0, 66.0);
        let sv = egui::Rect::from_center_size(center, egui::Vec2::splat(SV_PX));
        // Square body, including a corner that is radially farther out than
        // the ring's inner hit threshold.
        assert_eq!(wheel_target_at(center, center, sv), WheelTarget::Sv);
        assert_eq!(
            wheel_target_at(sv.right_top() + egui::vec2(-0.5, 0.5), center, sv),
            WheelTarget::Sv
        );
        // Ring band and the transparent area outside it.
        assert_eq!(
            wheel_target_at(center + egui::vec2(0.0, -60.0), center, sv),
            WheelTarget::Hue
        );
        assert_eq!(
            // Inside the hole but outside the 74px square.
            wheel_target_at(center + egui::vec2(0.0, -40.0), center, sv),
            WheelTarget::None
        );
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
