//! The pixel-rewriting commands.
//!
//! Ported 1:1 from `src/features/layers/commands_pixel.js`. These two rewrite
//! chunk contents rather than just layer structure, which is why the JS tree
//! split them out of `commands.js` with their own chunk/blend imports; the same
//! line is drawn here.

use crate::blend::over_blend;
use crate::chunkstore::{Chunk, is_all_zero};
use crate::constants::{CHUNK_LEN, CHUNK_PX, is_valid_canvas_size};
use crate::errors::{
    CODE_CANVAS_SIZE_INVALID, CODE_LAYER_HIDDEN, CODE_LAYER_LOCKED, DrawToolError, EngineResult,
};
use crate::model::command::{Command, CommandEffects, LayerChangeReason, LayerEffect, PixelEffect};
use crate::model::document::Document;
use crate::model::layer::Layer;
use crate::pixel::Rgba;

// <META - ROLE : mergePixel from commands_pixel.js L13-16 | L13-16>
/// Composite `lower` under `upper` through both opacities and the upper's blend
/// mode: `lower over transparent` first, then `upper` over that.
///
/// The intermediate pass is not an optimisation — layering the lower over a
/// transparent backdrop first is what produces `over(mid, upper)`, and doing it
/// the other way round gives a different colour wherever the lower is
/// translucent.
fn merge_pixel(
    lower: Rgba,
    lower_opacity: f64,
    upper: Rgba,
    upper_opacity: f64,
    upper_blend: &str,
) -> Rgba {
    let mid = over_blend([0, 0, 0, 0], lower, lower_opacity, "normal");
    over_blend(mid, upper, upper_opacity, upper_blend)
}

/// What one merged chunk decided, mirroring the JS `do` loop's three exits.
enum Merged {
    /// Both halves absent — the JS `if (!ld && !ud) continue`, no write at all.
    Skip,
    /// Written as `null` (the JS `putChunk(cx, cy, null)`), which deletes.
    Cleared,
    /// A new buffer to store.
    Written(Box<Chunk>),
}

/// The inner chunk loop of `MergeDownCommand::apply`.
///
/// Kept out of the command so the borrows of `lower.store` / `upper.store` end
/// before `put_chunk` needs `&mut lower.store`.
fn merge_chunks(
    lower: &crate::chunkstore::ChunkStore,
    upper: &crate::chunkstore::ChunkStore,
    lower_opacity: f64,
    upper_opacity: f64,
    upper_blend: &str,
    cx: u32,
    cy: u32,
) -> Merged {
    let ld = lower.get_chunk(cx, cy);
    let ud = upper.get_chunk(cx, cy);
    if ld.is_none() && ud.is_none() {
        return Merged::Skip;
    }
    let mut out = Box::new([0u8; CHUNK_LEN]);
    let mut touched = false;
    for i in 0..(CHUNK_PX * CHUNK_PX) as usize {
        let o = i * 4;
        let lr: Rgba = match ld {
            Some(d) => [d[o], d[o + 1], d[o + 2], d[o + 3]],
            None => [0, 0, 0, 0],
        };
        let ur: Rgba = match ud {
            Some(d) => [d[o], d[o + 1], d[o + 2], d[o + 3]],
            None => [0, 0, 0, 0],
        };
        if lr[3] == 0 && ur[3] == 0 {
            continue;
        }
        let merged = merge_pixel(lr, lower_opacity, ur, upper_opacity, upper_blend);
        if merged[3] == 0 {
            continue;
        }
        // The JS then did `packRGBA(r,g,b,a)` and `unpackRGBA(p)` before
        // writing; both are the identity on values that `overBlend` has already
        // clamped, and `a === 0` was skipped above, so the bytes are copied
        // straight across.
        out[o..o + 4].copy_from_slice(&merged);
        touched = true;
    }
    if touched && !is_all_zero(&out) {
        Merged::Written(out)
    } else {
        Merged::Cleared
    }
}

/// Merge the layer above into the layer below and remove the upper one.
///
/// **What is permanent is the merge itself**, not the data: `do` bakes the
/// upper's blend and opacity into the lower's pixels, so the blend *relation* is
/// gone. Setting the lower layer's blend to `multiply` afterwards re-blends the
/// merged region too, and you cannot get the two-layer look back — you have to
/// undo the merge. Undo restores both layers exactly (the comment in the JS
/// says the same, and `blend_modes.test.mjs` pins both halves).
#[derive(Debug, Clone)]
pub struct MergeDownCommand {
    pub upper_id: String,
    pub label: String,
    lower_id: Option<String>,
    lower_prev_opacity: f64,
    lower_before: Vec<(u32, u32, Box<Chunk>)>,
    upper: Option<Layer>,
    upper_index: usize,
    prev_active_id: Option<String>,
}

