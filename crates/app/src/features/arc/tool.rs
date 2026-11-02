//! Arc/chord tool state transitions: options, drag/resize/move, and commit.

use std::cell::RefCell;

use pixel_engine::model::edit_session::EditOptions;
use pixel_engine::raster::shape_raster::ShapeKind;

use crate::features::shape::{DragFlags, Pt, resize_geom};
use crate::input::ToolEvent;
use crate::tools::api::ToolCx;

use super::preview::mask_for;
use super::state::{ArcKind, ArcTool, ChordMode, Draw, Mode, Op};

impl ArcTool {
    pub fn new(kind: ArcKind) -> Self {
        Self {
            kind,
            start: 0.0,
            end: 180.0,
            width: 2,
            chord_mode: ChordMode::Fill,
            draw: None,
            pending: None,
            op: None,
            mode: Mode::Idle,
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

    /// Angles are degrees in `0..=360`; the option bar clamps too, but direct
    /// setters (tests, future restore) must not create out-of-range previews.
    pub fn set_start(&mut self, v: f64) -> bool {
        if !v.is_finite() {
            return false;
        }
        let v = v.clamp(0.0, 360.0);
        let changed = self.start != v;
        self.start = v;
        if let Some(p) = self.pending.as_mut() {
            p.start = v;
        }
        self.preview.borrow_mut().take();
        changed
    }

    pub fn end(&self) -> f64 {
        self.end
    }

    /// Angles are degrees in `0..=360`; see [`Self::set_start`].
    pub fn set_end(&mut self, v: f64) -> bool {
        if !v.is_finite() {
            return false;
        }
        let v = v.clamp(0.0, 360.0);
        let changed = self.end != v;
        self.end = v;
        if let Some(p) = self.pending.as_mut() {
            p.end = v;
        }
        self.preview.borrow_mut().take();
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
        if let Some(p) = self.pending.as_mut() {
            p.width = clamped;
        }
        self.preview.borrow_mut().take();
        changed
    }

    pub fn chord_mode(&self) -> ChordMode {
        self.chord_mode
    }

    pub fn set_chord_mode(&mut self, mode: ChordMode) {
        self.chord_mode = mode;
        if let Some(p) = self.pending.as_mut() {
            p.chord_mode = mode;
        }
        self.preview.borrow_mut().take();
    }

    /// JS `get fill` / `set fill`.
    pub fn fill(&self) -> bool {
        self.chord_mode == ChordMode::Fill
    }

    pub fn set_fill(&mut self, value: bool) {
        if value {
            self.set_chord_mode(ChordMode::Fill);
        } else if self.chord_mode == ChordMode::Fill {
            self.set_chord_mode(ChordMode::Outline);
        }
    }

    /// JS `get outline` / `set outline`.
    pub fn outline(&self) -> bool {
        self.chord_mode == ChordMode::Outline
    }

    pub fn set_outline(&mut self, value: bool) {
        if value {
            self.set_chord_mode(ChordMode::Outline);
        } else if self.chord_mode == ChordMode::Outline {
            self.set_chord_mode(ChordMode::Fill);
        }
    }

    pub fn is_drawing(&self) -> bool {
        self.draw.is_some()
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn flags(&self, ev: &ToolEvent, cx: &ToolCx) -> DragFlags {
        let s = cx.session.settings();
        DragFlags {
            shift: ev.shift,
            lock: ev.shift || s.shape_lock_aspect,
            center: ev.alt,
            snap: s.snap_unit,
        }
    }

    /// Discard any transient draw/transform and clear the confirmation box.
    pub fn discard(&mut self, cx: &mut ToolCx) {
        self.draw = None;
        self.pending = None;
        self.op = None;
        self.mode = Mode::Idle;
        self.preview.borrow_mut().take();
        cx.request_render();
    }

    /// Transient cancel: an Esc during a drag restores the pending box.
    pub(super) fn cancel_operation(&mut self, cx: &mut ToolCx) {
        match self.mode {
            Mode::Drawing => {
                self.draw = None;
                self.mode = if self.pending.is_some() {
                    Mode::Pending
                } else {
                    Mode::Idle
                };
            }
            Mode::Resizing | Mode::Moving => {
                if let Some(op) = self.op.take() {
                    let orig = match op {
                        Op::Resize { orig, .. } | Op::Move { orig, .. } => orig,
                    };
                    self.pending = Some(orig);
                    self.mode = Mode::Pending;
                }
            }
            _ => {}
        }
        self.preview.borrow_mut().take();
        cx.request_render();
    }

    pub(super) fn resize_to(&mut self, p: Pt, flags: DragFlags, shift: bool, cx: &mut ToolCx) {
        let Some(Op::Resize { handle, orig }) = self.op.as_ref() else {
            return;
        };
        let geom = resize_geom(ShapeKind::Rect, &orig.geom, handle, None, p, flags, shift);
        if let Some(pending) = self.pending.as_mut() {
            pending.geom = geom;
        }
        self.preview.borrow_mut().take();
        cx.request_render();
    }

    /// Bake the pending arc as one history entry.
    pub fn commit(&mut self, cx: &mut ToolCx) -> pixel_engine::errors::EngineResult<bool> {
        let Some(pending) = self.pending.clone() else {
            self.mode = Mode::Idle;
            return Ok(false);
        };
        let Some(draw) = Draw::from_pending(&pending) else {
            self.discard(cx);
            return Ok(false);
        };
        let Some((cw, ch)) = self.canvas.or_else(|| {
            cx.session
                .doc()
                .map(|d| (d.canvas.width_px, d.canvas.height_px))
        }) else {
            return Ok(false);
        };
        let label = self.label();
        let mask = mask_for(self.kind, &draw, draw.x1, draw.y1, cw as usize, ch as usize)?;
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
        let (mask, packed) = mask;
        let result = cx.session.stroke_with(|wr| {
            for (i, cell) in mask.data.iter().enumerate() {
                if *cell == 1 {
                    wr.set((i % cw as usize) as u32, (i / cw as usize) as u32, packed);
                }
            }
        });
        if let Err(e) = result {
            cx.session.stroke_cancel();
            return Err(e);
        }
        cx.session.stroke_commit(Some(label))?;
        self.pending = None;
        self.op = None;
        self.mode = Mode::Idle;
        self.preview.borrow_mut().take();
        cx.request_render();
        Ok(true)
    }
}
