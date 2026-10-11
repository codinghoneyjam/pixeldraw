//! Composite cache scaffolding for Phase 2.
//!
//! The actual invalidation and cache policy is implemented later by the display
//! and composite pipeline tasks; this stub keeps the module graph stable.

/// Minimal placeholder for the composite cache map used by the renderer.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct CompCache {
    pub generation: u64,
}

impl CompCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn invalidate_all(&mut self) {
        self.generation += 1;
    }
}
