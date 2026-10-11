//! `Session` — the document, its settings, its history and the event bus.
//!
//! Ported 1:1 from `src/features/document/session.js`. The JS `Session` extended
//! `EventTarget`; here the state is split so that an [`crate::model::EditHandle`]
//! can hold a borrow of the parts it needs alongside a borrow of the layer store
//! it is drawing into:
//!
//! ```text
//! Session { doc: Option<Document>, core: SessionCore }
//! SessionCore { history, editing, bus, settings }
//! ```
//!
//! The JS `_lastCmd` (written by a history subscription that re-dispatched
//! `_dispatchEffects(this._lastCmd)` after undo/redo) lives on the manager as
//! [`crate::model::UndoManager::last_effects`], because a session cannot
//! subscribe to a field of itself.

use crate::errors::{DrawToolError, EngineResult};
use crate::model::command::{Command, CommandEffects};
use crate::model::document::{Document, NewDocumentOptions, new_document};
use crate::model::edit_session::{EditHandle, EditOptions};
use crate::model::event::{EventBus, ModelEvent, Subscription};
use crate::model::history::UndoManager;
use crate::model::layer::Layer;
use crate::model::settings::{SettingInput, Settings, validate_setting};

/// Everything about a session that is not the document.
///
/// Split out of [`Session`] so [`crate::model::edit_session`] can borrow it for
/// as long as a stroke lasts without borrowing the document too.
pub struct SessionCore {
    pub(crate) history: UndoManager,
    pub(crate) editing: bool,
    pub(crate) bus: EventBus,
    settings: Settings,
}

impl SessionCore {
    pub(crate) fn new() -> Self {
        Self {
            history: UndoManager::new(),
            editing: false,
            bus: EventBus::new(),
            settings: Settings::default(),
        }
    }

    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }

    pub(crate) fn dispatch_effects(&mut self, effects: &CommandEffects) {
        // JS `_dispatchEffects`: the layers event, then every pixel event, then
        // one history event. The order is load-bearing — the layers panel
        // rebuilds before the renderer is told which chunks moved, which is
        // before the history buttons re-read their label.
        for event in ModelEvent::from_effects(effects) {
            self.bus.emit(event);
        }
        let state = self.history.state();
        self.bus.emit(ModelEvent::history_changed(state));
    }
}

impl std::fmt::Debug for SessionCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The settings tablet is too wide to print; the two things an assertion
        // message would want are the history depth and whether a stroke is open.
        f.debug_struct("SessionCore")
            .field("undo", &self.history.can_undo())
            .field("editing", &self.editing)
            .finish()
    }
}

