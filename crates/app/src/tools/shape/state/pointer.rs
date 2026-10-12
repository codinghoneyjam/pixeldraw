//! Pointer verbs — down/move/up over the pending machine.

use pixel_engine::raster::shape_raster::ShapeKind;

use crate::input::{Button, ToolEvent};
use crate::tools::api::ToolCx;

use super::super::geom::{
    DragFlags, drag_geom, hit_handle, inside_shape, move_geom, snap_place_point,
};
use super::super::pending::place_point;
use super::core::{Draw, Mode, Op, PendingShape, Place, ShapeTool};

impl ShapeTool {
    pub(super) fn pointer_down(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if ev.button != Button::Left {
            return;
        }
        if cx.session.doc().is_none()
            || !matches!(self.mode, Mode::Idle | Mode::Pending | Mode::Placing)
        {
            return;
        }
        self.refresh_live(cx);
        let p = (ev.x, ev.y);
        if self.mode == Mode::Placing && self.place.is_some() {
            place_point(self, ev, p, self.zoom(cx), cx);
            return;
        }
        if self.mode == Mode::Pending && self.pending.is_some() {
            let geom = self.pending.as_ref().expect("checked").geom.clone();
            let tol = 6.0 / self.zoom(cx);
            if let Some(h) = hit_handle(self.kind, &geom, ev.fx, ev.fy, tol) {
                self.op = Some(Op::Resize {
                    handle: h.id,
                    end: h.end,
                    orig: self.pending.clone().expect("checked"),
                });
                self.mode = Mode::Resizing;
                let flags = self.flags(ev, cx);
                self.resize_to(p, flags, ev.shift, cx);
                return;
            }
            let grab = (cx.session.settings().pen_size as f64 / 2.0).max(4.0 / self.zoom(cx));
            if inside_shape(self.kind, &geom, ev.fx, ev.fy, grab) {
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
        if self.kind == ShapeKind::Polygon {
            let layer_id = cx.session.doc().expect("checked").active_layer_id.clone();
            let snap = cx.session.settings().snap_unit;
            self.place = Some(Place {
                points: vec![snap_place_point(p, snap)],
                layer_id,
            });
            self.hover_pt = None;
            self.mode = Mode::Placing;
            self.emit_changed(cx);
            cx.request_render();
            return;
        }
        let layer_id = cx.session.doc().expect("checked").active_layer_id.clone();
        self.draw = Some(Draw {
            p0: p,
            cur: None,
            layer_id,
            shift: ev.shift,
            alt: ev.alt,
        });
        self.mode = Mode::Drawing;
        cx.request_render();
    }

    pub(super) fn pointer_move(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        self.refresh_live(cx);
        match self.mode {
            Mode::Drawing => {
                if let Some(draw) = self.draw.as_mut() {
                    draw.cur = Some((ev.x, ev.y));
                    draw.shift = ev.shift;
                    draw.alt = ev.alt;
                    cx.request_render();
                }
            }
            Mode::Placing => {
                if self.place.is_some() {
                    self.hover_pt = Some((ev.x, ev.y));
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
                    let geom = move_geom(self.kind, &orig.geom, ev.x - ox, ev.y - oy, snap);
                    self.pending = Some(PendingShape {
                        layer_id: orig.layer_id,
                        color: orig.color,
                        geom,
                    });
                    self.emit_changed(cx);
                    cx.request_render();
                }
            }
            Mode::Idle | Mode::Pending => {}
        }
    }

    pub(super) fn pointer_up(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        self.refresh_live(cx);
        if self.mode == Mode::Drawing
            && let Some(draw) = self.draw.clone()
        {
            let idle = if self.pending.is_some() {
                Mode::Pending
            } else {
                Mode::Idle
            };
            if ev.x == draw.p0.0 && ev.y == draw.p0.1 {
                self.draw = None;
                self.mode = idle;
            } else {
                let s = cx.session.settings();
                let flags = DragFlags {
                    shift: ev.shift,
                    lock: ev.shift || s.shape_lock_aspect,
                    center: ev.alt,
                    snap: s.snap_unit,
                };
                let geom = drag_geom(
                    self.kind,
                    draw.p0,
                    (ev.x, ev.y),
                    flags,
                    s.shape_radius as i32,
                );
                self.pending = Some(PendingShape {
                    layer_id: draw.layer_id,
                    color: s.primary_color,
                    geom,
                });
                self.draw = None;
                self.mode = Mode::Pending;
            }
            self.emit_changed(cx);
            cx.request_render();
        } else if matches!(self.mode, Mode::Resizing | Mode::Moving) && self.op.is_some() {
            self.op = None;
            self.mode = Mode::Pending;
            self.emit_changed(cx);
            cx.request_render();
        }
    }

    fn flags(&self, ev: &ToolEvent, cx: &ToolCx) -> DragFlags {
        let s = cx.session.settings();
        DragFlags {
            shift: ev.shift,
            lock: ev.shift || s.shape_lock_aspect,
            center: ev.alt,
            snap: s.snap_unit,
        }
    }
}
