//! Feature façades — one entry point per tool feature.
//!
//! Each module owns the tool implementation plus its option-bar block, so a
//! feature can be read in one place. `tools::…` and `ui::optionbar::…` keep
//! their old paths as re-exports of these until every caller has migrated.

pub mod arc;
pub mod eyedropper;
pub mod fill;
pub mod gradient;
pub mod hand;
pub mod pen;
pub mod shape;
pub mod text;
