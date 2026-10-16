//! Session glue — the W8 frame-loop state behind the egui shell.
//!
//! The engine bus cannot call back into tools or the renderer (`'static`
//! closures), so each frame drains a [`ModelEvent`] queue into the three
//! consumers, in order: [`CompCache`], [`ToolManager`], and the status bar.
//! [`AppState::new`] also owns the boot document, so the shell never renders
//! with no document attached.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use pixel_engine::comp_cache::{Budget, CompCache};
use pixel_engine::constants::FRAME_BUDGET_MS;
use pixel_engine::model::event::EventDetail;
use pixel_engine::model::{ModelEvent, NewDocumentOptions, Session};
use pixel_engine::view::ViewStore;

use crate::features::arc::ArcTool;
use crate::features::eyedropper::EyedropperTool;
use crate::features::fill::FillTool;
use crate::features::gradient::GradientTool;
use crate::features::hand::HandTool;
use crate::features::pen::PenTool;
use crate::features::shape::shape_tools;
use crate::features::text::TextTool;
use crate::input::InputController;
use crate::tools::api::ToolCx;
use crate::tools::manager::ToolManager;

/// The W8 event queue behind `Session::subscribe`.
pub type EventQueue = Rc<RefCell<VecDeque<ModelEvent>>>;

/// One status-bar message, newest last.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusMessage {
    pub level: String,
    pub text: String,
}

/// The frame budget as a wall-clock duration (`FRAME_BUDGET_MS`).
pub fn frame_budget() -> Duration {
    Duration::from_millis(FRAME_BUDGET_MS)
}

/// The deterministic test budget — the whole grid, no clock.
pub fn unlimited_budget() -> Budget {
    Budget::Unlimited
}

pub struct AppState {
    pub session: Session,
    pub view_store: ViewStore,
    pub tools: ToolManager,
    pub input: InputController,
    pub comp: CompCache,
    pub queue: EventQueue,
    pub status: Vec<StatusMessage>,
    pub history_dirty: bool,
    /// Last pointer position in canvas-widget CSS px, for move synthesis.
    pub last_pointer: Option<(f64, f64)>,
}

impl AppState {
    pub fn new() -> Self {
        let mut session = Session::new();
        session
            .new_document(NewDocumentOptions::default())
            .expect("default document opens");
        let queue: EventQueue = Rc::new(RefCell::new(VecDeque::new()));
        let sink = Rc::clone(&queue);
        session.subscribe(move |ev| {
            sink.borrow_mut().push_back(ev.clone());
            Ok(())
        });
        let mut tools = ToolManager::new();
        tools.register(Box::new(PenTool::pen()));
        tools.register(Box::new(PenTool::eraser()));
        tools.register(Box::new(FillTool::new()));
        tools.register(Box::new(EyedropperTool::new()));
        tools.register(Box::new(HandTool::new()));
        for tool in shape_tools() {
            tools.register(tool);
        }
        tools.register(Box::new(ArcTool::arc()));
        tools.register(Box::new(ArcTool::chord()));
        tools.register(Box::new(GradientTool::new()));
        tools.register(Box::new(TextTool::new()));
        let mut state = Self {
            session,
            view_store: ViewStore::new(),
            tools,
            input: InputController::new(),
            comp: CompCache::new(),
            queue,
            status: vec![],
            history_dirty: true,
            last_pointer: None,
        };
        state.after_document_replaced();
        let mut render = false;
        {
            let view = state.view_store.view();
            let mut cx = ToolCx {
                session: &mut state.session,
                view,
                need_render: &mut render,
            };
            let _ = state.tools.activate_initial(&mut cx);
        }
        state
    }

    /// Point the view at the current document and fit it.
    pub fn after_document_replaced(&mut self) {
        if let Some(doc) = self.session.doc() {
            let _ = self
                .view_store
                .set_canvas(doc.canvas.width_px, doc.canvas.height_px);
            self.view_store.fit();
        } else {
            self.view_store.clear_canvas();
        }
    }

