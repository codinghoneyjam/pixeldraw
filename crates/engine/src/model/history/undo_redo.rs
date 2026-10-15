//! The undo/redo stacks: commit, merge, revert, and the listener dispatch.
//!
//! Ported 1:1 from `src/features/document/history.js`. The budget arithmetic and
//! the dirty flag live in [`super::budget`]; this half owns the stack movement
//! and the notification that follows every mutation.
//!
//! The two orderings that are load-bearing, because the JS result changes if you
//! "simplify" them:
//!
//! * on a failing `do`, the command is pushed **back** and the error rethrown,
//!   so a rejected command leaves both stacks byte-identical;
//! * `undo()` returns `false` *before* it touches the document when a stack is
//!   empty — the JS read the stack length first.
//!
//! `notify` destructures `self` into fields rather than taking the stack as a
//! reference: borrowing a stack's top entry and `self.listeners` at once is only
//! provably disjoint through the field split.

use crate::errors::EngineResult;
use crate::model::command::{Command, CommandEffects};
use crate::model::document::Document;

use super::budget::{Entry, HistoryNotify, NotifyKind, UndoManager};

/// Which stack a notification is about.
#[derive(Debug, Clone, Copy)]
pub(super) enum NotifyStack {
    /// The entry the undo stack now has on top.
    Undo,
    /// The entry the redo stack now has on top.
    Redo,
    /// Neither — `clear` passes `command: null`, and the JS `Session`
    /// subscription skipped updating `_lastCmd` for it.
    Cleared,
}

impl UndoManager {
    /// JS `commit(cmd, { applied: false })` — run `do`, then push/merge.
    pub fn apply(
        &mut self,
        doc: &mut Document,
        cmd: impl Command + 'static,
    ) -> EngineResult<CommandEffects> {
        self.commit(Box::new(cmd), Some(doc))
    }

    /// JS `commit(cmd, { applied: true })` — the caller already ran the effect
    /// (a finished stroke), so only the bookkeeping happens.
    ///
    /// Takes no document, because the JS branch it mirrors never touches
    /// `this._doc`.
    pub fn record(&mut self, cmd: impl Command + 'static) -> EngineResult<CommandEffects> {
        self.commit(Box::new(cmd), None)
    }

    /// `doc` is `None` when the effect was already applied.
    fn commit(
        &mut self,
        mut cmd: Box<dyn Command>,
        doc: Option<&mut Document>,
    ) -> EngineResult<CommandEffects> {
        if let Some(doc) = doc {
            // A failing `do` propagates before anything is pushed, so the stack
            // is untouched — the JS relied on the same ordering.
            cmd.apply(doc)?;
        }
        let can_try =
            !self.merge_broken && self.undo_stack.last().is_some() && cmd.supports_merge();
        if can_try
            && let Some(top) = self.undo_stack.last_mut()
            && cmd.merge_with(top.cmd.as_mut())
        {
            self.redo_stack.clear();
            self.recalc_bytes();
            let effects = self
                .undo_stack
                .last()
                .map_or_else(CommandEffects::default, |e| e.cmd.effects());
            self.notify(NotifyKind::Commit, NotifyStack::Undo);
            return Ok(effects);
        }
        self.seq += 1;
        let size = cmd.byte_size();
        let effects = cmd.effects();
        self.undo_stack.push(Entry { seq: self.seq, cmd });
        self.total_bytes += size;
        self.redo_stack.clear();
        self.enforce_budget();
        self.merge_broken = !self
            .undo_stack
            .last()
            .is_some_and(|e| e.cmd.supports_merge());
        self.notify(NotifyKind::Commit, NotifyStack::Undo);
        Ok(effects)
    }

    /// JS `undo()`. `Ok(false)` means "nothing to undo", not a failure.
    pub fn undo(&mut self, doc: &mut Document) -> EngineResult<bool> {
        let Some(mut entry) = self.undo_stack.pop() else {
            return Ok(false);
        };
        if let Err(e) = entry.cmd.revert(doc) {
            // The JS pushed the command back and rethrew.
            self.undo_stack.push(entry);
            return Err(e);
        }
        self.total_bytes -= entry.cmd.byte_size();
        self.redo_stack.push(entry);
        self.merge_broken = true;
        self.notify(NotifyKind::Undo, NotifyStack::Redo);
        Ok(true)
    }

    /// JS `redo()`. A redo that fails is left on the redo stack.
    pub fn redo(&mut self, doc: &mut Document) -> EngineResult<bool> {
        let Some(mut entry) = self.redo_stack.pop() else {
            return Ok(false);
        };
        if let Err(e) = entry.cmd.apply(doc) {
            self.redo_stack.push(entry);
            return Err(e);
        }
        self.total_bytes += entry.cmd.byte_size();
        self.undo_stack.push(entry);
        self.enforce_budget();
        self.merge_broken = true;
        self.notify(NotifyKind::Redo, NotifyStack::Undo);
        Ok(true)
    }

    /// The listener dispatch.
    ///
    /// The stack is named rather than passed as a reference because the borrow
    /// of a stack's top entry and the borrow of `self.listeners` have to be seen
    /// as disjoint; destructuring `self` into fields makes that provable.
    pub(super) fn notify(&mut self, kind: NotifyKind, stack: NotifyStack) {
        let Self {
            undo_stack,
            redo_stack,
            listeners,
            last_effects,
            ..
        } = self;
        let stack = match stack {
            NotifyStack::Undo => undo_stack,
            NotifyStack::Redo => redo_stack,
            NotifyStack::Cleared => {
                for listener in listeners.values_mut() {
                    let _ = listener(&HistoryNotify {
                        kind,
                        command: None,
                    });
                }
                return;
            }
        };
        let Some(entry) = stack.last() else {
            return;
        };
        let command: &dyn Command = entry.cmd.as_ref();
        if kind != NotifyKind::Clear {
            *last_effects = Some(command.effects());
        }
        for listener in listeners.values_mut() {
            // The JS `try { fn(...) } catch (err) { console.error(err) }`: a
            // throwing subscriber must not stop the remaining listeners, and
            let _ = listener(&HistoryNotify {
                kind,
                command: Some(command),
            });
        }
    }
}
