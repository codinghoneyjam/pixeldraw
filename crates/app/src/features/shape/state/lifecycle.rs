//! The [`Tool`](crate::tools::api::Tool) impl — identity and event dispatch.

use pixel_engine::model::ModelEvent;
use pixel_engine::model::event::EventDetail;

use crate::input::{Button, HoverEvent, KeyEvent, ToolEvent};
use crate::tools::api::{CursorKind, OverlayInfo, Tool, ToolCx};
use crate::tools::overlay::OverlayOp;

use super::super::keys::handle_key;
use super::super::overlay::shape_overlay;
use super::super::pending::finish_placing;
use super::core::{Mode, ShapeTool};

impl Tool for ShapeTool {
    fn id(&self) -> &'static str {
        self.kind.as_str()
    }

    fn cursor(&self) -> CursorKind {
        CursorKind::Crosshair
    }

    fn activate(&mut self, cx: &mut ToolCx) {
        cx.request_render();
    }

    fn deactivate(&mut self, cx: &mut ToolCx) {
        if self.mode == Mode::Placing
            && let Some(place) = self.place.clone()
        {
            // Leaving keeps finished work, like pending: 3+ points finish.
            if place.points.len() >= 3 {
                let _ = finish_placing(self, cx);
            } else {
                self.place = None;
                self.hover_pt = None;
                self.mode = if self.pending.is_some() {
                    Mode::Pending
                } else {
                    Mode::Idle
                };
            }
        }
        if self.has_pending() {
            let _ = self.commit(cx);
        }
        self.draw = None;
        self.op = None;
        self.place = None;
        self.hover_pt = None;
        self.mode = Mode::Idle;
    }

    fn accepts_button(&self, button: Button) -> bool {
        button == Button::Left
    }

    fn pointer_down(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        self.pointer_down(ev, cx);
    }

    fn pointer_move(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        self.pointer_move(ev, cx);
    }

    fn pointer_up(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        self.pointer_up(ev, cx);
    }

    fn cancel(&mut self, cx: &mut ToolCx) {
        self.cancel(cx);
    }

    fn hover(&mut self, _ev: Option<&HoverEvent>, _cx: &mut ToolCx) {}

    fn key_down(&mut self, ev: &KeyEvent, cx: &mut ToolCx) -> bool {
        handle_key(self, ev, cx)
    }

    fn has_pending(&self) -> bool {
        self.has_pending()
    }

    fn discard_pending(&mut self, cx: &mut ToolCx) {
        self.discard_pending(cx);
    }

    fn on_model_event(&mut self, ev: &ModelEvent, cx: &mut ToolCx) {
        match &ev.detail {
            EventDetail::DocumentReplaced => self.discard_pending(cx),
            EventDetail::Layers { reason, .. } => self.layers_changed(*reason, cx),
            EventDetail::Settings { key, value } => self.settings_changed(*key, value, cx),
            _ => {}
        }
    }

    fn overlay(&self, info: &OverlayInfo, out: &mut Vec<OverlayOp>) {
        shape_overlay(self, info, out);
    }
}
