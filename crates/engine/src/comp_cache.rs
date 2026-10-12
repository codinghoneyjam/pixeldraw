//! Composite cache — the dirty-chunk compositor with the 6 ms frame budget.
//!
//! Ported from `src/render/renderer_composite.js` (`ensureComp`/`paintDirty`) and
//! the invalidation half of `src/render/renderer.js`. The module keeps the buffer
//! in **straight alpha** with no background: painting it is the display renderer's
//! job, exactly as in the JS tree (docs/render.md).

mod cache;
mod events;
mod paint;
#[cfg(test)]
mod tests;

pub use cache::CompCache;
pub use paint::Budget;