/// The live editing session: one document, one tool-settings tablet, one
/// history, and the seven signals every panel listens to.
pub struct Session {
    doc: Option<Document>,
    core: SessionCore,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            doc: None,
            core: SessionCore::new(),
        }
    }

    /// JS `get doc()` — `None` before the first document.
    pub fn doc(&self) -> Option<&Document> {
        self.doc.as_ref()
    }

    /// `_requireDoc()`.
    pub fn require_doc(&self) -> EngineResult<&Document> {
        self.doc.as_ref().ok_or_else(no_document)
    }

    pub fn doc_mut(&mut self) -> EngineResult<&mut Document> {
        self.doc.as_mut().ok_or_else(no_document)
    }

    /// `get history()` — mutable as well, because the JS let the UI write
    /// `session.history.limitBytes` (the autosave budget dial).
    pub fn history(&self) -> &UndoManager {
        &self.core.history
    }

    pub fn history_mut(&mut self) -> &mut UndoManager {
        &mut self.core.history
    }

    /// `get settings()` — a copy, like the JS frozen spread.
    pub fn settings(&self) -> Settings {
        self.core.settings().clone()
    }

    /// `get isEditing()`.
    ///
    /// Note that an abandoned [`EditHandle`] — dropped without `commit` or
    /// `cancel` — leaves this set for good, exactly as the JS left
    /// `session._edit` set. There is deliberately no `Drop` that clears it, so
    /// the parity stays observable (and tested).
    pub fn is_editing(&self) -> bool {
        self.core.editing
    }

    pub fn subscribe(
        &mut self,
        f: impl FnMut(&ModelEvent) -> EngineResult<()> + 'static,
    ) -> Subscription {
        self.core.bus.subscribe(f)
    }

    pub fn unsubscribe(&mut self, sub: Subscription) {
        self.core.bus.unsubscribe(sub);
    }

    pub fn listener_count(&self) -> usize {
        self.core.bus.listener_count()
    }

    /// `notify(level, text, code)` — the status bar.
    pub fn notify(&mut self, level: &'static str, text: impl Into<String>, code: &'static str) {
        self.core
            .bus
            .emit(ModelEvent::status_message(level, text, code));
    }

    /// `emitToolState(tool, pending)`.
    ///
    /// `pending` is the tool's own pending state, already serialized: `shape.js`
    /// compares `JSON.stringify({ pending, vertices })` for its own change
    /// detection, so the text is the contract. Its Rust type belongs to the
    /// Phase 2/3 tools, not to the engine.
    pub fn emit_tool_state(&mut self, tool: impl Into<String>, pending: impl Into<String>) {
        self.core.bus.emit(ModelEvent::tool_state(tool, pending));
    }

    /// `setSetting(key, value)`.
    pub fn set_setting(&mut self, key: &str, value: SettingInput<'_>) -> EngineResult<bool> {
        let (key, value) = validate_setting(key, value)?;
        if !self.core.settings.set(key, value.clone()) {
            return Ok(false);
        }
        self.core.bus.emit(ModelEvent::settings_changed(key, value));
        Ok(true)
    }

    /// `newDocument(opts)`.
    pub fn new_document(&mut self, opts: NewDocumentOptions) -> EngineResult<&Document> {
        self.install(new_document(opts)?, true);
        self.require_doc()
    }

    /// `loadDocument(doc, { markSaved = true })`.
    pub fn load_document(&mut self, doc: Document, mark_saved: bool) -> EngineResult<&Document> {
        self.install(doc, mark_saved);
        self.require_doc()
    }

    fn install(&mut self, doc: Document, mark_saved: bool) {
        self.doc = Some(doc);
        self.core.history.set_document();
        if mark_saved {
            self.core.history.mark_saved();
        }
        self.core.editing = false;
        self.core.bus.emit(ModelEvent::document_replaced());
        self.emit_history_changed();
    }

    fn emit_history_changed(&mut self) {
        let state = self.core.history.state();
        self.core.bus.emit(ModelEvent::history_changed(state));
    }

    fn dispatch_effects(&mut self, effects: &CommandEffects) {
        self.core.dispatch_effects(effects);
    }

    /// `execute(cmd)` — run the effect and record it in one entry.
    pub fn execute(&mut self, cmd: impl Command + 'static) -> EngineResult<()> {
        let effects = {
            let Self { doc, core } = self;
            let document = doc.as_mut().ok_or_else(no_document)?;
            core.history.apply(document, cmd)?
        };
        self.dispatch_effects(&effects);
        Ok(())
    }

    /// `record(cmd)` — the effect already happened (a finished stroke), so the
    /// command only lands in the history.
    pub fn record(&mut self, cmd: impl Command + 'static) -> EngineResult<()> {
        let effects = self.core.history.record(cmd)?;
        self.dispatch_effects(&effects);
        Ok(())
    }

    /// `undo()`. `Ok(false)` while a stroke is open, or with nothing to undo.
    pub fn undo(&mut self) -> EngineResult<bool> {
        let effects = self.step(true)?;
        if let Some(effects) = &effects {
            self.dispatch_effects(effects);
        }
        Ok(effects.is_some())
    }

    /// `redo()`. `Ok(false)` while a stroke is open, or with nothing to redo.
    pub fn redo(&mut self) -> EngineResult<bool> {
        let effects = self.step(false)?;
        if let Some(effects) = &effects {
            self.dispatch_effects(effects);
        }
        Ok(effects.is_some())
    }

    /// Shared body of `undo`/`redo`.
    ///
    /// Returns the effects of the command the manager acted on, which is the JS
    /// `this._lastCmd` — `None` when the isEditing guard fired or the stack was
    /// empty.
    fn step(&mut self, undo: bool) -> EngineResult<Option<CommandEffects>> {
        if self.core.editing {
            return Ok(None);
        }
        // The JS `history.undo()` returned false on an empty stack *before* it
        // ever touched `this._doc`, so a session with no document reports
        // "nothing to undo" rather than a missing-document error.
        let has_steps = if undo {
            self.core.history.can_undo()
        } else {
            self.core.history.can_redo()
        };
        if !has_steps {
            return Ok(None);
        }
        let Some(document) = self.doc.as_mut() else {
            return Err(no_document());
        };
        let acted = if undo {
            self.core.history.undo(document)?
        } else {
            self.core.history.redo(document)?
        };
        if !acted {
            return Ok(None);
        }
        Ok(self.core.history.last_effects().cloned())
    }

    // ---- the layer operations, as session.js's one-line delegates ----

    pub fn add_layer(&mut self, name: Option<&str>) -> EngineResult<String> {
        crate::model::operations::add_layer(self, name)
    }

    pub fn duplicate_layer(&mut self, id: &str) -> EngineResult<String> {
        crate::model::operations::duplicate_layer(self, id)
    }

    pub fn remove_layer(&mut self, id: &str) -> EngineResult<()> {
        crate::model::operations::remove_layer(self, id)
    }

    pub fn move_layer(&mut self, id: &str, to_index: i64) -> EngineResult<bool> {
        crate::model::operations::move_layer(self, id, to_index)
    }

    pub fn merge_down(&mut self, id: &str) -> EngineResult<()> {
        crate::model::operations::merge_down(self, id)
    }

    pub fn insert_layer(&mut self, layer: Layer, index: Option<usize>) -> EngineResult<String> {
        crate::model::operations::insert_layer(self, layer, index)
    }

    pub fn resize_canvas(&mut self, w: u32, h: u32) -> EngineResult<bool> {
        crate::model::operations::resize_canvas(self, w, h)
    }

    pub fn rename_layer(&mut self, id: &str, name: &str) -> EngineResult<bool> {
        crate::model::operations::rename_layer(self, id, name)
    }

    pub fn set_layer_visible(&mut self, id: &str, visible: bool) -> EngineResult<bool> {
        crate::model::operations::set_layer_visible(self, id, visible)
    }

    pub fn set_layer_locked(&mut self, id: &str, locked: bool) -> EngineResult<bool> {
        crate::model::operations::set_layer_locked(self, id, locked)
    }

    /// `setLayerOpacity(id, value, { final })` — see
    /// [`crate::model::operations::set_layer_opacity`] for what `final` means.
    pub fn set_layer_opacity(&mut self, id: &str, value: f64, final_: bool) -> EngineResult<bool> {
        crate::model::operations::set_layer_opacity(self, id, value, final_)
    }

    pub fn set_layer_blend(&mut self, id: &str, value: &'static str) -> EngineResult<bool> {
        crate::model::operations::set_layer_blend(self, id, value)
    }

    pub fn set_active_layer(&mut self, id: &str) -> EngineResult<bool> {
        crate::model::operations::set_active_layer(self, id)
    }

    pub fn begin_edit(&mut self, opts: EditOptions) -> EngineResult<EditHandle<'_>> {
        let Self { doc, core } = self;
        let document = doc.as_mut().ok_or_else(no_document)?;
        if core.editing {
            return Err(DrawToolError::invalid_state("already editing"));
        }
        let layer_id = opts
            .layer_id
            .clone()
            .unwrap_or_else(|| document.active_layer_id.clone());
        let layer = document.get_layer_mut(&layer_id)?;
        if layer.locked {
            // The JS notified *and* threw; the status message is what the option
            // bar's warning comes from, so it must be emitted before the error
            // travels.
            core.bus.emit(ModelEvent::status_message(
                "warn",
                "레이어가 잠겨 있습니다",
                crate::errors::CODE_LAYER_LOCKED,
            ));
            return Err(DrawToolError::layer_locked());
        }
        if !layer.visible {
            core.bus.emit(ModelEvent::status_message(
                "warn",
                "레이어가 숨겨져 있습니다",
                crate::errors::CODE_LAYER_HIDDEN,
            ));
            return Err(DrawToolError::layer_hidden());
        }
        let writer = crate::chunkstore::PixelWriter::new(&mut layer.store, layer.id.clone());
        let label = opts.label.unwrap_or_else(|| "연필".to_string());
        core.editing = true;
        Ok(EditHandle::new(writer, layer_id, label, core))
    }

    /// The one event the layer operations emit outside a command's effects.
    pub(crate) fn emit_layers_changed(
        &mut self,
        reason: crate::model::LayerChangeReason,
        layer_ids: Vec<String>,
    ) {
        self.core
            .bus
            .emit(ModelEvent::layers_changed(reason, layer_ids));
    }
}

