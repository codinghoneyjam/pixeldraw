//! The layer-structure commands: add, remove, move.
//!
//! Ported 1:1 from `commands.js`. Two behaviours are not derivable from the
//! signatures and both are pinned by tests:
//!
//! * `AddLayerCommand::undo` **discards** the layer it removed, so a stroke
//!   painted on that layer while it was in the document does not survive the
//!   undo/redo — it needs its own `PaintCommand` in the history. `do` re-inserts
//!   this command's own copy for the same reason: the JS moved the very object
//!   the command held;
//! * `MoveLayerCommand` captures `from` because the splice makes the two indices
//!   non-interchangeable — 1→0 is not the inverse of 0→1.
//!
//! The full reasoning is `docs/model_internals.md` §6.

use crate::constants::CHUNK_LEN;
use crate::errors::{DrawToolError, EngineResult};
use crate::model::command::{Command, CommandEffects, LayerChangeReason, LayerEffect, PixelEffect};
use crate::model::document::Document;
use crate::model::layer::Layer;

/// Insert a layer, optionally making it active.
#[derive(Debug, Clone)]
pub struct AddLayerCommand {
    pub layer: Layer,
    pub index: usize,
    pub make_active: bool,
    pub label: String,
    prev_active_id: Option<String>,
}

impl AddLayerCommand {
    pub fn new(layer: Layer, index: usize, make_active: bool, label: impl Into<String>) -> Self {
        Self {
            layer,
            index,
            make_active,
            label: label.into(),
            prev_active_id: None,
        }
    }

    /// JS default: `{ makeActive = true, label = "레이어 추가" }`.
    pub fn with_defaults(layer: Layer, index: usize) -> Self {
        Self::new(layer, index, true, "레이어 추가")
    }
}

impl Command for AddLayerCommand {
    fn label(&self) -> &str {
        &self.label
    }

    fn type_name(&self) -> &'static str {
        "add-layer"
    }

    fn apply(&mut self, doc: &mut Document) -> EngineResult<()> {
        self.prev_active_id = Some(doc.active_layer_id.clone());
        doc.insert(self.layer.clone(), self.index)?;
        if self.make_active {
            doc.set_active(&self.layer.id)?;
        }
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> EngineResult<()> {
        doc.remove(&self.layer.id)?;
        // The JS `_setActive(null)` produced `ACTIVE_LAYER_MISSING` when `do`
        // had never run; an empty id produces the same code.
        let prev = self.prev_active_id.as_deref().unwrap_or("");
        doc.set_active(prev)
    }

    fn byte_size(&self) -> usize {
        self.layer.store.chunk_count() * CHUNK_LEN
    }

    fn effects(&self) -> CommandEffects {
        let pixels = if self.layer.store.chunk_count() > 0 {
            vec![PixelEffect::All {
                layer_id: self.layer.id.clone(),
            }]
        } else {
            Vec::new()
        };
        CommandEffects {
            layers: Some(LayerEffect {
                reason: LayerChangeReason::Add,
                layer_ids: vec![self.layer.id.clone()],
            }),
            pixels,
        }
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Remove a layer and put it back exactly where it was.
#[derive(Debug, Clone)]
pub struct RemoveLayerCommand {
    pub layer_id: String,
    pub label: String,
    removed: Option<(Layer, usize)>,
    prev_active_id: Option<String>,
}

impl RemoveLayerCommand {
    /// JS default label: `"레이어 삭제"`.
    pub fn new(layer_id: impl Into<String>) -> Self {
        Self {
            layer_id: layer_id.into(),
            label: "레이어 삭제".to_string(),
            removed: None,
            prev_active_id: None,
        }
    }
}

impl Command for RemoveLayerCommand {
    fn label(&self) -> &str {
        &self.label
    }

    fn type_name(&self) -> &'static str {
        "remove-layer"
    }

    fn apply(&mut self, doc: &mut Document) -> EngineResult<()> {
        self.prev_active_id = Some(doc.active_layer_id.clone());
        self.removed = Some(doc.remove(&self.layer_id)?);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> EngineResult<()> {
        let (layer, index) = self
            .removed
            .take()
            .ok_or_else(|| DrawToolError::invalid_state("remove was never applied"))?;
        doc.insert(layer, index)?;
        let prev = self.prev_active_id.as_deref().unwrap_or("");
        doc.set_active(prev)
    }

    fn byte_size(&self) -> usize {
        self.removed
            .as_ref()
            .map_or(0, |(layer, _)| layer.store.chunk_count() * CHUNK_LEN)
    }

    fn effects(&self) -> CommandEffects {
        let has_pixels = self
            .removed
            .as_ref()
            .is_some_and(|(layer, _)| layer.store.chunk_count() > 0);
        let pixels = if has_pixels {
            vec![PixelEffect::All {
                layer_id: self.layer_id.clone(),
            }]
        } else {
            Vec::new()
        };
        CommandEffects {
            layers: Some(LayerEffect {
                reason: LayerChangeReason::Remove,
                layer_ids: vec![self.layer_id.clone()],
            }),
            pixels,
        }
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Reorder one layer.
///
/// The `from` index is captured rather than recomputed on undo because the JS
/// `_move` splice makes `from` and `to` non-interchangeable: moving 1→0 is not
/// the inverse of 0→1, and by the time `undo` runs the layer has already moved.
/// ([`crate::model::operations::move_layer`] fills it in from the pre-move index.)
#[derive(Debug, Clone)]
pub struct MoveLayerCommand {
    pub layer_id: String,
    pub from: usize,
    pub to: usize,
    pub label: String,
}

impl MoveLayerCommand {
    /// JS default label: `"레이어 순서 변경"`.
    pub fn new(layer_id: impl Into<String>, from: usize, to: usize) -> Self {
        Self {
            layer_id: layer_id.into(),
            from,
            to,
            label: "레이어 순서 변경".to_string(),
        }
    }
}

impl Command for MoveLayerCommand {
    fn label(&self) -> &str {
        &self.label
    }

    fn type_name(&self) -> &'static str {
        "move-layer"
    }

    fn apply(&mut self, doc: &mut Document) -> EngineResult<()> {
        doc.move_layer(&self.layer_id, self.to)
    }

    fn revert(&mut self, doc: &mut Document) -> EngineResult<()> {
        doc.move_layer(&self.layer_id, self.from)
    }

    fn byte_size(&self) -> usize {
        0
    }

    fn effects(&self) -> CommandEffects {
        CommandEffects::structure(LayerChangeReason::Move, vec![self.layer_id.clone()])
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
