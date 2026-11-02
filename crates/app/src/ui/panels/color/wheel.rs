//! HSV wheel painting, interaction and texture cache.

use super::{angle_to_hue, hsv_to_rgb, on_ring, rgb_to_hsv, with_slot_alpha};
use egui::{Color32, Ui};

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

/// The part of the wheel the current gesture started on. Locking it on
/// pointer-down keeps a ring drag from jumping into the SV square when the
/// pointer crosses the middle (and vice versa) — the JS binds the two canvases
/// separately, so each gesture owns exactly one target.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum WheelTarget {
    #[default]
    None,
    Hue,
    Sv,
}

pub(super) fn wheel_target_at(
    pos: egui::Pos2,
    center: egui::Pos2,
    sv_rect: egui::Rect,
) -> WheelTarget {
    // Square first: its corners reach past `RING_IN - 6`, so testing the ring
    // first would steal the square's corner pixels.
    if sv_rect.contains(pos) {
        WheelTarget::Sv
    } else {
        let dx = pos.x - center.x;
        let dy = pos.y - center.y;
        if on_ring(dx, dy) {
            WheelTarget::Hue
        } else {
            WheelTarget::None
        }
    }
}

/// Cached wheel texture: the hue ring is static — the JS cached the same base
/// in `paintPixels` — so only the SV square follows the hue. The old path
/// rebuilt all 132×132 pixels and re-uploaded them on every frame, which is
/// part of what made left-panel drags feel slow.
#[derive(Clone, Default)]
struct WheelTexture {
    handle: Option<egui::TextureHandle>,
    /// The hue ring pixels, built once and reused for every hue.
    ring: Option<std::sync::Arc<Vec<Color32>>>,
    /// Whole-degree hue the cached pixels were built for.
    hue_deg: i32,
}

pub(super) fn wheel_texture_id() -> egui::Id {
    egui::Id::new("hsv_wheel_texture")
}

/// Quantised cache key: the SV square is 24 blocks wide, so a whole degree is
/// already finer than it can show.
pub(super) fn wheel_hue_key(h: f32) -> i32 {
    h.round() as i32
}

/// The static hue ring: transparent outside `r_in..r_out`, JS `hsvToHex`
/// inside. No hue parameter — that is what makes caching it safe.
pub(super) fn build_ring(n: usize, r_out: f32, r_in: f32) -> Vec<Color32> {
    let size = n as f32;
    let mut pixels = vec![Color32::TRANSPARENT; n * n];
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
    pixels
}

/// Paint the hue-dependent SV square (24×24 blocks) over a ring copy.
/// `sv_left`/`sv_top` are relative to the wheel rect, like the pixels are.
pub(super) fn paint_sv_square(
    pixels: &mut [Color32],
    n: usize,
    sv_left: f32,
    sv_top: f32,
    sv_size: f32,
    h: f32,
) {
    let grid_n = 24usize;
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
}

pub(super) fn show_hsv_wheel(
    ui: &mut Ui,
    current_hex: &str,
    current_color: Color32,
) -> Option<WheelPick> {
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

    // Lock the target on press, not on every move. A ring drag can then pass
    // over the square without changing saturation, and a square drag can cross
    // the ring without changing hue.
    let target_id = response.id.with("drag_target");
    if ui.input(|i| i.pointer.primary_pressed())
        && let Some(pos) = response.interact_pointer_pos()
    {
        ui.data_mut(|d| d.insert_temp(target_id, wheel_target_at(pos, center, sv_rect)));
    }
    let target = ui
        .data(|d| d.get_temp::<WheelTarget>(target_id))
        .unwrap_or_default();
    let interacting =
        response.dragged() || response.clicked() || response.is_pointer_button_down_on();
    if interacting && let Some(pos) = response.interact_pointer_pos() {
        match target {
            WheelTarget::Hue => {
                let dx = pos.x - center.x;
                let dy = pos.y - center.y;
                // JS `pickHue`: off-ring offsets are ignored while the ring
                // gesture owns the pointer.
                if on_ring(dx, dy) {
                    h = angle_to_hue(dx, dy);
                    picked = Some((h, s, v));
                }
            }
            WheelTarget::Sv => {
                // JS `pointerToSv`: clamped, corners reachable.
                let rel_x = ((pos.x - sv_rect.left()) / sv_size).clamp(0.0, 1.0);
                let rel_y = ((pos.y - sv_rect.top()) / sv_size).clamp(0.0, 1.0);
                s = rel_x;
                v = 1.0 - rel_y;
                picked = Some((h, s, v));
            }
            WheelTarget::None => {}
        }
    }
    if ui.input(|i| i.pointer.any_released()) {
        ui.data_mut(|d| d.remove_temp::<WheelTarget>(target_id));
    }

    // The wheel image is cached: the ring is static and the SV square only
    // follows the hue, so an idle panel reuses both the pixels and the GPU
    // texture. The cache is pulled out before `load_texture` so no data lock
    // is held across a texture upload.
    let n = size as usize;
    let hue_key = wheel_hue_key(h);
    let (cached, cached_ring, cached_key) = ui.ctx().data_mut(|d| {
        let cache = d.get_temp_mut_or_default::<WheelTexture>(wheel_texture_id());
        (cache.handle.clone(), cache.ring.clone(), cache.hue_deg)
    });
    let texture = match cached.filter(|_| cached_key == hue_key) {
        Some(handle) => handle,
        None => {
            let ring =
                cached_ring.unwrap_or_else(|| std::sync::Arc::new(build_ring(n, r_out, r_in)));
            let mut pixels = (*ring).clone();
            paint_sv_square(
                &mut pixels,
                n,
                sv_rect.left() - rect.left(),
                sv_rect.top() - rect.top(),
                sv_size,
                h,
            );
            let image = egui::ColorImage {
                size: [n, n],
                source_size: egui::vec2(n as f32, n as f32),
                pixels,
            };
            let handle = ui
                .ctx()
                .load_texture("hsv_wheel", image, egui::TextureOptions::default());
            ui.ctx().data_mut(|d| {
                let cache = d.get_temp_mut_or_default::<WheelTexture>(wheel_texture_id());
                cache.handle = Some(handle.clone());
                cache.ring = Some(ring);
                cache.hue_deg = hue_key;
            });
            handle
        }
    };
    ui.painter().image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );

    // 3. Hue Marker
    let painter = ui.painter();
    let hue_rad = (h - 90.0).to_radians();
    // The marker sits on the ring's mid radius, which is exactly RING_MID.
    let marker_pos = center + egui::Vec2::new(hue_rad.cos(), hue_rad.sin()) * RING_MID;
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
