//! The arc/chord tool: drag state, option setters, and the [`Tool`] impl.
//!
//! Options (`start`/`end`/`width`/`chordMode`) are tool-local, not settings:
//! the Phase 3 option bar writes them through the setters below. Colours and
//! geometry freeze at `pointer_down` (the `Draw` snapshot); a mid-drag
//! settings change cannot move a stroke already in flight.

use std::cell::RefCell;

use pixel_engine::model::edit_session::EditOptions;

use crate::input::{Button, ToolEvent};
use crate::tools::api::{CursorKind, OverlayInfo, Tool, ToolCx};
use crate::tools::overlay::OverlayOp;

use super::preview::{PreviewKey, blit_preview, mask_for, pack_of};

/// Which registered tool this struct serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcKind {
    Arc,
    Chord,
}

/// Chord paint style — `chordMode` in the JS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordMode {
    Fill,
    Outline,
}

/// Frozen `pointer_down` state, exactly the JS `_draw` fields.
#[derive(Debug, Clone)]
pub(super) struct Draw {
    pub(super) x0: i32,
    pub(super) y0: i32,
    pub(super) x1: i32,
    pub(super) y1: i32,
    pub(super) layer_id: String,
    pub(super) fill_color: String,
    pub(super) outline_color: String,
    pub(super) start: f64,
    pub(super) end: f64,
    pub(super) width: i32,
    pub(super) chord_mode: ChordMode,
}

pub struct ArcTool {
    kind: ArcKind,
    start: f64,
    end: f64,
    width: i32,
    chord_mode: ChordMode,
    draw: Option<Draw>,
    /// Last-seen canvas size — `overlay()` has no session, like
    /// `ShapeTool.live`.
    canvas: Option<(u32, u32)>,
    /// `(x1,y1,start,end,width,mode,colour)`-keyed preview: the mask is
    /// canvas-sized (up to 2048×1088) and a move event lands every frame.
    preview: RefCell<Option<(PreviewKey, OverlayOp)>>,
}

impl ArcTool {
    pub fn new(kind: ArcKind) -> Self {
        Self {
            kind,
            start: 0.0,
            end: 180.0,
            width: 2,
            chord_mode: ChordMode::Fill,
            draw: None,
            canvas: None,
            preview: RefCell::new(None),
        }
    }

    pub fn arc() -> Self {
        Self::new(ArcKind::Arc)
    }

    pub fn chord() -> Self {
        Self::new(ArcKind::Chord)
    }

    fn label(&self) -> &'static str {
        match self.kind {
            ArcKind::Arc => "호",
            ArcKind::Chord => "현",
        }
    }

    pub fn start(&self) -> f64 {
        self.start
    }

    /// `panel_arc_options.js`: finite values only, unbounded.
    pub fn set_start(&mut self, v: f64) -> bool {
        if !v.is_finite() {
            return false;
        }
        let changed = self.start != v;
        self.start = v;
        changed
    }

    pub fn end(&self) -> f64 {
        self.end
    }

    /// `panel_arc_options.js`: finite values only, unbounded.
    pub fn set_end(&mut self, v: f64) -> bool {
        if !v.is_finite() {
            return false;
        }
        let changed = self.end != v;
        self.end = v;
        changed
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    /// `panel_arc_options.js`: integers clamp to 1..64.
    pub fn set_width(&mut self, v: i32) -> bool {
        let clamped = v.clamp(1, 64);
        let changed = self.width != clamped;
        self.width = clamped;
        changed
    }

    pub fn chord_mode(&self) -> ChordMode {
        self.chord_mode
    }

    pub fn set_chord_mode(&mut self, mode: ChordMode) {
        self.chord_mode = mode;
    }

    /// JS `get fill` / `set fill`.
    pub fn fill(&self) -> bool {
        self.chord_mode == ChordMode::Fill
    }

    pub fn set_fill(&mut self, value: bool) {
        if value {
            self.chord_mode = ChordMode::Fill;
        } else if self.chord_mode == ChordMode::Fill {
            self.chord_mode = ChordMode::Outline;
        }
    }

    /// JS `get outline` / `set outline`.
    pub fn outline(&self) -> bool {
        self.chord_mode == ChordMode::Outline
    }

    pub fn set_outline(&mut self, value: bool) {
        if value {
            self.chord_mode = ChordMode::Outline;
        } else if self.chord_mode == ChordMode::Outline {
            self.chord_mode = ChordMode::Fill;
        }
    }

    pub fn is_drawing(&self) -> bool {
        self.draw.is_some()
    }
}

