//! Structure and property commands.
//!
//! Ported 1:1 from `src/features/layers/commands.js`. The two pixel-rewriting
//! commands (`MergeDownCommand`, `ResizeCanvasCommand`) live in
//! [`crate::model::commands_pixel`], as they did in the JS tree, because they
//! carry chunk-level imports of their own.

use crate::chunkstore::Changeset;
use crate::errors::{DrawToolError, EngineResult};
use crate::model::command::{Command, CommandEffects, LayerChangeReason, LayerEffect, PixelEffect};
use crate::model::document::Document;
use crate::model::layer::{Layer, LayerField, LayerPropValue};

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

/// Insert a layer, optionally making it active.
///
/// **Deliberate surprise kept**: `undo` removes the layer from the document and
/// throws it away, then re-`do` inserts this command's own copy again. So a
/// stroke painted on the layer while it was in the document does not survive an
/// undo/redo — it needs its own `PaintCommand` in the history, which is exactly
/// how the JS behaved. The layer is inserted as a copy for that same reason: the
/// JS moved the very object the command held.
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
        self.layer.store.chunk_count() * crate::constants::CHUNK_LEN
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
        self.removed.as_ref().map_or(0, |(layer, _)| {
            layer.store.chunk_count() * crate::constants::CHUNK_LEN
        })
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

/// Reorder one layer. The `from` index is captured by the caller
/// ([`crate::model::operations::move_layer`]) because the JS `_move` splice
/// makes `from` and `to` non-interchangeable: moving 1→0 is not the inverse of
/// 0→1.
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

/// Change one of the five layer properties.
///
/// The only command in the tree that defines `mergeWith`, so a whole opacity
/// drag collapses into one history entry.
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
        Self {
            layer_id: layer_id.into(),
            field,
            before,
            after,
            label: "레이어 속성 변경".to_string(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::CHUNK_LEN;
    use crate::model::document::{NewDocumentOptions, new_document};
    use crate::pixel::pack_rgba;

    const RED: u32 = 0xff0000ff;

    fn doc() -> crate::model::document::Document {
        new_document(NewDocumentOptions::default().size(32, 32)).expect("valid")
    }

    // <META - ROLE : parity with helpers/model.js paintPixel + PaintCommand cases | L27-34>
    #[test]
    fn paint_do_undo_do_restores_the_bytes() {
        let mut d = doc();
        let id = d.active_layer_id.clone();
        let mut writer = crate::chunkstore::PixelWriter::new(
            &mut d.active_layer_mut().unwrap().store,
            id.clone(),
        );
        writer.set(1, 1, RED);
        let cs = writer.finish().expect("changeset");
        let mut cmd = PaintCommand::with_default_label(cs);
        assert_eq!(cmd.label(), "연필");
        assert_eq!(cmd.type_name(), "paint");
        assert!(cmd.byte_size() >= CHUNK_LEN);

        let pixel = d.get_layer(&id).unwrap().store.get_pixel(1, 1);
        assert_eq!(pixel, RED);
        cmd.revert(&mut d).expect("undo");
        assert_eq!(d.get_layer(&id).unwrap().store.get_pixel(1, 1), 0);
        assert_eq!(d.get_layer(&id).unwrap().store.chunk_count(), 0);
        cmd.apply(&mut d).expect("do");
        assert_eq!(d.get_layer(&id).unwrap().store.get_pixel(1, 1), RED);
    }

    #[test]
    fn add_layer_round_trips() {
        let mut d = doc();
        let first = d.layers[0].id.clone();
        let layer = crate::model::layer::Layer::create(d.ids.next(), "Second", 32, 32).expect("ok");
        let second = layer.id.clone();
        let mut cmd = AddLayerCommand::with_defaults(layer, 1);
        assert_eq!(cmd.label(), "레이어 추가");
        assert_eq!(cmd.byte_size(), 0);
        cmd.apply(&mut d).expect("ok");
        assert_eq!(d.layers.len(), 2);
        assert_eq!(d.active_layer_id, second);
        cmd.revert(&mut d).expect("ok");
        assert_eq!(d.layers.len(), 1);
        assert_eq!(d.active_layer_id, first);
        cmd.apply(&mut d).expect("ok");
        assert_eq!(d.layers.len(), 2);
    }

    #[test]
    fn duplicate_emits_all_when_the_layer_holds_pixels() {
        let mut d = doc();
        d.layers[0]
            .store
            .set_pixel(0, 0, pack_rgba(1.0, 2.0, 3.0, 255.0));
        let clone = d.layers[0]
            .clone_with(d.ids.next(), "Layer 1 copy")
            .expect("ok");
        let cmd = AddLayerCommand::with_defaults(clone, 1);
        let fx = Command::effects(&cmd);
        assert_eq!(
            fx.pixels,
            vec![PixelEffect::All {
                layer_id: cmd.layer.id.clone()
            }]
        );
        assert!(
            fx.layers
                .is_some_and(|l| l.reason == LayerChangeReason::Add)
        );
    }

    #[test]
    fn set_layer_prop_merges_only_opacity_on_the_same_layer() {
        let d = doc();
        let id = d.active_layer_id.clone();
        let mut a = SetLayerPropCommand::with_label(
            id.clone(),
            LayerField::Opacity,
            LayerPropValue::Opacity(1.0),
            LayerPropValue::Opacity(0.5),
            "o",
        );
        let mut b = SetLayerPropCommand::with_label(
            id.clone(),
            LayerField::Opacity,
            LayerPropValue::Opacity(0.5),
            LayerPropValue::Opacity(0.3),
            "o",
        );
        assert!(b.merge_with(&mut a));
        assert_eq!(a.after, LayerPropValue::Opacity(0.3));
        // A different field never merges.
        let mut named = SetLayerPropCommand::with_label(
            id.clone(),
            LayerField::Name,
            LayerPropValue::Name("a".into()),
            LayerPropValue::Name("b".into()),
            "o",
        );
        let mut opacity_new = SetLayerPropCommand::with_label(
            "layer-9999".to_string(),
            LayerField::Opacity,
            LayerPropValue::Opacity(1.0),
            LayerPropValue::Opacity(0.1),
            "o",
        );
        assert!(!opacity_new.merge_with(&mut a));
        assert!(!named.merge_with(&mut a));
        // A non-prop command never merges.
        let other = AddLayerCommand::with_defaults(
            crate::model::layer::Layer::create("layer-0009", "x", 32, 32).expect("ok"),
            1,
        );
        let mut boxed: Box<dyn Command> = Box::new(other);
        assert!(!opacity_new.merge_with(boxed.as_mut()));
    }

    #[test]
    fn move_command_swaps_by_index() {
        let mut d = doc();
        let second = crate::model::layer::Layer::create(d.ids.next(), "S", 32, 32).expect("ok");
        let sid = second.id.clone();
        d.insert(second, 1).expect("ok");
        let mut cmd = MoveLayerCommand::new(sid.clone(), 1, 0);
        cmd.apply(&mut d).expect("ok");
        assert_eq!(d.layers[0].id, sid);
        cmd.revert(&mut d).expect("ok");
        assert_eq!(d.layers[1].id, sid);
        assert_eq!(Command::byte_size(&cmd), 0);
    }
}
