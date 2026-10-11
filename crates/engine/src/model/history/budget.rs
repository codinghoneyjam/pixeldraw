//! The byte budget, the merge policy and the dirty-flag semantics of the undo
//! manager — the part a caller cannot read off the method signatures.
//!
//! Ported 1:1 from `src/features/document/history.js`. Two decisions drive
//! everything here and both are spelled out on the item that makes them:
//!
//! * the budget guard is `length > 1`, so a single over-sized entry always
//!   survives (the JS said the same, by the shape of its `while`);
//! * the JS compared **object identity** for `_savedTop`, which `Box<dyn
//!   Command>` cannot do, so the identity is carried as a number instead.
//!
//! `undo_redo` holds the rest of the manager.

use std::collections::BTreeMap;

use crate::constants::{UNDO_MAX_BYTES, UNDO_MAX_STEPS};
use crate::errors::EngineResult;
use crate::model::command::{Command, CommandEffects};
use crate::model::event::Subscription;
use std::fmt;

use super::undo_redo::NotifyStack;

/// What the subscriber's `{ kind }` was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyKind {
    Commit,
    Undo,
    Redo,
    Clear,
}

impl NotifyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Clear => "clear",
        }
    }
}

/// The `{ kind, command }` payload the JS `_notify` handed subscribers.
///
/// `command` is `None` only for `clear`, where the JS passed `command: null`.
pub struct HistoryNotify<'a> {
    pub kind: NotifyKind,
    pub command: Option<&'a dyn Command>,
}

/// A history subscriber.
///
/// Returning `Err` stands in for a JS listener that threw: [`UndoManager::notify`]
/// drops the error and keeps going, exactly as the `try/catch` + `console.error`
/// did. The engine has no console, so the error is not logged.
pub type HistoryListener = Box<dyn FnMut(&HistoryNotify<'_>) -> EngineResult<()>>;

/// One stack slot.
///
/// `seq` is a manager-local monotonic counter assigned at commit time. The JS
/// compared **object identity** (`evicted === this._savedTop`,
/// `top !== this._savedTop`); `Box<dyn Command>` has no `PartialEq`, and a raw
/// pointer comparison would outlive the eviction it is meant to track, so the
/// identity is carried as a number that follows the command between the undo and
/// redo stacks.
#[derive(Debug)]
pub(super) struct Entry {
    pub(super) seq: u64,
    pub(super) cmd: Box<dyn Command>,
}

/// The `history-changed` detail — the JS `_historyState()`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HistoryState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_label: Option<String>,
    pub redo_label: Option<String>,
    pub dirty: bool,
}

/// The undo/redo manager.
#[derive(Default)]
pub struct UndoManager {
    /// The JS `limitSteps` — default 200.
    pub limit_steps: usize,
    /// The JS `limitBytes` — default 96 MiB.
    pub limit_bytes: usize,
    pub(super) undo_stack: Vec<Entry>,
    pub(super) redo_stack: Vec<Entry>,
    pub(super) total_bytes: usize,
    pub(super) seq: u64,
    /// The `_savedTop` identity as a sequence number.
    pub(super) saved_top: Option<u64>,
    pub(super) saved_lost: bool,
    pub(super) merge_broken: bool,
    pub(super) listeners: BTreeMap<usize, HistoryListener>,
    pub(super) next_listener_id: usize,
    /// The `_lastCmd` effects the session re-dispatches on undo/redo.
    ///
    /// In the JS this lived on `Session` and was written by a history
    /// subscription; `Session` subscribes to nothing here, because it cannot
    /// borrow itself. `clear` does **not** update it — the JS subscription
    /// skipped `kind === "clear"`.
    pub(super) last_effects: Option<CommandEffects>,
}

impl fmt::Debug for UndoManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UndoManager")
            .field("limit_steps", &self.limit_steps)
            .field("limit_bytes", &self.limit_bytes)
            .field("undo", &self.undo_stack.len())
            .field("redo", &self.redo_stack.len())
            .field("total_bytes", &self.total_bytes)
            .field("listeners", &self.listeners.len())
            .field("merge_broken", &self.merge_broken)
            .finish()
    }
}

