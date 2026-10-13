//! Side panels — port of JS left/right panels.
//!
//! JS `index.html` left: color (slots/wheel/fields/palette/recent) + brush
//! (size/presets/preview). Right: layers (opacity/blend/list/buttons).
//! This is the MVP that makes the native shell usable like the JS original:
//! slots + palette + recent + brush size/presets + layer list/opacity/blend.
//! Wheel HSV custom paint and thumbnails are follow-ups.

use super::UiState;
use crate::session_glue::AppState;
use egui::{Color32, RichText, Slider, Ui};

pub const SIZE_PRESETS: [u32; 16] = [1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64];

pub const DEFAULT_PALETTE: [&str; 40] = [
    "#ffffff", "#ffcdd2", "#ffe0b2", "#fff9c4", "#c8e6c9", "#b2ebf2", "#bbdefb", "#e1bee7",
    "#dedede", "#ef5350", "#ffb74d", "#fff176", "#81c784", "#4dd0e1", "#64b5f6", "#ba68c8",
    "#9e9e9e", "#e53935", "#fb8c00", "#fdd835", "#43a047", "#00acc1", "#1e88e5", "#8e24aa",
    "#616161", "#b71c1c", "#e65100", "#f9a825", "#1b5e20", "#006064", "#0d47a1", "#4a148c",
    "#000000", "#5f0a15", "#6d2f00", "#6e5b00", "#103516", "#0f3c45", "#08214d", "#230a32",
];

pub const BLEND_MODES: [(&str, &str); 6] = [
    ("normal", "보통"),
    ("multiply", "곱하기"),
    ("screen", "스크린"),
    ("overlay", "오버레이"),
    ("darken", "어둡게"),
    ("lighten", "밝게"),
];

#[derive(Default)]
pub struct PanelsState {
    /// Which slot palette/recent clicks write to. false = primary, true = secondary.
    pub secondary_active: bool,
    pub recent: Vec<String>,
    pub rename_buffer: String,
    pub renaming: Option<String>,
}

fn parse_hex(hex: &str) -> Option<Color32> {
    let h = hex.trim_start_matches('#');
    let (r, g, b, a) = match h.len() {
        6 => (
            u8::from_str_radix(&h[0..2], 16).ok()?,
            u8::from_str_radix(&h[2..4], 16).ok()?,
            u8::from_str_radix(&h[4..6], 16).ok()?,
            255,
        ),
        8 => (
            u8::from_str_radix(&h[0..2], 16).ok()?,
            u8::from_str_radix(&h[2..4], 16).ok()?,
            u8::from_str_radix(&h[4..6], 16).ok()?,
            u8::from_str_radix(&h[6..8], 16).ok()?,
        ),
        _ => return None,
    };
    Some(Color32::from_rgba_unmultiplied(r, g, b, a))
}

fn to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn blend_label(mode: &str) -> &'static str {
    BLEND_MODES
        .iter()
        .find(|(m, _)| *m == mode)
        .map(|(_, l)| *l)
        .unwrap_or("보통")
}

fn push_recent(recent: &mut Vec<String>, hex: String) {
    recent.retain(|c| c != &hex);
    recent.insert(0, hex);
    recent.truncate(8);
}

fn apply_color(app: &mut AppState, panels: &mut PanelsState, secondary: bool, hex: &str) {
    let key = if secondary { "secondaryColor" } else { "primaryColor" };
    let _ = app.session.set_setting(
        key,
        pixel_engine::model::SettingInput::Str(hex),
    );
    push_recent(&mut panels.recent, hex.to_string());
}

