//! `Session` — the document, its settings, its history and the event bus. Facade.
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
//! `ops` carries the twelve one-line delegates to
//! [`crate::model::operations`]. The JS `_lastCmd` (written by a history
//! subscription that re-dispatched `_dispatchEffects(this._lastCmd)` after
//! undo/redo) lives on the manager as
//! [`crate::model::UndoManager::last_effects`], because a session cannot
//! subscribe to a field of itself. The reasoning is `docs/model_internals.md` §7.

mod core;
mod ops;

#[cfg(test)]
mod tests;

pub use core::{Session, SessionCore};
