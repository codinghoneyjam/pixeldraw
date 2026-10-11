//! Shared tool API for the native app shell.

use crate::tools::overlay::OverlayOp;
use pixel_engine::{errors::DrawToolError, model::Session, view::View};

pub struct ToolCx<'a> {
    pub session: &'a mut Session,
    pub view: View,
    pub need_render: &'a mut bool,
}

impl ToolCx<'_> {
    pub fn request_render(&mut self) {
        *self.need_render = true;
    }

    pub fn report(&mut self, e: DrawToolError) {
        self.session.notify("error", e.message.clone(), e.code);
    }

    pub fn zoom(&self) -> f64 {
        self.view.zoom()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorKind {
    Default,
    Crosshair,
    Cell,
    Copy,
    Grab,
    Grabbing,
    Text,
}

pub trait Tool {
    fn id(&self) -> &'static str;
    fn cursor(&self) -> CursorKind {
        CursorKind::Default
    }
    fn activate(&mut self, _cx: &mut ToolCx) {}
    fn deactivate(&mut self, _cx: &mut ToolCx) {}
    fn pointer_down(&mut self, _ev: &crate::input::ToolEvent, _cx: &mut ToolCx) {}
    fn pointer_move(&mut self, _ev: &crate::input::ToolEvent, _cx: &mut ToolCx) {}
    fn pointer_up(&mut self, _ev: &crate::input::ToolEvent, _cx: &mut ToolCx) {}
    fn cancel(&mut self, _cx: &mut ToolCx) {}
    fn hover(&mut self, _ev: Option<&crate::input::HoverEvent>, _cx: &mut ToolCx) {}
    fn key_down(&mut self, _ev: &crate::input::KeyEvent, _cx: &mut ToolCx) -> bool {
        let _ = (_ev, _cx);
        false
    }
    fn accepts_button(&self, button: crate::input::Button) -> bool {
        button == crate::input::Button::Left
    }
    fn has_pending(&self) -> bool {
        false
    }
    fn discard_pending(&mut self, _cx: &mut ToolCx) {
        let _ = _cx;
    }
    fn on_model_event(&mut self, _ev: &pixel_engine::model::ModelEvent, _cx: &mut ToolCx) {
        let _ = _ev;
        let _ = _cx;
    }
    fn overlay(&self, _info: &OverlayInfo, _out: &mut Vec<OverlayOp>) {
        let _ = (_info, _out);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OverlayInfo {
    pub view: View,
    pub dpr: f64,
}
