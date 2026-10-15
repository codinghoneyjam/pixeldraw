//! Pen / eraser option block: size box, quick presets, footprint preview.
use crate::session_glue::AppState;
use egui::{Color32, DragValue, RichText, Ui, Vec2};

pub fn pen_options(
    ui: &mut Ui,
    juice_clicks: &mut std::collections::HashMap<String, f64>,
    options: &mut super::ToolOptions,
    app: &mut AppState,
) {
    ui.label(RichText::new("브러시 크기:").size(12.0));
    let mut cur_size = options.size.unwrap_or(1);
    let resp = ui.add(
        DragValue::new(&mut cur_size)
            .range(1..=64)
            .speed(1.0)
            .suffix(" px"),
    );
    if resp.changed() {
        options.size = Some(cur_size);
        let _ = app.session.set_setting(
            "penSize",
            pixel_engine::model::SettingInput::Num(cur_size as f64),
        );
    }

    // Quick preset buttons (1, 2, 4, 8, 16, 32) — square, so the number
    // grid reads as one instrument like the brush panel's presets.
    ui.label(RichText::new("프리셋:").size(11.0).color(Color32::GRAY));
    for preset in [1, 2, 4, 8, 16, 32] {
        let active = cur_size == preset;
        let btn = egui::Button::new(RichText::new(format!("{preset}")).size(11.0))
            .min_size(Vec2::splat(22.0))
            .fill(if active {
                Color32::from_rgb(0x00, 0x96, 0xff)
            } else {
                Color32::from_rgb(0x2d, 0x30, 0x36)
            });
        if crate::ui::juice::button(ui, juice_clicks, &format!("pensize:{preset}"), true, btn)
            .clicked()
        {
            options.size = Some(preset);
            let _ = app.session.set_setting(
                "penSize",
                pixel_engine::model::SettingInput::Num(preset as f64),
            );
        }
    }

    // Mini preview footprint (22x22 box)
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, Color32::from_rgb(0x23, 0x24, 0x28));
    painter.rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3d, 0x43)),
        egui::StrokeKind::Inside,
    );
    let radius = (cur_size as f32 * 0.5).clamp(1.0, 9.0);
    painter.circle_filled(rect.center(), radius, Color32::WHITE);
}
