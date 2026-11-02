//! `Tool` implementation: pending box, pointer/keyboard, events, and overlay.

use pixel_engine::model::event::EventDetail;
use pixel_engine::model::{LayerChangeReason, ModelEvent};
use pixel_engine::raster::shape_raster::ShapeKind;

use crate::features::shape::overlay::boxed_frame_overlay;
use crate::features::shape::{Bbox, PendingGeom, drag_geom, hit_handle, inside_shape, move_geom};
use crate::input::{Button, KeyEvent, ToolEvent};
use crate::tools::api::{CursorKind, OverlayInfo, Tool, ToolCx};
use crate::tools::overlay::OverlayOp;

use super::preview::{PreviewKey, blit_preview, mask_for, pack_of};
use super::state::{ArcKind, ArcTool, ChordMode, Draw, Mode, Op, PendingArc};

impl Tool for ArcTool {
    fn arc_tool_mut(&mut self) -> Option<&mut crate::features::arc::ArcTool> {
        Some(self)
    }

    fn id(&self) -> &'static str {
        match self.kind {
            ArcKind::Arc => "arc",
            ArcKind::Chord => "chord",
        }
    }

    fn cursor(&self) -> CursorKind {
        CursorKind::Crosshair
    }

    fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    fn pending_box(&self) -> Option<[i32; 4]> {
        self.pending
            .as_ref()?
            .geom
            .bbox
            .map(|b| [b.x, b.y, b.w, b.h])
    }

    fn pending_box_resizable(&self) -> bool {
        true
    }

    fn set_pending_box(
        &mut self,
        box_: [i32; 4],
        cx: &mut ToolCx,
    ) -> pixel_engine::errors::EngineResult<()> {
        if box_[2] < 1 || box_[3] < 1 {
            return Err(pixel_engine::errors::DrawToolError::out_of_range(
                "arc box needs w,h >= 1",
            ));
        }
        if cx.session.doc().is_none() {
            return Err(pixel_engine::errors::DrawToolError::invalid_state(
                "no document loaded",
            ));
        }
        let doc = cx.session.doc().expect("checked");
        self.canvas = Some((doc.canvas.width_px, doc.canvas.height_px));
        let layer_id = doc.active_layer_id.clone();
        if let Some(p) = self.pending.as_mut() {
            p.geom.bbox = Some(Bbox {
                x: box_[0],
                y: box_[1],
                w: box_[2],
                h: box_[3],
            });
        } else {
            let settings = cx.session.settings();
            self.pending = Some(PendingArc {
                layer_id,
                fill_color: settings.primary_color,
                outline_color: settings.secondary_color,
                start: self.start,
                end: self.end,
                width: self.width,
                chord_mode: self.chord_mode,
                geom: PendingGeom {
                    bbox: Some(Bbox {
                        x: box_[0],
                        y: box_[1],
                        w: box_[2],
                        h: box_[3],
                    }),
                    p0: None,
                    p1: None,
                    radius: 0,
                    points: vec![],
                    preset: None,
                },
            });
        }
        self.draw = None;
        self.op = None;
        self.mode = Mode::Pending;
        self.preview.borrow_mut().take();
        cx.request_render();
        Ok(())
    }

    fn commit_pending(&mut self, cx: &mut ToolCx) -> pixel_engine::errors::EngineResult<bool> {
        self.commit(cx)
    }

    fn discard_pending(&mut self, cx: &mut ToolCx) {
        self.discard(cx);
    }

    fn pointer_down(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if ev.button != Button::Left {
            return;
        }
        if cx.session.doc().is_none() || !matches!(self.mode, Mode::Idle | Mode::Pending) {
            return;
        }
        let p = (ev.x, ev.y);
        if self.mode == Mode::Pending && self.pending.is_some() {
            let geom = self.pending.as_ref().expect("checked").geom.clone();
            let tol = 6.0 / cx.zoom();
            if let Some(h) = hit_handle(ShapeKind::Rect, &geom, ev.fx, ev.fy, tol) {
                self.op = Some(Op::Resize {
                    handle: h.id,
                    orig: self.pending.clone().expect("checked"),
                });
                self.mode = Mode::Resizing;
                let flags = self.flags(ev, cx);
                self.resize_to(p, flags, ev.shift, cx);
                return;
            }
            let grab = (cx.session.settings().pen_size as f64 / 2.0).max(4.0 / cx.zoom());
            if inside_shape(ShapeKind::Rect, &geom, ev.fx, ev.fy, grab) {
                self.op = Some(Op::Move {
                    ox: p.0,
                    oy: p.1,
                    orig: self.pending.clone().expect("checked"),
                });
                self.mode = Mode::Moving;
                return;
            }
            let _ = self.commit(cx);
        }
        let doc = cx.session.doc().expect("checked");
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
            shift: ev.shift,
            alt: ev.alt,
        });
        self.canvas = Some((doc.canvas.width_px, doc.canvas.height_px));
        self.mode = Mode::Drawing;
        self.preview.borrow_mut().take();
        cx.request_render();
    }

    fn pointer_move(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        match self.mode {
            Mode::Drawing => {
                if let Some(draw) = self.draw.as_mut() {
                    draw.x1 = ev.x;
                    draw.y1 = ev.y;
                    draw.shift = ev.shift;
                    draw.alt = ev.alt;
                    self.preview.borrow_mut().take();
                    cx.request_render();
                }
            }
            Mode::Resizing => {
                let flags = self.flags(ev, cx);
                self.resize_to((ev.x, ev.y), flags, ev.shift, cx);
            }
            Mode::Moving => {
                if let Some(Op::Move { ox, oy, orig }) = self.op.clone() {
                    let snap = cx.session.settings().snap_unit;
                    let geom = move_geom(ShapeKind::Rect, &orig.geom, ev.x - ox, ev.y - oy, snap);
                    if let Some(pending) = self.pending.as_mut() {
                        pending.geom = geom;
                    }
                    self.preview.borrow_mut().take();
                    cx.request_render();
                }
            }
            _ => {}
        }
    }

    fn pointer_up(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if self.mode == Mode::Drawing
            && let Some(draw) = self.draw.take()
        {
            self.preview.borrow_mut().take();
            cx.request_render();
            // Either axis equal means no box — the JS checks `||`, not `&&`.
            if ev.x == draw.x0 || ev.y == draw.y0 {
                self.mode = if self.pending.is_some() {
                    Mode::Pending
                } else {
                    Mode::Idle
                };
                return;
            }
            let flags = self.flags(ev, cx);
            let geom = drag_geom(ShapeKind::Rect, (draw.x0, draw.y0), (ev.x, ev.y), flags, 0);
            self.pending = Some(PendingArc {
                layer_id: draw.layer_id,
                fill_color: draw.fill_color,
                outline_color: draw.outline_color,
                start: draw.start,
                end: draw.end,
                width: draw.width,
                chord_mode: draw.chord_mode,
                geom,
            });
            self.mode = Mode::Pending;
        } else if matches!(self.mode, Mode::Resizing | Mode::Moving) {
            self.op = None;
            self.mode = Mode::Pending;
            self.preview.borrow_mut().take();
            cx.request_render();
        }
    }

    fn cancel(&mut self, cx: &mut ToolCx) {
        self.cancel_operation(cx);
    }

    fn deactivate(&mut self, cx: &mut ToolCx) {
        let _ = self.commit(cx);
        self.discard(cx);
    }

    fn key_down(&mut self, ev: &KeyEvent, cx: &mut ToolCx) -> bool {
        match ev.key.as_str() {
            "Enter" => {
                let _ = self.commit(cx);
                true
            }
            "Escape" => {
                if matches!(self.mode, Mode::Drawing | Mode::Resizing | Mode::Moving) {
                    self.cancel_operation(cx);
                } else {
                    self.discard(cx);
                }
                true
            }
            "Delete" | "Backspace" => {
                self.discard(cx);
                true
            }
            "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" => {
                let Some(orig) = self.pending.as_ref().map(|p| p.geom.clone()) else {
                    return false;
                };
                let step = if ev.shift { 32 } else { 1 };
                let (dx, dy) = match ev.key.as_str() {
                    "ArrowLeft" => (-step, 0),
                    "ArrowRight" => (step, 0),
                    "ArrowUp" => (0, -step),
                    _ => (0, step),
                };
                let geom = move_geom(ShapeKind::Rect, &orig, dx, dy, false);
                if let Some(p) = self.pending.as_mut() {
                    p.geom = geom;
                }
                self.preview.borrow_mut().take();
                cx.request_render();
                true
            }
            _ => false,
        }
    }

    fn on_model_event(&mut self, ev: &ModelEvent, cx: &mut ToolCx) {
        match &ev.detail {
            EventDetail::DocumentReplaced => self.discard(cx),
            EventDetail::Layers { reason, layer_ids } => match reason {
                LayerChangeReason::Active => {
                    let _ = self.commit(cx);
                }
                LayerChangeReason::Remove
                    if self
                        .pending
                        .as_ref()
                        .is_some_and(|p| layer_ids.contains(&p.layer_id)) =>
                {
                    self.discard(cx);
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn overlay(&self, info: &OverlayInfo, out: &mut Vec<OverlayOp>) {
        let draw = match (&self.draw, &self.pending) {
            (Some(draw), _) => draw.clone(),
            (_, Some(pending)) => {
                let Some(d) = Draw::from_pending(pending) else {
                    return;
                };
                d
            }
            _ => return,
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
        } else {
            let op = match mask_for(self.kind, &draw, draw.x1, draw.y1, cw as usize, ch as usize) {
                Ok((mask, _)) => blit_preview(&mask, packed),
                Err(_) => None,
            };
            if let Some(op) = op {
                out.push(op.clone());
                *self.preview.borrow_mut() = Some((key, op));
            }
        }
        if self.pending.is_some()
            && let Some(pending) = self.pending.as_ref()
        {
            boxed_frame_overlay(&pending.geom, info, out, true);
        }
    }
}
