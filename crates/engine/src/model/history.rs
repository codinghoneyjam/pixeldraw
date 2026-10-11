//! The undo history — step/byte budget and the merge policy.
//!
//! Ported 1:1 from `src/features/document/history.js`. Three behaviours here are
//! easy to get wrong and are therefore spelled out:
//!
//! 1. **The budget never empties the stack.** `_enforceBudget`'s guard is
//!    `length > 1`, so a single entry always survives however large it is.
//! 2. **The merge policy is `!mergeBroken && top && hasMergeWith && merged`.**
//!    Every part of it matters — see [`UndoManager::commit`].
//! 3. **A failing `do`/`undo` puts the command back.** The JS caught, restored
//!    and rethrew, so a rejected command leaves the stacks byte-identical.

use std::collections::BTreeMap;

use crate::constants::{UNDO_MAX_BYTES, UNDO_MAX_STEPS};
use crate::errors::EngineResult;
use crate::model::command::{Command, CommandEffects};
use crate::model::document::Document;
use crate::model::event::Subscription;
use std::fmt;

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
struct Entry {
    seq: u64,
    cmd: Box<dyn Command>,
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
    undo_stack: Vec<Entry>,
    redo_stack: Vec<Entry>,
    total_bytes: usize,
    seq: u64,
    /// The `_savedTop` identity as a sequence number.
    saved_top: Option<u64>,
    saved_lost: bool,
    merge_broken: bool,
    listeners: BTreeMap<usize, HistoryListener>,
    next_listener_id: usize,
    /// The `_lastCmd` effects the session re-dispatches on undo/redo.
    ///
    /// In the JS this lived on `Session` and was written by a history
    /// subscription; `Session` subscribes to nothing here, because it cannot
    /// borrow itself. `clear` does **not** update it — the JS subscription
    /// skipped `kind === "clear"`.
    last_effects: Option<CommandEffects>,
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

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.total_bytes = 0;
        self.saved_top = None;
        self.saved_lost = false;
        self.merge_broken = true;
        self.notify(NotifyKind::Clear, NotifyStack::Cleared);
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

    fn recalc_bytes(&mut self) {
        self.total_bytes = self.undo_stack.iter().map(|e| e.cmd.byte_size()).sum();
    }

    fn enforce_budget(&mut self) {
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

    /// The listener dispatch.
    ///
    /// The stack is named rather than passed as a reference because the borrow
    /// of a stack's top entry and the borrow of `self.listeners` have to be seen
    /// as disjoint; destructuring `self` into fields makes that provable.
    fn notify(&mut self, kind: NotifyKind, stack: NotifyStack) {
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
            // must not fail the commit. The engine has no console, so the
            // drop is where `console.error` used to be.
            let _ = listener(&HistoryNotify {
                kind,
                command: Some(command),
            });
        }
    }
}

/// Which stack a notification is about.
#[derive(Debug, Clone, Copy)]
enum NotifyStack {
    /// The entry the undo stack now has on top.
    Undo,
    /// The entry the redo stack now has on top.
    Redo,
    /// Neither — `clear` passes `command: null`, and the JS `Session`
    /// subscription skipped updating `_lastCmd` for it.
    Cleared,
}

/// A command that fails on `do`, for the failure-isolation test.
#[cfg(test)]
#[derive(Debug)]
struct FailingCommand;

#[cfg(test)]
use crate::errors::DrawToolError;

#[cfg(test)]
impl Command for FailingCommand {
    fn label(&self) -> &str {
        "fail"
    }