impl Tool for ArcTool {
    fn id(&self) -> &'static str {
        match self.kind {
            ArcKind::Arc => "arc",
            ArcKind::Chord => "chord",
        }
    }

    fn cursor(&self) -> CursorKind {
        CursorKind::Crosshair
    }

    fn pointer_down(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if ev.button != Button::Left {
            return;
        }
        let Some(doc) = cx.session.doc() else {
            return;
        };
        let settings = cx.session.settings();
        self.draw = Some(Draw {
            x0: ev.x,
            y0: ev.y,
            x1: ev.x,
            y1: ev.y,
            layer_id: doc.active_layer_id.clone(),
            fill_color: settings.primary_color,
            outline_color: settings.secondary_color,
            start: self.start,
            end: self.end,
            width: self.width,
            chord_mode: self.chord_mode,
        });
        self.canvas = Some((doc.canvas.width_px, doc.canvas.height_px));
        cx.request_render();
    }

    fn pointer_move(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        let Some(draw) = self.draw.as_mut() else {
            return;
        };
        draw.x1 = ev.x;
        draw.y1 = ev.y;
        cx.request_render();
    }

    fn pointer_up(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        let Some(draw) = self.draw.take() else {
            return;
        };
        self.preview.borrow_mut().take();
        cx.request_render();
        let (x1, y1) = (ev.x, ev.y);
        // Either axis equal means no box — the JS checks `||`, not `&&`.
        if x1 == draw.x0 || y1 == draw.y0 {
            return;
        }
        let Some(doc) = cx.session.doc() else {
            return;
        };
        let (w, h) = (doc.canvas.width_px as usize, doc.canvas.height_px as usize);
        let layer_id = draw.layer_id.clone();
        let label = self.label();
        let mask = match mask_for(self.kind, &draw, x1, y1, w, h) {
            Ok(m) => m,
            Err(e) => {
                cx.report(e);
                return;
            }
        };
        if cx
            .session
            .stroke_begin(EditOptions::on_layer(&layer_id).labeled(label))
            .is_err()
        {
            // Locked/hidden/gone: the engine already emitted the status, and
            // the JS threw here instead of painting.
            return;
        }
        let (mask, packed) = mask;
        let _ = cx.session.stroke_with(|wr| {
            for (i, cell) in mask.data.iter().enumerate() {
                if *cell == 1 {
                    wr.set((i % w) as u32, (i / w) as u32, packed);
                }
            }
        });
        // Commit failure (layer deleted mid-drag) must not break input.
        let _ = cx.session.stroke_commit(Some(label));
    }

    fn cancel(&mut self, cx: &mut ToolCx) {
        self.draw = None;
        self.preview.borrow_mut().take();
        cx.request_render();
    }

    fn deactivate(&mut self, cx: &mut ToolCx) {
        self.cancel(cx);
    }

    fn overlay(&self, _info: &OverlayInfo, out: &mut Vec<OverlayOp>) {
        let Some(draw) = self.draw.as_ref() else {
            return;
        };
        let Some((cw, ch)) = self.canvas else {
            return;
        };
        let packed = match pack_of(
            if self.kind == ArcKind::Arc || draw.chord_mode == ChordMode::Fill {
                &draw.fill_color
            } else {
                &draw.outline_color
            },
        ) {
            Ok(p) => p,
            Err(_) => return,
        };
        let key = PreviewKey {
            x0: draw.x0,
            y0: draw.y0,
            x1: draw.x1,
            y1: draw.y1,
            start: draw.start.to_bits(),
            end: draw.end.to_bits(),
            width: draw.width,
            outline: draw.chord_mode == ChordMode::Outline,
            packed,
        };
        if let Some((old_key, op)) = self.preview.borrow().as_ref()
            && *old_key == key
        {
            out.push(op.clone());
            return;
        }
        let op = match mask_for(self.kind, draw, draw.x1, draw.y1, cw as usize, ch as usize) {
            Ok((mask, _)) => blit_preview(&mask, packed),
            Err(_) => None,
        };
        if let Some(op) = op {
            out.push(op.clone());
            *self.preview.borrow_mut() = Some((key, op));
        }
    }
}