    /// Drain the queue into comp cache, tools, and status.
    /// Returns whether the frame must repaint/upload.
    pub fn drain_events(&mut self) -> bool {
        let mut render = false;
        let events: Vec<ModelEvent> = self.queue.borrow_mut().drain(..).collect();
        for ev in &events {
            render |= self.comp.apply_event(ev);
            match &ev.detail {
                EventDetail::Status { level, text, .. } => {
                    self.status.push(StatusMessage {
                        level: level.to_string(),
                        text: text.clone(),
                    });
                    if self.status.len() > 8 {
                        self.status.remove(0);
                    }
                }
                EventDetail::History(_) => {
                    self.history_dirty = true;
                }
                EventDetail::DocumentReplaced => {
                    self.after_document_replaced();
                }
                _ => {}
            }
            let view = self.view_store.view();
            let mut cx = ToolCx {
                session: &mut self.session,
                view,
                need_render: &mut render,
            };
            self.tools.on_model_event(ev, &mut cx);
        }
        render
    }

    /// The undo/redo/record state the history buttons read.
    pub fn history_state(&self) -> (bool, bool) {
        let h = self.session.history();
        (h.can_undo(), h.can_redo())
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixel_engine::view::View;

    use crate::input::RawPointer;

    /// Headless frame: controller → manager → comp → display pixels.
    #[test]
    fn headless_stroke_reaches_the_framebuffer() {
        use pixel_engine::display::{DisplayParams, GridMode, draw_display};

        let mut state = AppState::new();
        // The subscription attaches after the boot document, so the queue
        // starts empty; the comp cache still paints everything once.
        state.drain_events();
        let view = View::new(1.0, 0.0, 0.0).expect("valid view");
        let raw = RawPointer::new((10.0, 10.0));
        let accept = |b| b == crate::input::Button::Left;
        let ctx = crate::input::InputCtx {
            view,
            canvas: Some((512, 512)),
            viewport: Some((512.0, 512.0)),
            active_tool_id: "pen",
            tool_accepts_button: &accept,
            tool_cursor: crate::tools::CursorKind::Crosshair,
        };
        let out = state.input.pointer_down(&raw, &ctx);
        assert!(state.input.is_drawing());
        let mut render = false;
        {
            let mut cx = ToolCx {
                session: &mut state.session,
                view,
                need_render: &mut render,
            };
            for call in &out.tool_calls {
                match call {
                    crate::input::ToolCall::Down(ev) => {
                        state.tools.pointer_down(ev, &mut cx).expect("dispatch");
                    }
                    _ => panic!("down frame dispatches Down, got {call:?}"),
                }
            }
        }
        let up = state.input.pointer_up(&raw, &ctx);
        {
            let mut cx = ToolCx {
                session: &mut state.session,
                view,
                need_render: &mut render,
            };
            for call in &up.tool_calls {
                if let crate::input::ToolCall::Up(ev) = call {
                    state.tools.pointer_up(ev, &mut cx).expect("dispatch");
                }
            }
        }
        assert!(state.drain_events(), "stroke events request a frame");
        let doc = state.session.doc().expect("doc").clone();
        state.comp.update(&doc, None, frame_budget());
        let params = DisplayParams {
            view,
            canvas_w: doc.canvas.width_px,
            canvas_h: doc.canvas.height_px,
            css_w: 64.0,
            css_h: 64.0,
            dpr: 1.0,
            background: &doc.canvas.background,
            grid: GridMode::Off,
            comp: Some(state.comp.comp()),
        };
        let fb = draw_display(&params);
        assert!(
            fb.rgba.iter().any(|&b| b != 0),
            "the dot reaches the pixels"
        );
        assert!(session_undo_clears(&mut state));
    }

    fn session_undo_clears(state: &mut AppState) -> bool {
        use pixel_engine::display::{DisplayParams, GridMode, draw_display};
        use pixel_engine::view::View;

        assert!(state.session.undo().expect("undo"));
        assert!(state.drain_events());
        let doc = state.session.doc().expect("doc").clone();
        // Paint what remains dirty, then confirm a flat white frame.
        let mut guard = 0;
        while state.comp.update(&doc, None, frame_budget()) {
            guard += 1;
            assert!(guard < 8, "the cache settles");
        }
        let view = View::new(1.0, 0.0, 0.0).expect("valid view");
        let params = DisplayParams {
            view,
            canvas_w: doc.canvas.width_px,
            canvas_h: doc.canvas.height_px,
            css_w: 64.0,
            css_h: 64.0,
            dpr: 1.0,
            background: "#ffffff",
            grid: GridMode::Off,
            comp: Some(state.comp.comp()),
        };
        let fb = draw_display(&params);
        fb.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|px| *px == [255, 255, 255, 255])
    }
}
