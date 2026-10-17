//! Pixel-exact texture upload for the native canvas.
//!
//! The engine paints CPU framebuffers; this module is the only place that
//! hands those bytes to the GPU. `NEAREST` on both filters keeps zoomed pixels
//! sharp, and the upload is **not** once per frame unconditionally: the
//! [`UploadTracker`] compares a [`FrameStamp`] (view, viewport, dpr, background,
//! grid) plus this frame's damage — the chunks `comp.update` repainted and the
//! boxes of the previous/current hover overlays — and the texture only sees
//! [`Upload::Skip`], [`Upload::Full`] (first frame or a stamp change) or the one
//! damaged [`Upload::Region`] via `TextureHandle::set_partial`.
//!
//! One caveat sits behind `set_partial`: epaint hands the painter a delta, not
//! an image, so a region upload assumes the painter's texture survived from the
//! last full upload. In eframe's glow integration the painter (and its texture
//! map) is built once per window and only rebuilt together with everything else,
//! and every stamp change — a resize above all — uploads in full again.

use egui::{ColorImage, TextureHandle, TextureOptions};
use pixel_engine::display::{Framebuffer, GridMode};

/// A device-px `(x, y, w, h)` box.
pub type Rect = (i32, i32, i32, i32);

/// The frame inputs the last upload depended on. Same stamp + no damage ⇒ the
/// texture still matches the framebuffer, so nothing needs to move.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameStamp {
    pub zoom: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub css_w: f64,
    pub css_h: f64,
    pub dpr: f64,
    pub canvas_w: u32,
    pub canvas_h: u32,
    pub grid: GridMode,
    pub background: String,
}

/// What this frame's texture needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upload {
    /// Nothing the texture shows has changed.
    Skip,
    /// A new texture, a resize or a view change — upload the whole frame.
    Full,
    /// Only this device-px region changed.
    Region(Rect),
}

impl Upload {
    /// Fold one damaged box into the frame's decision.
    pub fn add(&mut self, rect: Option<Rect>) {
        let Some(rect) = rect else { return };
        *self = match *self {
            Upload::Full => Upload::Full,
            Upload::Skip => Upload::Region(rect),
            Upload::Region(cur) => Upload::Region(union_rect(cur, rect)),
        };
    }
}

/// The smallest `(x, y, w, h)` covering both boxes.
pub fn union_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.0.min(b.0);
    let y0 = a.1.min(b.1);
    let x1 = (a.0 + a.2).max(b.0 + b.2);
    let y1 = (a.1 + a.3).max(b.1 + b.3);
    (x0, y0, x1 - x0, y1 - y0)
}

/// [`union_rect`] over two optional boxes; `None` when both are missing.
pub fn union_opt(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(union_rect(a, b)),
        (a, b) => a.or(b),
    }
}

/// `a ∩ b`, or `None` when they do not overlap (engine `intersect_rects`, kept
/// local because that one is crate-private to `pixel_engine`).
pub fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.0.max(b.0);
    let y0 = a.1.max(b.1);
    let x1 = (a.0 + a.2).min(b.0 + b.2);
    let y1 = (a.1 + a.3).min(b.1 + b.3);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some((x0, y0, x1 - x0, y1 - y0))
}

/// Cross-frame upload memory — what the texture on the GPU currently holds.
#[derive(Debug, Default)]
pub struct UploadTracker {
    stamp: Option<FrameStamp>,
    overlay: Option<Rect>,
}

impl UploadTracker {
    /// The frame's opening decision: `Full` unless the stamp is unchanged.
    /// Callers add damage to a `Skip`/`Full` result with [`Upload::add`].
    pub fn begin(&self, stamp: &FrameStamp) -> Upload {
        if self.stamp.as_ref() == Some(stamp) {
            Upload::Skip
        } else {
            Upload::Full
        }
    }

