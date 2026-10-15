//! Hover/click micro-animation — the native cut of the `style_juicy.css` tier.
//!
//! Two primitives, both frame-driven:
//!
//! - [`hover_fade`] wraps `ctx.animate_bool_responsive` (cubic-out over the
//!   style `animation_time`). egui requests repaints while it runs, so fades
//!   play without any pump on our side.
//! - [`click_flash`] is a one-shot white flash over `FLASH_SECS`. It keeps its
//!   own clock table (widget key → click time) and requests repaints while a
//!   flash is live; [`click_pop`] is the same clock as a press scale.
//!
//! Custom-painted controls (toolbar icons, swatches) read the values before
//! painting. Stock `egui::Button`s — whose hover state is unknowable before
//! `ui.add` returns — use the overlay helpers [`hover_glow`]/[`click_flash`]
//! after the response comes back.

use std::collections::HashMap;

/// Flash lifetime in seconds (~11 frames at 60fps).
pub const FLASH_SECS: f64 = 0.18;

/// Peak overlay alpha for hover glow and click flash.
pub const GLOW_ALPHA: u8 = 36;
pub const FLASH_ALPHA: u8 = 90;

/// Responsive hover fade 0..1 for a stable widget id.
pub fn hover_fade(ctx: &egui::Context, id: egui::Id, hovered: bool) -> f32 {
    ctx.animate_bool_responsive(id, hovered)
}

/// White hover glow over an already-placed widget. Returns the fade for
/// callers that also want to drive something else off it.
pub fn hover_glow(ui: &mut egui::Ui, id: egui::Id, resp: &egui::Response, radius: f32) -> f32 {
    let f = hover_fade(ui.ctx(), id, resp.hovered());
    if f > 0.0 {
        ui.painter().rect_filled(
            resp.rect,
            radius,
            egui::Color32::from_white_alpha((GLOW_ALPHA as f32 * f) as u8),
        );
    }
    f
}

/// One-shot click flash overlay. Call every frame with the response; the
/// flash plays off the stored click time, so the frame that *places* the
/// widget and the frame that *reports* the click stay decoupled.
pub fn click_flash(
    clicks: &mut HashMap<String, f64>,
    ui: &mut egui::Ui,
    key: &str,
    resp: &egui::Response,
    radius: f32,
) {
    let now = ui.ctx().input(|i| i.time);
    if resp.clicked() {
        clicks.insert(key.to_string(), now);
    }
    let Some(t0) = clicks.get(key).copied() else {
        return;
    };
    let dt = now - t0;
    if dt < 0.0 || dt >= FLASH_SECS {
        if dt >= FLASH_SECS {
            clicks.remove(key);
        }
        return;
    }
    ui.ctx().request_repaint();
    let alpha = (FLASH_ALPHA as f64 * (1.0 - dt / FLASH_SECS)) as u8;
    ui.painter()
        .rect_filled(resp.rect, radius, egui::Color32::from_white_alpha(alpha));
}

/// Press scale from the same clock table: dips to 0.84 and springs back.
/// Returns 1.0 when idle. Drives custom-painted rects (swatches, icons).
pub fn click_pop(
    clicks: &mut HashMap<String, f64>,
    ctx: &egui::Context,
    key: &str,
    clicked: bool,
) -> f32 {
    let now = ctx.input(|i| i.time);
    if clicked {
        clicks.insert(key.to_string(), now);
    }
    let Some(t0) = clicks.get(key).copied() else {
        return 1.0;
    };
    let dt = now - t0;
    if dt < 0.0 || dt >= FLASH_SECS {
        if dt >= FLASH_SECS {
            clicks.remove(key);
        }
        return 1.0;
    }
    ctx.request_repaint();
    1.0 - 0.16 * (std::f64::consts::PI * dt / FLASH_SECS).sin() as f32
}

/// Stock-button juice: hover glow + click flash in one call, for controls
/// built on `egui::Button` (whose hover state is unknowable before `ui.add`
/// returns, so pre-paint lerps are impossible — the overlay does it after).
/// Disabled buttons skip the juice and behave like plain `add_enabled`.
pub fn button(
    ui: &mut egui::Ui,
    clicks: &mut HashMap<String, f64>,
    key: &str,
    enabled: bool,
    btn: egui::Button<'_>,
) -> egui::Response {
    if !enabled {
        return ui.add_enabled(false, btn);
    }
    let resp = ui.add(btn);
    let hid = ui.id().with("jbtn").with(key);
    hover_glow(ui, hid, &resp, 4.0);
    click_flash(clicks, ui, key, &resp, 4.0);
    resp
}

/// Channel-wise color lerp for hover fills.
pub fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    egui::Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_color_hits_both_ends_and_the_middle() {
        let a = egui::Color32::BLACK;
        let b = egui::Color32::WHITE;
        assert_eq!(lerp_color(a, b, 0.0), a);
        assert_eq!(lerp_color(a, b, 1.0), b);
        assert_eq!(
            lerp_color(a, b, 0.5),
            egui::Color32::from_rgb(127, 127, 127)
        );
        assert_eq!(lerp_color(a, b, -1.0), a);
        assert_eq!(lerp_color(a, b, 2.0), b);
    }
}
