//! `Document` — canvas invariants, layer ownership and the constructor. Facade.
//!
//! Ported 1:1 from `src/features/document/document.js`. The thing that matters
//! most here is the **guard order** in [`Document::new`]: which check fires first
//! decides the error code a caller sees, and the JS order is not the order a
//! fresh reader would guess. The full reasoning is in
//! `docs/model_internals.md` §5.
//!
//! The canvas triple and the viewport live in [`crate::model::canvas`], the id
//! counter in [`crate::model::ids`], both split out so this module stays readable.

mod types;
mod validate;

#[cfg(test)]
mod tests;

pub use types::{Document, DocumentSpec, NewDocumentOptions, new_document};
pub use validate::self_check_store_fits;
