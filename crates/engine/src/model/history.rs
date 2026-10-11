//! The undo history — step/byte budget and the merge policy. Facade.
//!
//! Ported 1:1 from `src/features/document/history.js`. Three behaviours a
//! signature cannot show: the budget never empties the stack, the merge policy
//! is four terms not three, and a failing `do`/`undo` puts the command back.
//! The long form is in `docs/model_internals.md` §3.

mod budget;
mod undo_redo;

#[cfg(test)]
mod tests;

pub use budget::{HistoryListener, HistoryNotify, HistoryState, NotifyKind, UndoManager};
