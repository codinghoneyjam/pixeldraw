//! Schema validation for document/layer JSON — facade (JS `io/validate*`).
//!
//! `structural` holds the per-object schema checks (JS
//! `validate_structural.js`) and the error collector every
//! check shares; `semantic` holds the chunk-decode checks (JS
//! `validate_semantic.js`) plus the two document-level entry
//! points (JS `validate.js`), which are the only code that
//! interleaves both kinds of check. JS parity traps
//! (check order, the 50-cap early return, `Number.isInteger`
//! over JSON, the hand-rolled regexes): `docs/io_internals.md`.

mod semantic;
mod structural;
#[cfg(test)]
mod tests;

pub use semantic::{check_chunks_semantic, validate_document, validate_layer_file};
pub use structural::{
    CanvasInfo, LayerInfo, ValidationError, ValidationReport, Viewport, check_canvas, check_chunk,
    check_layer, check_raster,
};