impl UndoManager {
    /// JS `new UndoManager({limitSteps = 200, limitBytes = 96 MiB})`.
    pub fn new() -> Self {
        Self {
            limit_steps: UNDO_MAX_STEPS,
            limit_bytes: UNDO_MAX_BYTES,
            ..Self::default()
        }
    }

    pub fn with_limits(limit_steps: usize, limit_bytes: usize) -> Self {
        Self {
            limit_steps,
            limit_bytes,
            ..Self::default()
        }
    }

    /// JS `setDocument(doc)`.
    ///
    /// The JS manager held the document; here the `Session` that owns it passes
    /// `&mut Document` to [`UndoManager::apply`] instead, because a manager
    /// nested inside its own session cannot borrow it back. What `setDocument`
    /// did to the **stacks** still has to happen explicitly, so this is that
    /// call — and it also sets `mergeBroken`, so the first commit after a
    /// document swap never merges into the previous document's entry.
    pub fn set_document(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.total_bytes = 0;
        self.saved_top = None;
        self.saved_lost = false;
        self.merge_broken = true;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_label(&self) -> Option<String> {
        self.undo_stack.last().map(|e| e.cmd.label().to_string())
    }

    pub fn redo_label(&self) -> Option<String> {
        self.redo_stack.last().map(|e| e.cmd.label().to_string())
    }

    pub fn break_merge(&mut self) {
        self.merge_broken = true;
    }

    pub fn mark_saved(&mut self) {
        self.saved_top = self.undo_stack.last().map(|e| e.seq);
        self.saved_lost = false;
    }

    /// `_savedLost || top !== savedTop`.
    pub fn is_dirty(&self) -> bool {
        if self.saved_lost {
            return true;
        }
        self.undo_stack.last().map(|e| e.seq) != self.saved_top
    }

    /// The effects of the most recently notified command — the session's
    /// `_lastCmd`. `None` before the first commit and after a `clear`.
    pub fn last_effects(&self) -> Option<&CommandEffects> {
        self.last_effects.as_ref()
    }

    /// The `history-changed` detail.
    pub fn state(&self) -> HistoryState {
        HistoryState {
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
            undo_label: self.undo_label(),
            redo_label: self.redo_label(),
            dirty: self.is_dirty(),
        }
    }

    pub fn subscribe(
        &mut self,
        f: impl FnMut(&HistoryNotify<'_>) -> EngineResult<()> + 'static,
    ) -> Subscription {
        let id = self.next_listener_id;
        self.next_listener_id += 1;
        self.listeners.insert(id, Box::new(f));
        Subscription::new(id)
    }

    pub fn unsubscribe(&mut self, sub: Subscription) {
        self.listeners.remove(&sub.id());
    }

    /// Evict oldest-first while over either budget. The `length > 1` guard is
    /// the JS's: one over-sized entry always survives, because the loop stops
    /// before the stack can empty.
    pub(super) fn enforce_budget(&mut self) {
        while self.undo_stack.len() > 1
            && (self.undo_stack.len() > self.limit_steps || self.total_bytes > self.limit_bytes)
        {
            let evicted = self.undo_stack.remove(0);
            self.total_bytes -= evicted.cmd.byte_size();
            if Some(evicted.seq) == self.saved_top {
                self.saved_top = None;
                self.saved_lost = true;
            }
        }
    }

    pub(super) fn recalc_bytes(&mut self) {
        self.total_bytes = self.undo_stack.iter().map(|e| e.cmd.byte_size()).sum();
    }

    /// JS `clear()` — the one mutation that skips `last_effects`.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.total_bytes = 0;
        self.saved_top = None;
        self.saved_lost = false;
        self.merge_broken = true;
        // `clear` deliberately skips `last_effects`: the JS subscription skipped
        // `kind === "clear"`, so `last_effects()` survives a clear.
        self.notify(NotifyKind::Clear, NotifyStack::Cleared);
    }
}
