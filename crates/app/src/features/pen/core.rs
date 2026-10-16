//! Pen stroke handling — `pointerDown/Move/Up`, alt-sampling, shift lines.

use pixel_engine::composite::sample_pixel;
use pixel_engine::model::edit_session::EditOptions;
use pixel_engine::model::event::EventDetail;
use pixel_engine::model::{LayerChangeReason, ModelEvent};
use pixel_engine::pixel::{pack_rgba, parse_hex, to_hex8};
use pixel_engine::raster::brush::{stroke_point, stroke_segment};

use crate::input::{Button, HoverEvent, ToolEvent};
use crate::tools::api::{CursorKind, OverlayInfo, Tool, ToolCx};
use crate::tools::overlay::OverlayOp;

/// `PenTool::new` rejects anything else, like the JS constructor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PenMode {
    Draw,
    Erase,
}

pub struct PenTool {
    pub(super) mode: PenMode,
    pub(super) drawing: bool,
    ignore_stroke: bool,
    pub(super) size: usize,
    packed: u32,
    pub(super) last_point: Option<(i32, i32)>,
    pub(super) hover: Option<(i32, i32)>,
    /// `hover()` is the only overlay path with a session, so it caches the
    /// live `penSize` for the `zoom < 2` preview (`_safePenSize` in the JS).
    pub(super) preview_size: usize,
}

impl PenTool {
    pub fn pen() -> Self {
        Self::with_mode(PenMode::Draw)
    }

    pub fn eraser() -> Self {
        Self::with_mode(PenMode::Erase)
    }

    pub fn with_mode(mode: PenMode) -> Self {
        Self {
            mode,
            drawing: false,
            ignore_stroke: false,
            size: 1,
            packed: 0,
            last_point: None,
            hover: None,
            preview_size: 1,
        }
    }

    fn label(&self) -> &'static str {
        match self.mode {
            PenMode::Draw => "연필",
            PenMode::Erase => "지우개",
        }
    }

    fn pick_color(cx: &ToolCx, mode: PenMode) -> u32 {
        if mode == PenMode::Erase {
            return 0;
        }
        let hex = cx.session.settings().primary_color;
        match parse_hex(&hex) {
            Some(c) => pack_rgba(c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64),
            // Unparseable setting: opaque black, exactly like the JS fallback.
            None => pack_rgba(0.0, 0.0, 0.0, 255.0),
        }
    }

    /// Alt-click sampling: composite colour → `primaryColor`, transparent →
    /// `"투명 픽셀"`. No stroke is opened, like the JS `_eyedrop` early return.
    fn eyedrop(&self, ev: &ToolEvent, cx: &mut ToolCx) {
        let Some(doc) = cx.session.doc() else {
            return;
        };
        let rgba = sample_pixel(
            &doc.layer_sources(),
            ev.x,
            ev.y,
            doc.canvas.width_px as i32,
            doc.canvas.height_px as i32,
        );
        if rgba[3] == 0 {
            cx.session.notify("info", "투명 픽셀", "TRANSPARENT_PIXEL");
            return;
        }
        let hex = to_hex8(
            rgba[0] as f64,
            rgba[1] as f64,
            rgba[2] as f64,
            rgba[3] as f64,
        );
        let _ = cx
            .session
            .set_setting("primaryColor", pixel_engine::model::SettingInput::Str(&hex));
    }

    fn finish_up(&mut self, cx: &mut ToolCx) {
        self.drawing = false;
        self.ignore_stroke = false;
        if cx.session.has_stroke() {
            // Commit failure (e.g. a layer deleted mid-stroke) must not break
            // input — the JS swallowed the same way.
            let _ = cx.session.stroke_commit(Some(self.label()));
        }
    }

    fn cancel_stroke(&mut self, cx: &mut ToolCx) {
        if cx.session.has_stroke() {
            cx.session.stroke_cancel();
        }
        self.drawing = false;
        self.ignore_stroke = false;
    }
}

