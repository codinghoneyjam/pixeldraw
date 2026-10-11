//! `SessionCore` and `Session` — the document/history holder and its API.
//!
//! The split from the facade exists for [`crate::model::edit_session`]: an
//! [`EditHandle`] must borrow the non-document state for as long as a stroke
//! lasts, which is impossible while that state lives in the same struct as the
//! document it draws into (`docs/model_internals.md` §7).
//!
//! Two orderings are load-bearing: `step` reads the stack length **before**
//! `self.doc` (an empty session with no document reports "nothing to undo", not
//! `INVALID_STATE`), and `dispatch_effects` emits layers, then pixels, then
//! history — the layers panel rebuilds before the renderer learns which chunks
//! moved, which is before the history buttons re-read their label.

use crate::errors::{DrawToolError, EngineResult};
use crate::model::command::{Command, CommandEffects};
use crate::model::document::{Document, NewDocumentOptions, new_document};
use crate::model::edit_session::{EditHandle, EditOptions};
use crate::model::event::{EventBus, ModelEvent, Subscription};
use crate::model::history::UndoManager;
use crate::model::settings::{SettingInput, Settings, validate_setting};

/// Everything about a session that is not the document.
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
    ///
    /// The JS guarded `doc instanceof Document` and raised `INVALID_STATE`; that
    /// is unreachable here, because the parameter is a `Document`.
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