    /// The previous frame's overlay box in device px — part of this frame's
    /// damage, because erasing the old preview paints over those pixels.
    pub fn overlay_rect(&self) -> Option<Rect> {
        self.overlay
    }

    /// Record what this frame drew; every path calls it, upload or not.
    pub fn finish(&mut self, stamp: FrameStamp, overlay: Option<Rect>) {
        self.stamp = Some(stamp);
        self.overlay = overlay;
    }
}

pub struct CanvasTexture {
    handle: Option<TextureHandle>,
    size: [usize; 2],
    tracker: UploadTracker,
    /// Row-compacted staging for region uploads, reused across frames.
    scratch: Vec<u8>,
}

impl CanvasTexture {
    pub fn new() -> Self {
        Self {
            handle: None,
            size: [0, 0],
            tracker: UploadTracker::default(),
            scratch: Vec::new(),
        }
    }

    /// The frame's upload memory; `canvas_view::render_frame` reads and writes it.
    pub fn tracker_mut(&mut self) -> &mut UploadTracker {
        &mut self.tracker
    }

    /// Apply the frame's decided upload to the GPU.
    pub fn present(&mut self, ctx: &egui::Context, fb: &Framebuffer, upload: Upload) {
        match upload {
            Upload::Skip => {}
            Upload::Full => self.upload(ctx, &fb.rgba, fb.width as usize, fb.height as usize),
            Upload::Region(rect) => self.upload_region(ctx, fb, rect),
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

    /// `set_partial` for one device-px region of the current framebuffer.
    ///
    /// The region path only runs once the tracker says the stamp is unchanged,
    /// so a missing or differently-sized texture here is an invariant break —
    /// fall back to the full upload rather than set a partial delta on a
    /// texture whose size the delta no longer matches.
    fn upload_region(&mut self, ctx: &egui::Context, fb: &Framebuffer, rect: Rect) {
        let size = [fb.width as usize, fb.height as usize];
        if self.handle.is_none() || self.size != size {
            return self.upload(ctx, &fb.rgba, size[0], size[1]);
        }
        let bounds = (0, 0, fb.width as i32, fb.height as i32);
        let Some((x, y, w, h)) = intersect_rect(rect, bounds) else {
            return;
        };
        // Rows are the contiguous runs; a stride of `fb.width * 4` forbids one
        // slice, so the scratch buffer re-gathers them.
        self.scratch.clear();
        self.scratch.reserve(w as usize * h as usize * 4);
        for row in 0..h as usize {
            let start = ((y as usize + row) * fb.width as usize + x as usize) * 4;
            self.scratch
                .extend_from_slice(&fb.rgba[start..start + w as usize * 4]);
        }
        let image = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &self.scratch);
        if let Some(handle) = self.handle.as_mut() {
            handle.set_partial([x as usize, y as usize], image, TextureOptions::NEAREST);
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

    fn stamp(zoom: f64) -> FrameStamp {
        FrameStamp {
            zoom,
            offset_x: 1.0,
            offset_y: 2.0,
            css_w: 640.0,
            css_h: 480.0,
            dpr: 1.0,
            canvas_w: 2048,
            canvas_h: 1088,
            grid: GridMode::Off,
            background: "transparent".to_string(),
        }
    }

    #[test]
    fn the_tracker_skips_until_the_stamp_changes() {
        let mut t = UploadTracker::default();
        let s = stamp(2.0);
        assert_eq!(
            t.begin(&s),
            Upload::Full,
            "the first frame has no texture yet"
        );
        t.finish(s.clone(), None);
        assert_eq!(t.begin(&s), Upload::Skip);
        let mut moved = s.clone();
        moved.offset_x = 1.5;
        assert_eq!(
            t.begin(&moved),
            Upload::Full,
            "a pan invalidates the texture"
        );
    }

    #[test]
    fn damage_folds_into_one_region_and_full_wins() {
        let mut u = Upload::Skip;
        u.add(None);
        assert_eq!(u, Upload::Skip);
        u.add(Some((4, 5, 6, 7)));
        assert_eq!(u, Upload::Region((4, 5, 6, 7)));
        u.add(Some((10, 0, 2, 2)));
        assert_eq!(u, Upload::Region((4, 0, 8, 12)), "the union, not a list");
        u.add(Some((0, 0, 1, 1)));
        assert_eq!(u, Upload::Region((0, 0, 12, 12)));
        // Full swallows late damage; a region can never downgrade it.
        let mut f = Upload::Full;
        f.add(Some((3, 3, 3, 3)));
        assert_eq!(f, Upload::Full);
    }

    #[test]
    fn intersect_rect_reports_empty_overlaps() {
        assert_eq!(
            intersect_rect((0, 0, 4, 4), (2, 2, 4, 4)),
            Some((2, 2, 2, 2))
        );
        assert_eq!(
            intersect_rect((0, 0, 2, 2), (2, 0, 2, 2)),
            None,
            "touching edges do not overlap"
        );
        assert_eq!(intersect_rect((-4, -4, 2, 2), (0, 0, 2, 2)), None);
    }

    #[test]
    fn present_maps_skip_full_and_region_to_texture_deltas() {
        /// One pending delta, flattened for assertions: `(pos, size, pixels)`.
        type Shot = (Option<[usize; 2]>, [usize; 2], Vec<egui::Color32>);

        /// Take the pending deltas and `clear` them — epaint panics on drop if
        /// a delta was never handled, so an assert failure inside this test
        /// must not double-panic on the guard.
        fn take_deltas(ctx: &egui::Context) -> Vec<Shot> {
            let mut deltas = ctx.tex_manager().write().take_delta();
            let out = deltas
                .set
                .values()
                .flatten()
                .map(|d| match &d.image {
                    egui::ImageData::Color(image) => (d.pos, image.size, image.pixels.clone()),
                })
                .collect();
            deltas.clear();
            out
        }

        let ctx = egui::Context::default();
        let mut fb = Framebuffer::new(4, 3);
        fb.fill_rect(0, 0, 4, 3, [7, 8, 9, 255]);
        let mut tex = CanvasTexture::new();

        // No texture yet: even a Region has to allocate it in full, because a
        // partial delta on missing storage would only set the corner. (The
        // context's own font atlas also allocates, hence the size search.)
        tex.present(&ctx, &fb, Upload::Region((1, 1, 1, 1)));
        let deltas = take_deltas(&ctx);
        let alloc = deltas
            .iter()
            .find(|d| d.1 == [4, 3])
            .expect("the canvas texture allocation");
        assert_eq!(alloc.0, None, "the fallback is a full upload");

        // A matching texture + region is a real partial delta, bytes included.
        tex.present(&ctx, &fb, Upload::Region((1, 1, 2, 2)));
        let deltas = take_deltas(&ctx);
        assert_eq!(deltas.len(), 1, "one delta, and it is the canvas patch");
        assert_eq!(deltas[0].0, Some([1, 1]));
        assert_eq!(deltas[0].1, [2, 2]);
        assert_eq!(deltas[0].2.len(), 4);
        assert_eq!(deltas[0].2[0], egui::Color32::from_rgb(7, 8, 9));
        assert_eq!(deltas[0].2[3], egui::Color32::from_rgb(7, 8, 9));

        // Skip, and a region clipped away entirely, upload nothing at all.
        tex.present(&ctx, &fb, Upload::Skip);
        tex.present(&ctx, &fb, Upload::Region((9, 9, 2, 2)));
        assert!(take_deltas(&ctx).is_empty());

        // A resize is a full upload again.
        tex.present(&ctx, &fb, Upload::Full);
        let deltas = take_deltas(&ctx);
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].0, None);
        assert_eq!(deltas[0].1, [4, 3]);
    }
}