fn no_document() -> DrawToolError {
    DrawToolError::invalid_state("no document loaded")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Keeps the imports honest: `SetLayerPropCommand`/`PaintCommand`/
    // `MoveLayerCommand`/`ResizeCanvasCommand` all belong to the surface the
    // session re-exports through `crate::model`.
    #[test]
    fn sessions_start_empty_and_settings_start_at_the_defaults() {
        let mut s = Session::new();
        assert!(s.doc().is_none());
        assert!(!s.is_editing());
        assert_eq!(s.settings(), Settings::default());
        assert!(s.require_doc().is_err());
        // No document and nothing to undo: the JS returned `false` here too.
        assert!(!s.undo().expect("no document to undo into"));
        assert!(!s.redo().expect("no document to redo into"));
        assert_eq!(s.history().state(), crate::model::HistoryState::default());
    }

    #[test]
    fn new_document_dispatches_replaced_then_history() {
        let mut s = Session::new();
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::<&'static str>::new()));
        let a = log.clone();
        s.subscribe(move |e| {
            a.borrow_mut().push(e.name);
            Ok(())
        });
        s.new_document(NewDocumentOptions::default().size(32, 32))
            .expect("new");
        assert_eq!(
            &*log.borrow(),
            &[
                crate::events::EVENT_DOCUMENT_REPLACED,
                crate::events::EVENT_HISTORY_CHANGED,
            ]
        );
        assert!(s.doc().is_some());
        assert!(!s.history().can_undo());
        assert_eq!(s.history().state(), crate::model::HistoryState::default());
        assert_eq!(s.listener_count(), 1);
    }

    #[test]
    fn undo_and_redo_walk_the_stroke_and_property_stacks() {
        let mut s = Session::new();
        s.new_document(NewDocumentOptions::default().size(32, 32))
            .expect("new");
        let id = s.doc().unwrap().active_layer_id.clone();
        s.set_layer_visible(&id, false).expect("visible");
        assert!(
            s.begin_edit(EditOptions::default()).is_err(),
            "a hidden layer refuses an edit"
        );
        s.set_layer_visible(&id, true).expect("visible");
        let mut edit = s.begin_edit(EditOptions::default()).expect("edit");
        assert!(edit.is_active());
        edit.writer().expect("editing").set(1, 1, 0xff0000ff);
        assert!(edit.commit(None).expect("commit"));
        assert!(!s.is_editing());
        // Three entries: the two visibility toggles and the stroke.
        let mut n = 0;
        while s.undo().expect("undo") {
            n += 1;
        }
        assert_eq!(n, 3);
        assert!(s.redo().expect("redo"));
        assert!(s.redo().expect("redo"));
        assert!(s.redo().expect("redo"));
        assert!(!s.redo().expect("nothing left"));
    }
}
