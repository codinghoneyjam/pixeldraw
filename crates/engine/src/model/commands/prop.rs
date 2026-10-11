//! `SetLayerPropCommand` — the only command that merges.
//!
//! Ported 1:1 from `commands.js`. The merge policy exists so a whole opacity
//! drag collapses into one history entry. Only the opacity field participates;
//! a name/visible/locked change stays its own step however many times it fires
//! (`docs/model_internals.md` §6).

use crate::errors::EngineResult;
use crate::model::command::{Command, CommandEffects, LayerChangeReason};
use crate::model::document::Document;
use crate::model::layer::{LayerField, LayerPropValue};

/// Change one of the five layer properties.
#[derive(Debug, Clone)]
pub struct SetLayerPropCommand {
    pub layer_id: String,
    pub field: LayerField,
    pub before: LayerPropValue,
    pub after: LayerPropValue,
    pub label: String,
}

impl SetLayerPropCommand {
    /// JS default label: `"레이어 속성 변경"`.
    pub fn new(
        layer_id: impl Into<String>,
        field: LayerField,
        before: LayerPropValue,
        after: LayerPropValue,
    ) -> Self {
        Self::with_label(layer_id, field, before, after, "레이어 속성 변경")
    }

    pub fn with_label(
        layer_id: impl Into<String>,
        field: LayerField,
        before: LayerPropValue,
        after: LayerPropValue,
        label: impl Into<String>,
    ) -> Self {
        Self {
            layer_id: layer_id.into(),
            field,
            before,
            after,
            label: label.into(),
        }
    }
}

impl Command for SetLayerPropCommand {
    fn label(&self) -> &str {
        &self.label
    }

    fn type_name(&self) -> &'static str {
        "set-layer-prop"
    }

    fn supports_merge(&self) -> bool {
        true
    }

    fn apply(&mut self, doc: &mut Document) -> EngineResult<()> {
        doc.get_layer_mut(&self.layer_id)?
            .set_prop(self.field, self.after.clone())
    }

    fn revert(&mut self, doc: &mut Document) -> EngineResult<()> {
        doc.get_layer_mut(&self.layer_id)?
            .set_prop(self.field, self.before.clone())
    }

    fn byte_size(&self) -> usize {
        0
    }

    fn effects(&self) -> CommandEffects {
        CommandEffects::structure(LayerChangeReason::Prop, vec![self.layer_id.clone()])
    }

    fn merge_with(&mut self, prev: &mut dyn Command) -> bool {
        let Some(prev) = prev.as_any().downcast_mut::<SetLayerPropCommand>() else {
            return false;
        };
        if prev.layer_id != self.layer_id || prev.field != self.field {
            return false;
        }
        // Only opacity merges: a name/visible/locked change must stay its own
        // history step, however many times it fires.
        if self.field != LayerField::Opacity {
            return false;
        }
        prev.after = self.after.clone();
        true
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