impl Tool for PenTool {
    fn id(&self) -> &'static str {
        match self.mode {
            PenMode::Draw => "pen",
            PenMode::Erase => "eraser",
        }
    }

    fn cursor(&self) -> CursorKind {
        match self.mode {
            PenMode::Draw => CursorKind::Pen,
            PenMode::Erase => CursorKind::Eraser,
        }
    }

    fn activate(&mut self, _cx: &mut ToolCx) {
        self.last_point = None;
    }

    fn deactivate(&mut self, cx: &mut ToolCx) {
        self.cancel_stroke(cx);
        self.last_point = None;
    }

    fn pointer_down(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if ev.button != Button::Left || self.drawing {
            return;
        }
        if ev.alt {
            self.eyedrop(ev, cx);
            return;
        }
        if cx
            .session
            .stroke_begin(EditOptions::on_active_layer().labeled(self.label()))
            .is_err()
        {
            self.ignore_stroke = true;
            self.drawing = false;
            return;
        }
        self.drawing = true;
        self.ignore_stroke = false;
        self.size = cx.session.settings().pen_size as usize;
        self.preview_size = self.size;
        self.packed = Self::pick_color(cx, self.mode);
        if ev.shift && self.last_point.is_some() {
            let (lx, ly) = self.last_point.expect("shift path checks last_point");
            let size = self.size;
            let packed = self.packed;
            let _ = cx
                .session
                .stroke_with(|w| stroke_segment(w, lx, ly, ev.x, ev.y, size, packed));
            self.last_point = Some((ev.x, ev.y));
            cx.session.stroke_flush();
            self.finish_up(cx);
            return;
        }
        let (size, packed) = (self.size, self.packed);
        let _ = cx
            .session
            .stroke_with(|w| stroke_point(w, ev.x, ev.y, size, packed));
        self.last_point = Some((ev.x, ev.y));
        self.hover = Some((ev.x, ev.y));
        cx.session.stroke_flush();
    }

    fn pointer_move(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if self.ignore_stroke || !self.drawing || !cx.session.has_stroke() {
            return;
        }
        let pts: &[(i32, i32)] = if ev.coalesced.is_empty() {
            &[(ev.x, ev.y)]
        } else {
            &ev.coalesced
        };
        let (size, packed) = (self.size, self.packed);
        let mut last = self.last_point;
        let _ = cx.session.stroke_with(|w| {
            for &(px, py) in pts {
                if let Some((lx, ly)) = last {
                    stroke_segment(w, lx, ly, px, py, size, packed);
                }
                last = Some((px, py));
            }
        });
        self.last_point = last.or(self.last_point);
        if let Some(lp) = self.last_point {
            self.hover = Some(lp);
        }
        cx.session.stroke_flush();
    }

    fn pointer_up(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if self.ignore_stroke {
            self.ignore_stroke = false;
            self.drawing = false;
            return;
        }
        if !self.drawing || !cx.session.has_stroke() {
            return;
        }
        // `ToolEvent.x/y` are always integers, so the JS `isInteger` guard holds.
        if let Some((lx, ly)) = self.last_point
            && (ev.x != lx || ev.y != ly)
        {
            let (size, packed) = (self.size, self.packed);
            let _ = cx
                .session
                .stroke_with(|w| stroke_segment(w, lx, ly, ev.x, ev.y, size, packed));
            self.last_point = Some((ev.x, ev.y));
        }
        self.hover = Some((ev.x, ev.y));
        self.finish_up(cx);
    }

    fn cancel(&mut self, cx: &mut ToolCx) {
        // Parity: `cancel` keeps `last_point` (only `deactivate` clears it).
        self.cancel_stroke(cx);
    }

    fn hover(&mut self, ev: Option<&HoverEvent>, cx: &mut ToolCx) {
        self.hover = ev.map(|h| (h.x, h.y));
        self.preview_size = cx.session.settings().pen_size as usize;
        cx.request_render();
    }

    fn on_model_event(&mut self, ev: &ModelEvent, _cx: &mut ToolCx) {
        match &ev.detail {
            EventDetail::DocumentReplaced => self.last_point = None,
            EventDetail::Layers { reason, .. } if *reason == LayerChangeReason::Active => {
                self.last_point = None;
            }
            _ => {}
        }
    }

    fn overlay(&self, info: &OverlayInfo, out: &mut Vec<OverlayOp>) {
        super::preview::overlay_preview(self, info, out);
    }
}
