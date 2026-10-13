//! Theme — the native replacement for `style_*.css`.
//!
//! JS used four cascading tiers (base/polish/juicy/picker). egui has no
//! cascade: one [`egui::Visuals`] + a few [`egui::Style`] tweaks own the whole
//! look. Tokens follow `game_design_visual_reference.md` §5.1 (deep navy +
//! neon accents); no extra crates, no runtime assets.

use egui::{Color32, Stroke, Vec2, Visuals};

/// Deep-navy void + neon rim, matching the JS dark UI.
pub fn neon_dark() -> Visuals {
    let mut v = Visuals::dark();
    v.panel_fill = Color32::from_rgb(0x0d, 0x13, 0x22);
    v.window_fill = Color32::from_rgb(0x0d, 0x13, 0x22);
    v.extreme_bg_color = Color32::from_rgb(0x07, 0x0a, 0x10);
    v.code_bg_color = Color32::from_rgb(0x07, 0x0a, 0x10);
    v.faint_bg_color = Color32::from_rgb(0x1a, 0x25, 0x3d);
    v.hyperlink_color = Color32::from_rgb(0x00, 0xd2, 0xff);
    v.selection.bg_fill = Color32::from_rgb(0x00, 0x9e, 0xcc);
    v.selection.stroke = Stroke::new(1.0, Color32::from_rgb(0x00, 0xd2, 0xff));
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = 6.into();
    }
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x1a, 0x25, 0x3d);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x00, 0xd2, 0xff));
    v.widgets.active.bg_fill = Color32::from_rgb(0x00, 0x9e, 0xcc);
    v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x1a, 0x25, 0x3d);
    v.window_stroke = Stroke::new(1.0, Color32::from_rgb(0x1a, 0x25, 0x3d));
    v.window_corner_radius = 8.into();
    v.menu_corner_radius = 6.into();
    v
}

/// Spacing shared by every panel: tighter than egui defaults so 14 tools and
/// both side panels fit a 1280 window without clipping.
pub fn apply_spacing(style: &mut egui::Style) {
    style.spacing.button_padding = Vec2::new(6.0, 3.0);
    style.spacing.item_spacing = Vec2::new(6.0, 4.0);
    style.spacing.menu_margin = 8.0.into();
    // (scroll bar width is not exposed in egui 0.36 Spacing; leave default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_is_dark_navy() {
        let v = neon_dark();
        assert_eq!(v.panel_fill, Color32::from_rgb(0x0d, 0x13, 0x22));
        assert_eq!(v.hyperlink_color, Color32::from_rgb(0x00, 0xd2, 0xff));
    }
}
