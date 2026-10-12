//! Pending/draw transitions — set, discard, commit, cancel, resize, events.

use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::model::{LayerChangeReason, SettingKey, SettingValue};
use pixel_engine::raster::shape_raster::ShapeKind;

use crate::tools::api::ToolCx;

use super::super::commit::{CommitResult, commit_pending};
use super::super::geom::{DragFlags, Pt, resize_geom};
use super::super::pending::{PendingValue, parse_pending_value};
use super::core::{LIVE_KEYS, Mode, Op, PendingShape, ShapeTool};

impl ShapeTool {
    /// `setPending` — `None` discards; anything else validates like the JS
    /// (`OUT_OF_RANGE` on bad geometry, `INVALID_STATE` with no document).
    pub fn set_pending(&mut self, v: Option<PendingValue>, cx: &mut ToolCx) -> EngineResult<()> {
        if v.is_none() {
            self.discard_pending(cx);
            return Ok(());
        }
        if cx.session.doc().is_none() {
            return Err(DrawToolError::invalid_state("no document loaded"));
        }
        let s = cx.session.settings();
        let layer_id = self
            .pending
            .as_ref()
            .map(|p| p.layer_id.clone())
            .unwrap_or_else(|| cx.session.doc().expect("checked").active_layer_id.clone());
        // An existing pending keeps its captured colour — nudging the bbox
        // after a colour change must not recolour it.
        let color = self
            .pending
            .as_ref()
            .map(|p| p.color.clone())
            .unwrap_or(s.primary_color);
        let geom = parse_pending_value(self.kind, v.expect("checked"), s.shape_radius)?;
        self.pending = Some(PendingShape {
            layer_id,
            color,
            geom,
        });
        self.draw = None;
        self.op = None;
        self.place = None;
        self.hover_pt = None;
        self.mode = Mode::Pending;
        self.emit_changed(cx);
        cx.request_render();
        Ok(())
    }

    pub fn discard_pending(&mut self, cx: &mut ToolCx) {
        self.pending = None;
        self.draw = None;
        self.op = None;
        self.place = None;
        self.hover_pt = None;
        self.mode = Mode::Idle;
        self.emit_changed(cx);
        cx.request_render();
    }

    /// `commit` — one history entry, or `false` with no pending. Layer errors
    /// discard the pending shape; anything else propagates (JS rethrew).
    pub fn commit(&mut self, cx: &mut ToolCx) -> EngineResult<bool> {
        match commit_pending(self.kind, self.label(), self.pending.clone(), cx) {
            Ok(CommitResult::Committed) => {
                self.pending = None;
                self.op = None;
                self.mode = Mode::Idle;
                self.emit_changed(cx);
                cx.request_render();
                Ok(true)
            }
            Ok(CommitResult::Discarded) => {
                self.discard_pending(cx);
                Ok(false)
            }
            Ok(CommitResult::Empty) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn cancel(&mut self, cx: &mut ToolCx) {
        match self.mode {
            // Placing never opened an edit — nothing to roll back.
            Mode::Placing => {
                self.place = None;
                self.hover_pt = None;
                self.mode = if self.pending.is_some() {
                    Mode::Pending
                } else {
                    Mode::Idle
                };
                self.emit_changed(cx);
                cx.request_render();
            }
            Mode::Drawing => {
                self.draw = None;
                self.mode = if self.pending.is_some() {
                    Mode::Pending
                } else {
                    Mode::Idle
                };
                self.emit_changed(cx);
                cx.request_render();
            }
            Mode::Resizing | Mode::Moving => {
                if let Some(op) = self.op.take() {
                    let orig = match op {
                        Op::Resize { orig, .. } => orig,
                        Op::Move { orig, .. } => orig,
                    };
                    self.pending = Some(orig);
                }
                self.mode = Mode::Pending;
                self.emit_changed(cx);
                cx.request_render();
            }
            Mode::Idle | Mode::Pending => {}
        }
    }

    pub(super) fn resize_to(&mut self, p: Pt, flags: DragFlags, shift_snap: bool, cx: &mut ToolCx) {
        if let Some(Op::Resize { handle, end, orig }) = self.op.clone() {
            let geom = resize_geom(self.kind, &orig.geom, &handle, end, p, flags, shift_snap);
            self.pending = Some(PendingShape {
                layer_id: orig.layer_id,
                color: orig.color,
                geom,
            });
            self.emit_changed(cx);
            cx.request_render();
        }
    }

    pub(super) fn settings_changed(
        &mut self,
        key: SettingKey,
        value: &SettingValue,
        cx: &mut ToolCx,
    ) {
        if !LIVE_KEYS.contains(&key) {
            return;
        }
        if self.kind == ShapeKind::Rrect
            && let Some(p) = self.pending.as_mut()
            && key == SettingKey::ShapeRadius
            && let SettingValue::Int(n) = value
        {
            p.geom.radius = *n as i32;
        }
        if self.pending.is_some() || self.draw.is_some() {
            self.emit_changed(cx);
            cx.request_render();
        }
    }

    pub(super) fn layers_changed(&mut self, reason: LayerChangeReason, cx: &mut ToolCx) {
        if !self.has_pending() {
            return;
        }
        if reason == LayerChangeReason::Active {
            let _ = self.commit(cx);
        } else if reason == LayerChangeReason::Remove {
            // The JS also watched `"replace"`; the engine has no such reason
            // (Add/Move/Prop cover the cases), so only a removed layer drops
            // a pending that points at it.
            let gone = self
                .pending
                .as_ref()
                .map(|p| {
                    cx.session
                        .doc()
                        .and_then(|d| d.get_layer(&p.layer_id).ok())
                        .is_none()
                })
                .unwrap_or(false);
            if gone {
                self.discard_pending(cx);
            }
        }
    }
}
