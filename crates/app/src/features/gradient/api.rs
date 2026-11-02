//! `Tool` implementation: pending box, pointer/keyboard, events, and overlay.

use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::model::event::EventDetail;
use pixel_engine::model::{LayerChangeReason, ModelEvent};
use pixel_engine::raster::gradient::radial_gradient_mask;
use pixel_engine::raster::shape_raster::ShapeKind;
use pixel_engine::view::canvas_to_screen;

use crate::features::shape::overlay::boxed_frame_overlay;
use crate::features::shape::{Bbox, PendingGeom, hit_handle, inside_shape, move_geom};
use crate::input::{Button, KeyEvent, ToolEvent};
use crate::tools::api::{CursorKind, OverlayInfo, Tool, ToolCx};
use crate::tools::overlay::OverlayOp;

use super::preview::blit_preview;
use super::state::{
    Center, EdgeMode, GradientTool, Mode, Op, PendingGradient, PreviewKey, circle_of, square_geom,
};

impl Tool for GradientTool {
    fn gradient_tool_mut(&mut self) -> Option<&mut crate::features::gradient::GradientTool> {
        Some(self)
    }

    fn id(&self) -> &'static str {
        "radial_gradient"
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

    fn set_pending_box(&mut self, box_: [i32; 4], cx: &mut ToolCx) -> EngineResult<()> {
        if box_[2] < 1 || box_[3] < 1 {
            return Err(DrawToolError::out_of_range("gradient box needs w,h >= 1"));
        }
        if cx.session.doc().is_none() {
            return Err(DrawToolError::invalid_state("no document loaded"));
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
            self.pending = Some(PendingGradient {
                layer_id,
                primary: settings.primary_color,
                secondary: settings.secondary_color,
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
        self.center = None;
        self.op = None;
        self.mode = Mode::Pending;
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
                let flags = Self::flags(ev, cx);
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
        self.center = Some(Center {
            x: ev.x,
            y: ev.y,
            x1: ev.x,
            y1: ev.y,
            layer_id: doc.active_layer_id.clone(),
            primary: settings.primary_color,
            secondary: settings.secondary_color,
        });
        self.canvas = Some((doc.canvas.width_px, doc.canvas.height_px));
        self.mode = Mode::Drawing;
        self.preview.borrow_mut().take();
        cx.request_render();
    }

    fn pointer_move(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        match self.mode {
            Mode::Drawing => {
                if let Some(center) = self.center.as_mut() {
                    center.x1 = ev.x;
                    center.y1 = ev.y;
                    self.preview.borrow_mut().take();
                    cx.request_render();
                }
            }
            Mode::Resizing => {
                let flags = Self::flags(ev, cx);
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
            && let Some(center) = self.center.take()
        {
            self.preview.borrow_mut().take();
            cx.request_render();
            let radius = Self::up_radius(&center, ev);
            if radius < 1.0 {
                self.mode = if self.pending.is_some() {
                    Mode::Pending
                } else {
                    Mode::Idle
                };
                return;
            }
            self.pending = Some(PendingGradient {
                layer_id: center.layer_id,
                primary: center.primary,
                secondary: center.secondary,
                geom: square_geom(center.x, center.y, radius.round() as i32),
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
        // Which geometry is live, and where its circle sits.
        let (key, cx_i, cy_i, radius, primary, secondary) =
            if let Some(center) = self.center.as_ref() {
                let radius = Self::drag_radius(center);
                if radius < 1.0 {
                    return;
                }
                (
                    PreviewKey {
                        x: center.x,
                        y: center.y,
                        x1: center.x1,
                        y1: center.y1,
                        inner_radius: self.inner_radius.to_bits(),
                        edge_solid: self.edge_mode == EdgeMode::Solid,
                        primary: center.primary.clone(),
                        secondary: center.secondary.clone(),
                    },
                    center.x,
                    center.y,
                    radius,
                    center.primary.clone(),
                    center.secondary.clone(),
                )
            } else if let Some(pending) = self.pending.as_ref() {
                let Some((c_x, c_y, radius)) = circle_of(&pending.geom) else {
                    return;
                };
                if radius < 1.0 {
                    return;
                }
                let end = (c_x + radius.round() as i32, c_y);
                (
                    PreviewKey {
                        x: c_x,
                        y: c_y,
                        x1: end.0,
                        y1: end.1,
                        inner_radius: self.inner_radius.to_bits(),
                        edge_solid: self.edge_mode == EdgeMode::Solid,
                        primary: pending.primary.clone(),
                        secondary: pending.secondary.clone(),
                    },
                    c_x,
                    c_y,
                    radius,
                    pending.primary.clone(),
                    pending.secondary.clone(),
                )
            } else {
                return;
            };
        let Some((cw, ch)) = self.canvas else {
            return;
        };
        let cached = self
            .preview
            .borrow()
            .as_ref()
            .and_then(|(old_key, op)| (*old_key == key).then(|| op.clone()));
        if let Some(op) = cached {
            out.push(op);
        } else {
            let op = Self::gradient_for(
                f64::from(cx_i),
                f64::from(cy_i),
                radius,
                &primary,
                &secondary,
                self.inner_radius,
                self.edge_mode,
                cw as usize,
                ch as usize,
            )
            .ok()
            .and_then(|g| radial_gradient_mask(&g).ok())
            .and_then(|m| blit_preview(m.w, m.h, &m.data));
            if let Some(op) = op {
                out.push(op.clone());
                *self.preview.borrow_mut() = Some((key, op));
            }
        }
        // The dashed radius ring: `setLineDash([4,3])` verbatim (no dpr
        // factor) and `rgba(255,255,255,0.9)` ≈ 230, like the JS.
        let dev = canvas_to_screen(info.view, f64::from(cx_i), f64::from(cy_i));
        out.push(OverlayOp::Circle {
            cx: dev.x * info.dpr,
            cy: dev.y * info.dpr,
            r: radius * info.view.zoom() * info.dpr,
            rgba: [255, 255, 255, 230],
            width: 1.0,
            dash: Some((4.0, 3.0)),
        });
        if let Some(pending) = self.pending.as_ref() {
            boxed_frame_overlay(&pending.geom, info, out, true);
        }
    }
}
