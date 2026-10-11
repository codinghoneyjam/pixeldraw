//! `PaintCommand` — one stroke's chunk diff.
//!
//! Ported 1:1 from `commands.js`. Nothing here has an ordering surprise; the
//! `Chunks` effect shape it reports is [`crate::model::command::PixelEffect`].

use crate::chunkstore::Changeset;
use crate::errors::EngineResult;
use crate::model::command::{Command, CommandEffects, PixelEffect};
use crate::model::document::Document;

/// `chunkList(cs)` — the `(cx, cy)` pairs of a changeset.
fn chunk_list(cs: &Changeset) -> Vec<(u32, u32)> {
    cs.chunks.iter().map(|(cx, cy, _, _)| (*cx, *cy)).collect()
}

/// One stroke's worth of pixels.
#[derive(Debug, Clone)]
pub struct PaintCommand {
    pub change_set: Changeset,
    pub label: String,
}

impl PaintCommand {
    pub fn new(change_set: Changeset, label: impl Into<String>) -> Self {
        Self {
            change_set,
            label: label.into(),
        }
    }

    /// JS default label: `constructor(changeSet, label = "연필")`.
    pub fn with_default_label(change_set: Changeset) -> Self {
        Self::new(change_set, "연필")
    }
}

impl Command for PaintCommand {
    fn label(&self) -> &str {
        &self.label
    }

    fn type_name(&self) -> &'static str {
        "paint"
    }

    fn apply(&mut self, doc: &mut Document) -> EngineResult<()> {
        let layer = doc.get_layer_mut(&self.change_set.layer_id)?;
        for (cx, cy, _, after) in &self.change_set.chunks {
            layer
                .store
                .put_chunk(*cx, *cy, after.as_deref().map(|c| &c[..]))?;
        }
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> EngineResult<()> {
        let layer = doc.get_layer_mut(&self.change_set.layer_id)?;
        for (cx, cy, before, _) in &self.change_set.chunks {
            layer
                .store
                .put_chunk(*cx, *cy, before.as_deref().map(|c| &c[..]))?;
        }
        Ok(())
    }

    fn byte_size(&self) -> usize {
        self.change_set.byte_size()
    }

    fn effects(&self) -> CommandEffects {
        CommandEffects {
            layers: None,
            pixels: vec![PixelEffect::Chunks {
                layer_id: self.change_set.layer_id.clone(),
                chunks: chunk_list(&self.change_set),
            }],
        }
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
