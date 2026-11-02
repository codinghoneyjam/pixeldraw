//! Text stamp state and the mask/commit/placement operations.

use std::cell::RefCell;
use std::collections::BTreeMap;

use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::model::edit_session::EditOptions;
use pixel_engine::raster::gradient::GradientMask;
use pixel_engine::raster::text_mask::{GlyphSet, TextParams, text_mask};

use crate::input::ToolEvent;
use crate::tools::api::ToolCx;
use crate::tools::overlay::OverlayOp;

use super::state::{Mode, PendingText, PreviewKey, bbox_of_mask, load_glyph_sets, pack_of};

pub struct TextTool {
    pub(super) text: String,
    pub(super) font: String,
    pub(super) stroke_width: i32,
    pub(super) glyph_sets: BTreeMap<String, GlyphSet>,
    pub(super) pending: Option<PendingText>,
    pub(super) moving: Option<(i32, i32, PendingText)>,
    pub(super) mode: Mode,
    pub(super) preview: RefCell<Option<(PreviewKey, OverlayOp)>>,
}

impl TextTool {
    pub fn new() -> Self {
        Self::with_glyph_sets(load_glyph_sets())
    }

    pub fn with_glyph_sets(glyph_sets: BTreeMap<String, GlyphSet>) -> Self {
        Self {
            text: String::new(),
            font: "default-42".to_string(),
            stroke_width: 0,
            glyph_sets,
            pending: None,
            moving: None,
            mode: Mode::Idle,
            preview: RefCell::new(None),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// `panel_text_options.js`: the text input applies on every keystroke.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.refresh_pending();
    }

    pub fn font(&self) -> &str {
        &self.font
    }

    pub fn set_font(&mut self, font: impl Into<String>) {
        self.font = font.into();
        self.refresh_pending();
    }

    pub fn stroke_width(&self) -> i32 {
        self.stroke_width
    }

    /// Integer widths clamp to 0..16.
    pub fn set_stroke_width(&mut self, width: i32) -> bool {
        let clamped = width.clamp(0, 16);
        let changed = self.stroke_width != clamped;
        self.stroke_width = clamped;
        if changed {
            self.refresh_pending();
        }
        changed
    }

    /// `panel_text_options.js`: `Number.isInteger` widths clamp, anything
    /// else resets to 0 (the previous value is not kept).
    pub fn set_stroke_width_number(&mut self, width: f64) -> bool {
        if width.is_finite() && width.fract() == 0.0 {
            self.set_stroke_width(width as i32)
        } else {
            let changed = self.stroke_width != 0;
            self.stroke_width = 0;
            if changed {
                self.refresh_pending();
            }
            changed
        }
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn mask_for(&self, p: &PendingText) -> EngineResult<GradientMask> {
        let set = self
            .glyph_sets
            .get(&p.font)
            .ok_or_else(|| DrawToolError::invalid_state("텍스트 글꼴을 불러오지 못했습니다"))?;
        text_mask(&TextParams {
            text: &p.text,
            glyphs: &set.glyphs,
            x: p.anchor.0,
            y: p.anchor.1,
            fill: p.fill,
            stroke: p.stroke,
            stroke_width: p.stroke_width,
            shape_stroke: true,
            w: p.canvas.0 as usize,
            h: p.canvas.1 as usize,
        })
    }

    /// Re-run the mask after a text/font/stroke option change.
    fn refresh_pending(&mut self) {
        let Some(mut p) = self.pending.take() else {
            return;
        };
        p.text = self.text.clone();
        p.font = self.font.clone();
        p.stroke_width = self.stroke_width;
        match self.mask_for(&p) {
            Ok(mask) => match bbox_of_mask(&mask) {
                Some(bbox) => {
                    p.bbox = bbox;
                    self.pending = Some(p);
                    self.preview.borrow_mut().take();
                }
                None => {
                    self.mode = Mode::Idle;
                    self.preview.borrow_mut().take();
                }
            },
            Err(_) => {
                self.mode = Mode::Idle;
                self.preview.borrow_mut().take();
            }
        }
    }

    pub fn discard(&mut self, cx: &mut ToolCx) {
        self.pending = None;
        self.moving = None;
        self.mode = Mode::Idle;
        self.preview.borrow_mut().take();
        cx.request_render();
    }

    /// Bake the pending text as one history entry.
    pub fn commit(&mut self, cx: &mut ToolCx) -> EngineResult<bool> {
        let Some(pending) = self.pending.clone() else {
            self.mode = Mode::Idle;
            return Ok(false);
        };
        let label = "텍스트";
        let mask = self.mask_for(&pending)?;
        if let Err(e) = cx
            .session
            .stroke_begin(EditOptions::on_layer(&pending.layer_id).labeled(label))
        {
            if matches!(e.code, "LAYER_LOCKED" | "LAYER_HIDDEN" | "LAYER_NOT_FOUND") {
                self.discard(cx);
                return Ok(false);
            }
            return Err(e);
        }
        let w = pending.canvas.0 as usize;
        let result = cx.session.stroke_with(|wr| {
            for (i, cell) in mask.data.iter().enumerate() {
                if *cell != 0 {
                    wr.set((i % w) as u32, (i / w) as u32, *cell);
                }
            }
        });
        if let Err(e) = result {
            cx.session.stroke_cancel();
            return Err(e);
        }
        cx.session.stroke_commit(Some(label))?;
        self.pending = None;
        self.moving = None;
        self.mode = Mode::Idle;
        self.preview.borrow_mut().take();
        cx.request_render();
        Ok(true)
    }

    pub(super) fn place(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if self.text.is_empty() {
            return;
        }
        let Some(doc) = cx.session.doc() else {
            return;
        };
        let (w, h) = (doc.canvas.width_px, doc.canvas.height_px);
        let Some(set) = self.glyph_sets.get(&self.font) else {
            cx.session.notify(
                "warn",
                "텍스트 글꼴을 불러오지 못했습니다",
                "TEXT_FONT_MISSING",
            );
            return;
        };
        if let Some(ch) = self.text.chars().find(|c| !set.glyphs.contains_key(c)) {
            cx.session.notify(
                "warn",
                format!("지원하지 않는 문자: {ch}"),
                "TEXT_GLYPH_MISSING",
            );
            return;
        }
        let settings = cx.session.settings();
        let fill = pack_of(&settings.primary_color);
        let stroke = pack_of(&settings.secondary_color);
        let params = TextParams {
            text: &self.text,
            glyphs: &set.glyphs,
            x: ev.x,
            y: ev.y,
            fill,
            stroke,
            stroke_width: self.stroke_width,
            shape_stroke: true,
            w: w as usize,
            h: h as usize,
        };
        let mask = match text_mask(&params) {
            Ok(m) => m,
            Err(e) => {
                cx.report(e);
                return;
            }
        };
        let Some(bbox) = bbox_of_mask(&mask) else {
            return;
        };
        self.pending = Some(PendingText {
            layer_id: doc.active_layer_id.clone(),
            text: self.text.clone(),
            font: self.font.clone(),
            stroke_width: self.stroke_width,
            anchor: (ev.x, ev.y),
            fill,
            stroke,
            bbox,
            canvas: (w, h),
        });
        self.mode = Mode::Pending;
        self.preview.borrow_mut().take();
        cx.request_render();
    }

    pub(super) fn move_to(&mut self, dx: i32, dy: i32, cx: &mut ToolCx) {
        let Some((ox, oy, orig)) = self.moving.clone() else {
            return;
        };
        let mut next = orig;
        next.anchor = (ox + dx, oy + dy);
        if let Ok(mask) = self.mask_for(&next)
            && let Some(bbox) = bbox_of_mask(&mask)
        {
            next.bbox = bbox;
            self.pending = Some(next);
            self.preview.borrow_mut().take();
            cx.request_render();
        }
    }
}

impl Default for TextTool {
    fn default() -> Self {
        Self::new()
    }
}
