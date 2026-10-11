//! `MergeDownCommand` — bake the upper layer into the lower and drop the upper.
//!
//! Ported 1:1 from `commands_pixel.js`'s merge half. The general contract is in
//! `docs/model_internals.md` §4 (what the bake makes permanent, and the ordering
//! of the flag guards). Only the two things the signature cannot carry stay here:
//! why `merge_pixel` composites through a transparent backdrop first, and why
//! the chunk loop lives outside the command.

use super::resize::CutChunk;
use crate::blend::over_blend;
use crate::chunkstore::{Chunk, ChunkStore, is_all_zero};
use crate::constants::{CHUNK_LEN, CHUNK_PX};
use crate::errors::{CODE_LAYER_HIDDEN, CODE_LAYER_LOCKED, DrawToolError, EngineResult};
use crate::model::command::{Command, CommandEffects, LayerChangeReason, LayerEffect, PixelEffect};
use crate::model::document::Document;
use crate::model::layer::Layer;
use crate::pixel::Rgba;

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
    lower: &ChunkStore,
    upper: &ChunkStore,
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
    lower_before: Vec<CutChunk>,
    upper: Option<Layer>,
    upper_index: usize,
    prev_active_id: Option<String>,
}

impl MergeDownCommand {
    /// JS default label: `"?래래 병합"`.
    pub fn new(upper_id: impl Into<String>) -> Self {
        Self {
            upper_id: upper_id.into(),
            label: "?래래 병합".to_string(),
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
