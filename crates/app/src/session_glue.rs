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
use std::time::{Duration, Instant};

use pixel_engine::comp_cache::{Budget, CompCache};
use pixel_engine::constants::FRAME_BUDGET_MS;
use pixel_engine::model::event::EventDetail;
use pixel_engine::model::{ModelEvent, NewDocumentOptions, Session};
use pixel_engine::view::ViewStore;

use crate::ui::autosave::{AutosaveGlue, FlushError};
use crate::ui::settings_store::SettingsStore;

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
    /// `settings.json` debounce + merge-save (Phase 3.9).
    pub settings_store: SettingsStore,
    /// `autosave.json` schedule + boot record (Phase 3.10).
    pub autosave: AutosaveGlue,
    /// A dialog is open this frame: shortcut handling (UI-3) and the canvas
    /// key stream read this.
    pub ui_busy: bool,
}

impl AppState {
    /// Headless construction: nothing is read from or written to disk. Tests
    /// and any non-shell caller stay hermetic; the shell uses
    /// [`AppState::boot`] so settings + autosave join the boot order.
    pub fn new() -> Self {
        Self::boot_with(SettingsStore::empty(), AutosaveGlue::disabled())
    }

    /// The shell's boot: read `settings.json` + `autosave.json` before any
    /// listener attaches (`ui/app.js` L83-99 — `restoreSettings` runs before
    /// the first document and before the autosave store attaches, so neither
    /// restore writes itself back).
    pub fn boot() -> Self {
        Self::boot_with(SettingsStore::load(), AutosaveGlue::boot())
    }

    fn boot_with(settings_store: SettingsStore, autosave: AutosaveGlue) -> Self {
        let mut session = Session::new();
        settings_store.restore_into(&mut session);
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
            settings_store,
            autosave,
            ui_busy: false,
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
        let now = Instant::now();
        for ev in &events {
            self.settings_store.on_event(ev, now);
            self.autosave.on_event(ev, now);
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

    /// Frame-loop persistence (Phase 3.9/3.10): the settings debounce and the
    /// autosave schedule. The shell calls this once per frame after the UI ran.
    pub fn tick_persistence(&mut self, now: Instant) {
        self.settings_store.tick(now, &self.session);
        if let Some(outcome) = self.autosave.flush_if_due(&self.session, now) {
            self.report_flush(outcome);
        }
    }

    /// `pagehide` parity: write the pending autosave right now (window close /
    /// focus loss); a no-op when nothing is dirty.
    pub fn flush_autosave(&mut self) {
        if let Some(outcome) = self.autosave.flush_now(&self.session) {
            self.report_flush(outcome);
        }
    }

    fn report_flush(&mut self, outcome: Result<(), FlushError>) {
        let Err(kind) = outcome else { return };
        let (text, code) = match kind {
            FlushError::Quota => ("자동저장 공간 부족", "STORAGE_QUOTA"),
            FlushError::Other => ("자동저장 실패", "INVALID_STATE"),
        };
        self.session.notify("warn", text, code);
    }

    /// `(name, when)` for the boot restore dialog; `None` = nothing to offer.
    pub fn autosave_peek(&self) -> Option<(String, String)> {
        self.autosave.peek()
    }

    /// The restore dialog resolved to 복원. Returns whether a document was
    /// installed; a record failing the deep checks is cleared first
    /// (3.10a `load` semantics).
    pub fn restore_autosave(&mut self) -> bool {
        let Some(result) = self.autosave.restore_pending() else {
            return false;
        };
        match result {
            Ok(doc) => {
                let _ = self.session.load_document(doc, false);
                self.after_document_replaced();
                self.drain_events();
                true
            }
            Err(err) => {
                eprintln!("autosave restore failed: {err}");
                self.autosave.clear();
                false
            }
        }
    }

    /// The restore dialog resolved to 버리기 / Esc / close: drop the record.
    pub fn discard_autosave(&mut self) {
        self.autosave.clear();
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

    /// Phase 3.9 boot order: the settings restore runs before the first
    /// document and before the listener attaches (`app.js` L83-99), so nothing
    /// is queued and the file cannot be written back by its own events.
    #[test]
    fn boot_applies_settings_without_queueing_events() {
        let path = std::env::temp_dir().join(format!(
            "pixeldraw-appstate-settings-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, r##"{"penSize": 12, "activeTool": "eraser"}"##).expect("seed");
        let store = SettingsStore::load_from(Some(path.clone()));
        let mut state = AppState::boot_with(store, AutosaveGlue::disabled());
        assert_eq!(state.session.settings().pen_size, 12);
        assert_eq!(state.session.settings().active_tool, "eraser");
        assert!(state.queue.borrow().is_empty(), "restore is listener-free");
        assert!(!state.drain_events());
        let _ = std::fs::remove_file(&path);
    }

    /// Phase 3.10b: restoring installs the autosave document through the queue
    /// and marks the dialog resolved. The restored document reads **clean** —
    /// the JS oracle returns `isDirty() == false` after
    /// `loadDocument(doc, {markSaved:false})` (checked with node on
    /// 2026-10-17; the roadmap's "복원 직후 dirty ●" note is wrong).
    #[test]
    fn restoring_an_autosave_installs_the_document_clean() {
        let path = std::env::temp_dir().join(format!(
            "pixeldraw-appstate-autosave-{}.json",
            std::process::id()
        ));
        let seed = AppState::new();
        let doc = seed.session.doc().expect("doc");
        let doc_id = doc.id.clone();
        let bytes = crate::ui::autosave::AutosaveRecord::build(doc, 42)
            .expect("build")
            .encode();
        std::fs::write(&path, &bytes).expect("seed");

        let glue = AutosaveGlue::boot_from(Some(path.clone()));
        assert!(glue.peek().is_some(), "the record is offered");
        let mut state = AppState::boot_with(SettingsStore::empty(), glue);
        assert!(state.autosave_peek().is_some());
        assert!(state.restore_autosave());
        assert_eq!(state.session.doc().expect("doc").id, doc_id);
        assert!(
            !state.session.history().is_dirty(),
            "matches the JS oracle: loadDocument({{markSaved:false}}) is clean"
        );
        assert!(state.autosave_peek().is_none(), "the dialog is resolved");
        let _ = std::fs::remove_file(&path);
    }
}
