//! `Tool` implementation: pointer, keyboard, model-event, and overlay wiring.

use pixel_engine::errors::EngineResult;
use pixel_engine::model::event::EventDetail;
use pixel_engine::model::{LayerChangeReason, ModelEvent};
use pixel_engine::raster::shape_raster::ShapeKind;

use crate::features::shape::overlay::boxed_frame_overlay;
use crate::features::shape::{Bbox, PendingGeom, inside_shape};
use crate::input::{Button, KeyEvent, ToolEvent};
use crate::tools::api::{CursorKind, OverlayInfo, Tool, ToolCx};
use crate::tools::overlay::OverlayOp;

use super::state::{Mode, PreviewKey, crop_preview};
use super::tool::TextTool;

impl Tool for TextTool {
    fn text_tool_mut(&mut self) -> Option<&mut crate::features::text::TextTool> {
        Some(self)
    }

    fn id(&self) -> &'static str {
        "text"
    }

    fn cursor(&self) -> CursorKind {
        CursorKind::Text
    }

    fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    fn pending_box(&self) -> Option<[i32; 4]> {
        self.pending.as_ref().map(|p| p.bbox)
    }

    fn pending_box_resizable(&self) -> bool {
        false
    }

    fn set_pending_box(&mut self, box_: [i32; 4], cx: &mut ToolCx) -> EngineResult<()> {
        let Some(current) = self.pending.as_ref().map(|p| p.bbox) else {
            return Ok(());
        };
        // Text only translates: W/H are displayed but never change the glyphs.
        let dx = box_[0] - current[0];
        let dy = box_[1] - current[1];
        if dx == 0 && dy == 0 {
            return Ok(());
        }
        let Some(p) = self.pending.as_mut() else {
            return Ok(());
        };
        p.anchor.0 += dx;
        p.anchor.1 += dy;
        p.bbox[0] += dx;
        p.bbox[1] += dy;
        self.preview.borrow_mut().take();
        cx.request_render();
        Ok(())
    }

    fn commit_pending(&mut self, cx: &mut ToolCx) -> EngineResult<bool> {
        self.commit(cx)
    }

    fn discard_pending(&mut self, cx: &mut ToolCx) {
        self.discard(cx);
    }

    fn pointer_down(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if ev.button != Button::Left {
            return;
        }
        if cx.session.doc().is_none() {
            return;
        }
        if self.mode == Mode::Pending
            && let Some(p) = self.pending.as_ref()
        {
            let geom = PendingGeom {
                bbox: Some(Bbox {
                    x: p.bbox[0],
                    y: p.bbox[1],
                    w: p.bbox[2],
                    h: p.bbox[3],
                }),
                p0: None,
                p1: None,
                radius: 0,
                points: vec![],
                preset: None,
            };
            if inside_shape(ShapeKind::Rect, &geom, ev.fx, ev.fy, 0.0) {
                self.moving = Some((ev.x, ev.y, p.clone()));
                self.mode = Mode::Moving;
                return;
            }
            let _ = self.commit(cx);
        }
        self.place(ev, cx);
    }

    fn pointer_move(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if self.mode == Mode::Moving
            && let Some((ox, oy, _)) = self.moving.clone()
        {
            self.move_to(ev.x - ox, ev.y - oy, cx);
        }
    }

    fn pointer_up(&mut self, _ev: &ToolEvent, cx: &mut ToolCx) {
        if self.mode == Mode::Moving {
            self.moving = None;
            self.mode = Mode::Pending;
            cx.request_render();
        }
    }

    fn cancel(&mut self, cx: &mut ToolCx) {
        if self.mode == Mode::Moving
            && let Some((_, _, orig)) = self.moving.take()
        {
            self.pending = Some(orig);
            self.mode = Mode::Pending;
            self.preview.borrow_mut().take();
            cx.request_render();
        }
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
                if self.mode == Mode::Moving {
                    self.cancel(cx);
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
                let (dx, dy) = {
                    let step = if ev.shift { 32 } else { 1 };
                    match ev.key.as_str() {
                        "ArrowLeft" => (-step, 0),
                        "ArrowRight" => (step, 0),
                        "ArrowUp" => (0, -step),
                        _ => (0, step),
                    }
                };
                if let Some(p) = self.pending.as_mut() {
                    p.anchor.0 += dx;
                    p.anchor.1 += dy;
                    p.bbox[0] += dx;
                    p.bbox[1] += dy;
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
        let Some(pending) = self.pending.as_ref() else {
            return;
        };
        let key = PreviewKey {
            text: pending.text.clone(),
            font: pending.font.clone(),
            stroke_width: pending.stroke_width,
            anchor: pending.anchor,
            fill: pending.fill,
            stroke: pending.stroke,
            bbox: pending.bbox,
        };
        if let Some((old_key, op)) = self.preview.borrow().as_ref()
            && *old_key == key
        {
            out.push(op.clone());
        } else if let Ok(mask) = self.mask_for(pending)
            && let Some(op) = crop_preview(&mask, pending.bbox)
        {
            out.push(op.clone());
            *self.preview.borrow_mut() = Some((key, op));
        }
        let geom = PendingGeom {
            bbox: Some(Bbox {
                x: pending.bbox[0],
                y: pending.bbox[1],
                w: pending.bbox[2],
                h: pending.bbox[3],
            }),
            p0: None,
            p1: None,
            radius: 0,
            points: vec![],
            preset: None,
        };
        boxed_frame_overlay(&geom, info, out, false);
    }
}