impl MergeDownCommand {
    /// JS default label: `"?�래�?병합"`.
    pub fn new(upper_id: impl Into<String>) -> Self {
        Self {
            upper_id: upper_id.into(),
            label: "?�래�?병합".to_string(),
            lower_id: None,
            lower_prev_opacity: 1.0,
            lower_before: Vec::new(),
            upper: None,
            upper_index: 0,
            prev_active_id: None,
        }
    }

    /// `_resolve(doc)` — the upper, the layer under it, and its index.
    fn resolve(&self, doc: &Document) -> EngineResult<usize> {
        let upper_index = doc
            .index_of(&self.upper_id)
            .ok_or_else(|| DrawToolError::layer_not_found(&self.upper_id))?;
        if upper_index == 0 {
            return Err(DrawToolError::invalid_state("no layer below to merge into"));
        }
        Ok(upper_index)
    }

    /// `_checkFlags(upper, lower)` — locked wins over hidden.
    fn check_flags(upper: &Layer, lower: &Layer) -> EngineResult<()> {
        if upper.locked || lower.locked {
            return Err(DrawToolError::new(
                CODE_LAYER_LOCKED,
                "cannot merge locked layer",
            ));
        }
        if !upper.visible || !lower.visible {
            return Err(DrawToolError::new(
                CODE_LAYER_HIDDEN,
                "cannot merge hidden layer",
            ));
        }
        Ok(())
    }
}

impl Command for MergeDownCommand {
    fn label(&self) -> &str {
        &self.label
    }