pub fn show_left(ui: &mut Ui, ui_state: &mut UiState, app: &mut AppState) {
    ui.heading("색상");
    {
        let s = app.session.settings();
        let primary = parse_hex(&s.primary_color).unwrap_or(Color32::BLACK);
        let secondary = parse_hex(&s.secondary_color).unwrap_or(Color32::WHITE);
        ui.horizontal(|ui| {
            let mut p = primary;
            let mut q = secondary;
            if ui.color_edit_button_srgba(&mut p).changed() {
                apply_color(app, &mut ui_state.panels, false, &to_hex(p));
            }
            if ui.color_edit_button_srgba(&mut q).changed() {
                apply_color(app, &mut ui_state.panels, true, &to_hex(q));
            }
            if ui.button("교체(X)").clicked() {
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
            if ui.button("초기화(D)").clicked() {
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
        ui.horizontal(|ui| {
            ui.radio_value(&mut ui_state.panels.secondary_active, false, "전경");
            ui.radio_value(&mut ui_state.panels.secondary_active, true, "배경");
        });
    }
    ui.separator();
    ui.label("팔레트");
    egui::Grid::new("palette_grid").num_columns(8).spacing(egui::vec2(2.0, 2.0)).show(ui, |ui| {
        for (i, hex) in DEFAULT_PALETTE.iter().enumerate() {
            let c = parse_hex(hex).unwrap_or(Color32::GRAY);
            let btn = egui::Button::new("").fill(c).min_size(egui::vec2(16.0, 16.0));
            if ui.add(btn).clicked() {
                let secondary = ui_state.panels.secondary_active;
                apply_color(app, &mut ui_state.panels, secondary, hex);
            }
            if (i + 1) % 8 == 0 {
                ui.end_row();
            }
        }
    });
    ui.separator();
    ui.label("최근 색");
    ui.horizontal_wrapped(|ui| {
        for hex in ui_state.panels.recent.clone() {
            let c = parse_hex(&hex).unwrap_or(Color32::GRAY);
            if ui
                .add(egui::Button::new("").fill(c).min_size(egui::vec2(16.0, 16.0)))
                .clicked()
            {
                let secondary = ui_state.panels.secondary_active;
                apply_color(app, &mut ui_state.panels, secondary, &hex);
            }
        }
    });
    ui.separator();
    ui.heading("브러시");
    {
        let size = app.session.settings().pen_size;
        let mut v = size;
        let resp = ui.add(Slider::new(&mut v, 1..=64).text("굵기"));
        if resp.changed() {
            let _ = app.session.set_setting(
                "penSize",
                pixel_engine::model::SettingInput::Num(f64::from(v)),
            );
        }
        egui::Grid::new("brush_presets").num_columns(4).spacing(egui::vec2(4.0, 4.0)).show(ui, |ui| {
            for (i, s) in SIZE_PRESETS.iter().enumerate() {
                let active = *s == app.session.settings().pen_size;
                let label = if active {
                    RichText::new(format!("{s}")).strong()
                } else {
                    RichText::new(format!("{s}"))
                };
                if ui.button(label).clicked() {
                    let _ = app.session.set_setting(
                        "penSize",
                        pixel_engine::model::SettingInput::Num(f64::from(*s)),
                    );
                }
                if (i + 1) % 4 == 0 {
                    ui.end_row();
                }
            }
        });
    }
}

pub fn show_right(ui: &mut Ui, ui_state: &mut UiState, app: &mut AppState) {
    ui.heading("레이어");
    let (active_id, layers_snapshot) = match app.session.doc() {
        Some(d) => (
            d.active_layer_id.clone(),
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
                    )
                })
                .collect::<Vec<_>>(),
        ),
        None => return,
    };
    let active = layers_snapshot.iter().find(|l| l.0 == active_id).cloned();
    if let Some((id, _, _, _, opacity, blend)) = active.clone() {
        ui.horizontal(|ui| {
            ui.label("불투명도:");
            let mut v = (opacity * 100.0).round() as u32;
            let resp = ui.add(Slider::new(&mut v, 0..=100).text("%"));
            if resp.changed() {
                let _ = app.session.set_layer_opacity(&id, f64::from(v) / 100.0, false);
            }
            if resp.drag_stopped() {
                let _ = app.session.set_layer_opacity(&id, f64::from(v) / 100.0, true);
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
    egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
        // Top-first like JS.
        for (id, name, visible, locked, _opacity, _blend) in layers_snapshot.iter().rev() {
            let selected = *id == active_id;
            ui.horizontal(|ui| {
                let eye = if *visible { "눈" } else { "숨김" };
                if ui.small_button(eye).clicked() {
                    let _ = app.session.set_layer_visible(id, !*visible);
                }
                let label = if selected {
                    RichText::new(name.clone()).strong()
                } else {
                    RichText::new(name.clone())
                };
                if ui.selectable_label(selected, label).clicked() {
                    let _ = app.session.set_active_layer(id);
                }
                let lock = if *locked { "잠금" } else { "열림" };
                if ui.small_button(lock).clicked() {
                    let _ = app.session.set_layer_locked(id, !*locked);
                }
            });
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
            } else if selected && ui.small_button("이름변경").clicked() {
                ui_state.panels.renaming = Some(id.clone());
                ui_state.panels.rename_buffer = name.clone();
            }
        }
    });
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        if ui.button("추가").clicked() {
            let _ = app.session.add_layer(None);
        }
        let has_active = app.session.doc().is_some();
        if ui.add_enabled(has_active, egui::Button::new("복제")).clicked() {
            let _ = app.session.duplicate_layer(&active_id);
        }
        let n = app.session.doc().map(|d| d.layers.len()).unwrap_or(0);
        if ui.add_enabled(n > 1, egui::Button::new("삭제")).clicked() {
            let _ = app.session.remove_layer(&active_id);
        }
        if ui.button("병합").clicked() {
            let _ = app.session.merge_down(&active_id);
        }
        if ui.button("위로").clicked() {
            move_active(app, 1);
        }
        if ui.button("아래로").clicked() {
            move_active(app, -1);
        }
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
    fn presets_cover_js_range() {
        assert_eq!(SIZE_PRESETS.len(), 16);
        assert_eq!(SIZE_PRESETS[0], 1);
        assert_eq!(SIZE_PRESETS[15], 64);
    }

    #[test]
    fn palette_has_40_colors() {
        assert_eq!(DEFAULT_PALETTE.len(), 40);
    }

    #[test]
    fn blend_modes_match_js() {
        assert_eq!(BLEND_MODES.len(), 6);
        assert_eq!(BLEND_MODES[0].0, "normal");
        assert_eq!(blend_label("multiply"), "곱하기");
    }
}
