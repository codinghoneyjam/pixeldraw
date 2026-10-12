//! Pixel-exact texture upload for the native canvas.
//!
//! The engine paints CPU framebuffers; this module is the only place that
//! hands those bytes to the GPU. `NEAREST` on both filters keeps zoomed pixels
//! sharp, and uploads happen only when the size or the bytes change — never
//! once per frame unconditionally.

use egui::{ColorImage, TextureHandle, TextureOptions};

pub struct CanvasTexture {
    handle: Option<TextureHandle>,
    size: [usize; 2],
}

impl CanvasTexture {
    pub fn new() -> Self {
        Self {
            handle: None,
            size: [0, 0],
        }
    }

    pub fn size(&self) -> [usize; 2] {
        self.size
    }

    pub fn has_texture(&self) -> bool {
        self.handle.is_some()
    }

    /// Upload `fb` (`w*h*4` straight-alpha RGBA). Same size refreshes the
    /// texture in place; a new size recreates it.
    pub fn upload(&mut self, ctx: &egui::Context, fb: &[u8], w: usize, h: usize) {
        assert_eq!(fb.len(), w * h * 4, "framebuffer must be w*h*4 bytes");
        let image = ColorImage::from_rgba_unmultiplied([w, h], fb);
        match self.handle.as_mut() {
            Some(handle) if self.size == [w, h] => {
                handle.set(image, TextureOptions::NEAREST);
            }
            _ => {
                self.handle = Some(ctx.load_texture("canvas", image, TextureOptions::NEAREST));
                self.size = [w, h];
            }
        }
    }

    /// Paint the texture 1:1 — screen rect size equals texture size, full UV.
    /// Callers size the framebuffer from [`device_size`] so this holds.
    pub fn paint(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        if let Some(handle) = &self.handle {
            ui.painter().image(
                handle.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    }
}

impl Default for CanvasTexture {
    fn default() -> Self {
        Self::new()
    }
}

/// Device pixels for a UI rect: `round(size * pixels_per_point)`, at least 1.
/// The framebuffer is cut to this size so [`CanvasTexture::paint`] stays 1:1.
pub fn device_size(rect: egui::Rect, pixels_per_point: f32) -> (u32, u32) {
    let w = (rect.width() * pixels_per_point).round().max(1.0) as u32;
    let h = (rect.height() * pixels_per_point).round().max(1.0) as u32;
    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_size_rounds_and_floors_at_one() {
        let r = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 100.0));
        assert_eq!(device_size(r, 1.0), (200, 100));
        assert_eq!(device_size(r, 1.25), (250, 125));
        // NaN/zero/negative viewports still yield a usable buffer.
        let zero = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(0.0, 0.0));
        assert_eq!(device_size(zero, 1.0), (1, 1));
        assert_eq!(device_size(r, 0.0), (1, 1));
    }

    #[test]
    fn fresh_texture_has_no_handle() {
        let tex = CanvasTexture::new();
        assert!(!tex.has_texture());
        assert_eq!(tex.size(), [0, 0]);
    }
}
