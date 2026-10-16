//! `features/shape/shape.js` — the pending shape state machine.
//!
//! Facade: tool state in `core`, transitions in `transitions`, pointer verbs
//! in `pointer`, and the [`Tool`](crate::tools::api::Tool) impl in `lifecycle`.

mod core;
mod lifecycle;
mod pointer;
mod transitions;

pub(crate) use core::Mode;
pub use core::{PendingShape, ShapeTool};