    fn type_name(&self) -> &'static str {
        "x"
    }

    fn apply(&mut self, _doc: &mut Document) -> EngineResult<()> {
        Err(DrawToolError::invalid_state("do-fail"))
    }

    fn revert(&mut self, _doc: &mut Document) -> EngineResult<()> {
        Ok(())
    }

    fn byte_size(&self) -> usize {
        0
    }

    fn effects(&self) -> CommandEffects {
        CommandEffects::default()
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::DrawToolError;
    use crate::model::commands::SetLayerPropCommand;
    use crate::model::document::{NewDocumentOptions, new_document};
    use crate::model::layer::{LayerField, LayerPropValue};

    fn doc() -> Document {
        new_document(NewDocumentOptions::default().size(32, 32)).expect("valid")
    }

    fn name_cmd(id: &str, before: &str, after: &str, label: &str) -> SetLayerPropCommand {
        SetLayerPropCommand::with_label(
            id.to_string(),
            LayerField::Name,
            LayerPropValue::Name(before.to_string()),
            LayerPropValue::Name(after.to_string()),
            label,
        )
    }

    // <META - ROLE : parity with tests/model_history.test.mjs B5.4 merge case | L20-30>
    #[test]
    fn thirty_opacity_commits_collapse_into_one_entry() {
        let mut d = doc();
        let id = d.active_layer_id.clone();
        let mut hist = UndoManager::new();
        hist.set_document();
        for i in 0..30 {
            let opacity = f64::from(i + 1) / 30.0;
            let current = d.get_layer(&id).unwrap().opacity;
            hist.apply(
                &mut d,
                SetLayerPropCommand::with_label(
                    id.clone(),
                    LayerField::Opacity,
                    LayerPropValue::Opacity(current),
                    LayerPropValue::Opacity(opacity),
                    "o",
                ),
            )
            .expect("commit");
        }
        let mut n = 0;
        while hist.undo(&mut d).expect("undo") {
            n += 1;
        }
        assert_eq!(n, 1);
        assert_eq!(d.get_layer(&id).unwrap().opacity, 1.0);
    }

    #[test]
    fn break_merge_starts_a_new_entry() {
        let mut d = doc();
        let id = d.active_layer_id.clone();
        let mut hist = UndoManager::new();
        hist.set_document();
        hist.apply(
            &mut d,
            SetLayerPropCommand::with_label(
                id.clone(),
                LayerField::Opacity,
                LayerPropValue::Opacity(1.0),
                LayerPropValue::Opacity(0.5),
                "o",
            ),
        )
        .expect("commit");
        hist.break_merge();
        hist.apply(
            &mut d,
            SetLayerPropCommand::with_label(
                id.clone(),
                LayerField::Opacity,
                LayerPropValue::Opacity(0.5),
                LayerPropValue::Opacity(0.2),
                "o",
            ),
        )
        .expect("commit");
        assert_eq!(hist.undo_label().as_deref(), Some("o"));
        assert!(hist.undo(&mut d).expect("undo"));
        assert_eq!(d.get_layer(&id).unwrap().opacity, 0.5);
        assert!(hist.undo(&mut d).expect("undo"));
        assert_eq!(d.get_layer(&id).unwrap().opacity, 1.0);
        assert!(!hist.can_undo());
    }

    // <META - ROLE : parity with tests/model_history.test.mjs B5.4 eviction case | L46-56>
    #[test]
    fn evicts_the_oldest_but_always_keeps_one() {
        let mut d = doc();
        let id = d.active_layer_id.clone();
        let mut hist = UndoManager::with_limits(3, UNDO_MAX_BYTES);
        hist.set_document();
        for i in 0..5 {
            hist.apply(
                &mut d,
                name_cmd(
                    &id,
                    &format!("n{i}"),
                    &format!("n{}", i + 1),
                    &format!("c{i}"),
                ),
            )
            .expect("commit");
        }
        let mut n = 0;
        while hist.undo(&mut d).expect("undo") {
            n += 1;
        }
        assert_eq!(n, 3);
    }

    // <META - ROLE : parity with tests/model_history.test.mjs B5.4 dirty case | L58-72>
    #[test]
    fn dirty_and_saved_lost_semantics() {
        let mut d = doc();
        let id = d.active_layer_id.clone();
        let mut hist = UndoManager::with_limits(2, UNDO_MAX_BYTES);
        hist.set_document();
        assert!(!hist.is_dirty());
        hist.mark_saved();
        assert!(!hist.is_dirty());
        hist.apply(&mut d, name_cmd(&id, "a", "b", "c0"))
            .expect("commit");
        assert!(hist.is_dirty());
        hist.mark_saved();
        assert!(!hist.is_dirty());
        hist.apply(&mut d, name_cmd(&id, "b", "c", "c1"))
            .expect("commit");
        hist.apply(&mut d, name_cmd(&id, "c", "d", "c2"))
            .expect("commit");
        assert!(hist.is_dirty(), "the evicted entry was the saved one");
        hist.mark_saved();
        assert!(!hist.is_dirty());
    }

    #[test]
    fn a_failing_do_does_not_mutate_the_stack() {
        let mut d = doc();
        let id = d.active_layer_id.clone();
        let mut hist = UndoManager::new();
        hist.set_document();
        hist.apply(&mut d, name_cmd(&id, "a", "b", "ok"))
            .expect("commit");
        let err = hist.apply(&mut d, FailingCommand).expect_err("must fail");
        assert_eq!(err.code, crate::errors::CODE_INVALID_STATE);
        assert_eq!(hist.undo_label().as_deref(), Some("ok"));
        assert!(!hist.can_redo());
    }

    // <META - ROLE : parity with tests/model_history.test.mjs B5.4 order case | L74-98>
    #[test]
    fn commit_notifies_in_order_and_tolerates_failures() {
        let mut d = doc();
        let id = d.active_layer_id.clone();
        let mut hist = UndoManager::new();
        hist.set_document();
        let order = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let a = order.clone();
        hist.subscribe(move |n| {
            a.borrow_mut().push(format!("first:{}", n.kind.as_str()));
            Ok(())
        });
        hist.subscribe(|_| Err(DrawToolError::invalid_state("boom")));
        let c = order.clone();
        hist.subscribe(move |n| {
            c.borrow_mut().push(format!("third:{}", n.kind.as_str()));
            Ok(())
        });
        hist.apply(&mut d, name_cmd(&id, "a", "b", "c"))
            .expect("commit");
        assert_eq!(&*order.borrow(), &["first:commit", "third:commit"]);
        assert!(hist.undo(&mut d).expect("undo"));
        assert_eq!(order.borrow()[2..].to_vec(), &["first:undo", "third:undo"]);
        assert!(hist.can_redo());
        hist.apply(&mut d, name_cmd(&id, "b", "c", "c2"))
            .expect("commit");
        assert!(!hist.can_redo(), "a commit clears the redo stack");
    }

    #[test]
    fn clear_resets_everything_and_notifies() {
        let mut d = doc();
        let id = d.active_layer_id.clone();
        let mut hist = UndoManager::new();
        hist.set_document();
        hist.mark_saved();
        hist.apply(&mut d, name_cmd(&id, "a", "b", "c"))
            .expect("commit");
        assert!(hist.is_dirty());
        let seen = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let inner = std::rc::Rc::clone(&seen);
        let sub = hist.subscribe(move |n| {
            if n.kind == NotifyKind::Clear {
                inner.set(inner.get() + 1);
            }
            Ok(())
        });
        hist.clear();
        assert_eq!(seen.get(), 1);
        assert!(!hist.can_undo());
        assert!(!hist.can_redo());
        assert!(!hist.is_dirty());
        assert_eq!(hist.total_bytes, 0);
        // `last_effects` survives a clear, matching the JS subscription skip.
        assert!(hist.last_effects().is_some());
        hist.unsubscribe(sub);
        hist.clear();
        // The listener is gone, so `seen` stays at 1.
        assert_eq!(seen.get(), 1);
    }

    #[test]
    fn undo_of_nothing_is_false_not_an_error() {
        let mut d = doc();
        let mut hist = UndoManager::new();
        assert!(!hist.undo(&mut d).expect("undo"));
        assert!(!hist.redo(&mut d).expect("redo"));
        assert_eq!(hist.undo_label(), None);
        assert_eq!(hist.state(), HistoryState::default());
    }

    #[test]
    fn defaults_are_the_contract_numbers() {
        let h = UndoManager::new();
        assert_eq!(h.limit_steps, UNDO_MAX_STEPS);
        assert_eq!(h.limit_bytes, UNDO_MAX_BYTES);
    }
}
