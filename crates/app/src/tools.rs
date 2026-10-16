//! Tool façade and shared overlay API.

pub mod api;
pub mod manager;
pub mod overlay;
pub use api::*;
pub use overlay::*;

/// Shared test fixtures (`session64`, `view1`, `down`, `pixel`). Public so the
/// feature-owned test suites can reuse them.
#[cfg(test)]
pub mod tests;