    fn type_name(&self) -> &'static str {
        "merge-down"
    }

    fn apply(&mut self, doc: &mut Document) -> EngineResult<()> {
        let upper_index = self.resolve(doc)?;
        // Locked/hidden are checked before anything is snapshotted, so a
        // rejected merge leaves the layers byte-identical.
        Self::check_flags(&doc.layers[upper_index], &doc.layers[upper_index - 1])?;

        self.prev_active_id = Some(doc.active_layer_id.clone());
        self.upper_index = upper_index;

        // Snapshot the lower layer's chunks before anything is written.
        let (lower_id, lower_opacity, lower_before) = {
            let lower = &doc.layers[upper_index - 1];
            let mut before = Vec::new();
            lower
                .store
                .for_each_chunk(|cx, cy, data| before.push((cx, cy, Box::new(*data))));
            (lower.id.clone(), lower.opacity, before)
        };
        self.lower_id = Some(lower_id);
        self.lower_prev_opacity = lower_opacity;
        self.lower_before = lower_before;

        {
            let (head, tail) = doc.layers.split_at_mut(upper_index);
            let lower = &mut head[upper_index - 1];
            let upper = &tail[0];
            let w = doc.canvas.width_px;
            let h = doc.canvas.height_px;
            for cy in 0..h / CHUNK_PX {
                for cx in 0..w / CHUNK_PX {
                    match merge_chunks(
                        &lower.store,
                        &upper.store,
                        lower.opacity,
                        upper.opacity,
                        upper.blend,
                        cx,
                        cy,
                    ) {
                        Merged::Skip => {}
                        Merged::Cleared => lower.store.put_chunk(cx, cy, None)?,
                        Merged::Written(chunk) => {
                            lower.store.put_chunk(cx, cy, Some(&chunk[..]))?
                        }
                    }
                }
            }
            // The lower opacity is baked into the pixels, so it becomes 1.
            lower.opacity = 1.0;
        }

        let lower_id = self.lower_id.clone().unwrap_or_default();
        self.upper = Some(doc.remove(&self.upper_id)?.0);
        doc.set_active(&lower_id)
    }

    fn revert(&mut self, doc: &mut Document) -> EngineResult<()> {
        let lower_id = self
            .lower_id
            .clone()
            .ok_or_else(|| DrawToolError::invalid_state("merge was never applied"))?;
        {
            let lower = doc.get_layer_mut(&lower_id)?;
            for cy in 0..lower.store.chunks_y {
                for cx in 0..lower.store.chunks_x {
                    lower.store.put_chunk(cx, cy, None)?;
                }
            }
            for (cx, cy, data) in &self.lower_before {
                lower.store.put_chunk(*cx, *cy, Some(&data[..]))?;
            }
            lower.opacity = self.lower_prev_opacity;
        }
        let upper = self
            .upper
            .take()
            .ok_or_else(|| DrawToolError::invalid_state("merge was never applied"))?;
        doc.insert(upper, self.upper_index)?;
        let prev = self.prev_active_id.as_deref().unwrap_or("");
        doc.set_active(prev)
    }

    fn byte_size(&self) -> usize {
        let lower_bytes = self.lower_before.len() * CHUNK_LEN;
        let upper_bytes = self
            .upper
            .as_ref()
            .map_or(0, |u| u.store.chunk_count() * CHUNK_LEN);
        lower_bytes + upper_bytes
    }

    fn effects(&self) -> CommandEffects {
        CommandEffects {
            layers: Some(LayerEffect {
                reason: LayerChangeReason::Remove,
                layer_ids: vec![self.upper_id.clone()],
            }),
            pixels: vec![PixelEffect::All {
                layer_id: self.lower_id.clone().unwrap_or_default(),
            }],
        }
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// One layer's worth of chunks that fell off the edge during a resize, in
/// ascending key order.
///
/// The `CutLayer` alias follows the `ChunkDiff` precedent in `chunkstore.rs`,
/// for the same reason: clippy's `type_complexity` fires on the nested tuple
/// long before a human finds it hard to read.
pub type CutChunk = (u32, u32, Box<Chunk>);

/// `(layerId, removed chunks)` — the JS `_cut` entry shape.
pub type CutLayer = (String, Vec<CutChunk>);

/// Resize the canvas and every layer store, keeping what fell off the edge.
///
/// The interesting asymmetry: `do` resizes each store to the new size and
/// records what dropped out, while `undo` resizes back and puts it in again.
/// Resizing is not a round-trip in general — a chunk that was partly outside
/// both sizes is restored only from the snapshot, never from a re-scan.
#[derive(Debug, Clone)]
pub struct ResizeCanvasCommand {
    pub new_w: u32,
    pub new_h: u32,
    pub label: String,
    old_w: u32,
    old_h: u32,
    cut: Vec<CutLayer>,
    captured: bool,
    layer_ids: Vec<String>,
}

impl ResizeCanvasCommand {
    /// JS default label: `"캔버스 크기 변경"`.
    ///
    /// The size is validated in the constructor, so an impossible command
    /// cannot even be built.
    pub fn new(new_w: u32, new_h: u32) -> EngineResult<Self> {
        if !is_valid_canvas_size(new_w, new_h) {
            return Err(DrawToolError::new(
                CODE_CANVAS_SIZE_INVALID,
                format!("invalid canvas size {new_w}x{new_h}"),
            ));
        }
        Ok(Self {
            new_w,
            new_h,
            label: "캔버스 크기 변경".to_string(),
            old_w: 0,
            old_h: 0,
            cut: Vec::new(),
            captured: false,
            layer_ids: Vec::new(),
        })
    }
}

impl Command for ResizeCanvasCommand {
    fn label(&self) -> &str {
        &self.label
    }

    fn type_name(&self) -> &'static str {
        "resize-canvas"
    }

    fn apply(&mut self, doc: &mut Document) -> EngineResult<()> {
        // `_captured` means a redo keeps the *original* old size, not the size
        // the previous undo left behind.
        if !self.captured {
            self.old_w = doc.canvas.width_px;
            self.old_h = doc.canvas.height_px;
            self.captured = true;
        }
        self.cut = Vec::new();
        self.layer_ids = doc.layers.iter().map(|l| l.id.clone()).collect();
        for layer in doc.layers.iter_mut() {
            let removed = layer.store.resize_to(self.new_w, self.new_h)?;
            if !removed.is_empty() {
                self.cut.push((layer.id.clone(), removed));
            }
        }
        doc.set_canvas_size(self.new_w, self.new_h)
    }

    fn revert(&mut self, doc: &mut Document) -> EngineResult<()> {
        doc.set_canvas_size(self.old_w, self.old_h)?;
        for layer in doc.layers.iter_mut() {
            layer.store.resize_to(self.old_w, self.old_h)?;
        }
        for (layer_id, chunks) in &self.cut {
            let layer = doc.get_layer_mut(layer_id)?;
            for (cx, cy, data) in chunks {
                layer.store.put_chunk(*cx, *cy, Some(&data[..]))?;
            }
        }
        Ok(())
    }

    fn byte_size(&self) -> usize {
        self.cut
            .iter()
            .map(|(_, chunks)| chunks.len() * CHUNK_LEN)
            .sum()
    }

    fn effects(&self) -> CommandEffects {
        CommandEffects {
            layers: None,
            pixels: self
                .layer_ids
                .iter()
                .map(|id| PixelEffect::All {
                    layer_id: id.clone(),
                })
                .collect(),
        }
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend::over;
    use crate::model::document::{NewDocumentOptions, new_document};
    use crate::model::layer::Layer;
    use crate::pixel::unpack_rgba;

    const RED: u32 = 0xff0000ff;
    const WHITE: u32 = 0xffffffff;
    /// Black is `0x000000ff`, not the red I first pasted in — that swapped the
    /// merge from "white over black" to "white over red" and the golden value
    /// `[128,128,128]` caught it.
    const BLACK: u32 = 0xff000000;

    fn doc() -> Document {
        new_document(NewDocumentOptions::default().size(32, 32)).expect("valid")
    }

    /// The `paintPixel` helper of `tests/helpers/model.js`, against a raw store.
    fn dot(store: &mut crate::chunkstore::ChunkStore, x: u32, y: u32, packed: u32) {
        store.set_pixel(x, y, packed);
    }

    // <META - ROLE : parity with tests/model_history.test.mjs B5.6 merge cases | L126-199>
    #[test]
    fn merge_down_matches_the_composite_and_round_trips() {
        let mut d = doc();
        let lower_id = d.active_layer_id.clone();
        dot(&mut d.layers[0].store, 0, 0, BLACK);
        let mut upper = Layer::create(d.ids.next(), "U", 32, 32).expect("ok");
        let upper_id = upper.id.clone();
        dot(&mut upper.store, 0, 0, WHITE);
        d.insert(upper, 1).expect("ok");
        d.layers[1].opacity = 1.0;
        d.layers[0].opacity = 1.0;

        let mut cmd = MergeDownCommand::new(upper_id);
        Command::apply(&mut cmd, &mut d).expect("merge");
        // The JS expectation: `over(over(transparent, black, 1), white, 1)`.
        let got = unpack_rgba(d.get_layer(&lower_id).unwrap().store.get_pixel(0, 0));
        assert_eq!(
            got,
            over(
                over([0, 0, 0, 0], [0, 0, 0, 255], 1.0),
                [255, 255, 255, 255],
                1.0
            )
        );
        assert_eq!(d.layers.len(), 1);

        Command::revert(&mut cmd, &mut d).expect("undo");
        assert_eq!(d.layers.len(), 2);
        assert_eq!(
            unpack_rgba(d.get_layer(&lower_id).unwrap().store.get_pixel(0, 0)),
            [0, 0, 0, 255]
        );
        assert_eq!(
            unpack_rgba(d.get_layer(&cmd.upper_id).unwrap().store.get_pixel(0, 0)),
            [255, 255, 255, 255]
        );
        Command::apply(&mut cmd, &mut d).expect("redo");
        assert_eq!(d.layers.len(), 1);
    }

    #[test]
    fn merge_down_bakes_opacity_and_sets_it_to_one() {
        let mut d = doc();
        let lower_id = d.active_layer_id.clone();
        dot(&mut d.layers[0].store, 3, 3, BLACK);
        let mut upper = Layer::create(d.ids.next(), "upper", 32, 32).expect("ok");
        let upper_id = upper.id.clone();
        dot(&mut upper.store, 3, 3, WHITE);
        upper.opacity = 0.5;
        d.insert(upper, 1).expect("ok");
        let mut cmd = MergeDownCommand::new(upper_id);
        Command::apply(&mut cmd, &mut d).expect("merge");
        // Golden: white@0.5 over black is [128,128,128,255].
        assert_eq!(
            unpack_rgba(d.get_layer(&lower_id).unwrap().store.get_pixel(3, 3)),
            [128, 128, 128, 255]
        );
        assert_eq!(d.get_layer(&lower_id).unwrap().opacity, 1.0);
        assert_eq!(d.active_layer_id, lower_id);
    }

    #[test]
    fn merge_down_rejects_locked_hidden_and_the_bottom_layer() {
        let mut d = doc();
        let bottom = d.active_layer_id.clone();
        let top = Layer::create(d.ids.next(), "top", 32, 32).expect("ok");
        let top_id = top.id.clone();
        d.insert(top, 1).expect("ok");

        let mut m = MergeDownCommand::new(bottom.clone());
        assert_eq!(
            Command::apply(&mut m, &mut d).unwrap_err().code,
            crate::errors::CODE_INVALID_STATE
        );
        d.get_layer_mut(&top_id).unwrap().locked = true;
        let mut m = MergeDownCommand::new(top_id.clone());
        assert_eq!(
            Command::apply(&mut m, &mut d).unwrap_err().code,
            CODE_LAYER_LOCKED
        );
        d.get_layer_mut(&top_id).unwrap().locked = false;
        d.get_layer_mut(&bottom).unwrap().visible = false;
        let mut m = MergeDownCommand::new(top_id.clone());
        assert_eq!(
            Command::apply(&mut m, &mut d).unwrap_err().code,
            CODE_LAYER_HIDDEN
        );
        // Nothing moved.
        assert_eq!(d.layers.len(), 2);
    }

    #[test]
    fn merge_down_byte_size_counts_both_snapshots() {
        let mut d = doc();
        let lower_id = d.active_layer_id.clone();
        // Both layers hold a chunk, so the snapshot is one chunk each side.
        dot(&mut d.layers[0].store, 0, 0, BLACK);
        let mut upper = Layer::create(d.ids.next(), "U", 32, 32).expect("ok");
        dot(&mut upper.store, 0, 0, WHITE);
        d.insert(upper, 1).expect("ok");
        let upper_id = d.layers[1].id.clone();
        let mut cmd = MergeDownCommand::new(upper_id);
        assert_eq!(Command::byte_size(&cmd), 0);
        Command::apply(&mut cmd, &mut d).expect("merge");
        assert_eq!(Command::byte_size(&cmd), CHUNK_LEN * 2);
        // Undo restores the lower's chunk count, so the size does not drift.
        Command::revert(&mut cmd, &mut d).expect("undo");
        assert_eq!(d.get_layer(&lower_id).unwrap().store.chunk_count(), 1);
    }

    #[test]
    fn resize_canvas_round_trip_and_redo_keeps_the_original_size() {
        let mut d = new_document(NewDocumentOptions::default().size(64, 64)).expect("valid");
        let id = d.active_layer_id.clone();
        dot(&mut d.get_layer_mut(&id).unwrap().store, 60, 60, RED);
        let mut cmd = ResizeCanvasCommand::new(32, 32).expect("valid");
        assert!(ResizeCanvasCommand::new(33, 32).is_err());
        assert_eq!(cmd.label(), "캔버스 크기 변경");
        assert_eq!(cmd.type_name(), "resize-canvas");
        Command::apply(&mut cmd, &mut d).expect("shrink");
        assert_eq!(d.canvas.width_px, 32);
        assert_eq!(d.get_layer(&id).unwrap().store.get_pixel(60, 60), 0);
        assert_eq!(Command::byte_size(&cmd), CHUNK_LEN);
        Command::revert(&mut cmd, &mut d).expect("undo");
        assert_eq!(d.canvas.width_px, 64);
        assert_eq!(d.get_layer(&id).unwrap().store.get_pixel(60, 60), RED);
        // A do/undo/do cycle keeps the size captured on the first `do`.
        Command::apply(&mut cmd, &mut d).expect("redo");
        Command::revert(&mut cmd, &mut d).expect("undo again");
        assert_eq!(d.canvas.width_px, 64);
    }

    #[test]
    fn resize_canvas_effects_name_every_layer() {
        let mut d = doc();
        let mut extra = Layer::create(d.ids.next(), "S", 32, 32).expect("ok");
        dot(&mut extra.store, 0, 0, RED);
        d.insert(extra, 1).expect("ok");
        let mut cmd = ResizeCanvasCommand::new(64, 64).expect("valid");
        Command::apply(&mut cmd, &mut d).expect("grow");
        let fx = Command::effects(&cmd);
        assert!(fx.layers.is_none());
        assert_eq!(fx.pixels.len(), 2);
        assert!(
            fx.pixels
                .iter()
                .all(|p| matches!(p, PixelEffect::All { .. }))
        );
    }
}
