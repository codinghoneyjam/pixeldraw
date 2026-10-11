//! The constructor's and `insert`'s guard sequences.
//!
//! Ported 1:1 from `src/features/document/document.js`. **The order is the
//! behaviour**: a document holding two faults reports the one the JS loop hit
//! first, so `[valid, wrong-size, duplicate]` raises `DUP_LAYER_ID`, not
//! `INVALID_STATE`. The JS did it in one pass; splitting that into two passes
//! is exactly the mistake that three tests caught (`docs/model_internals.md` §5).

use crate::constants::{MAX_LAYERS, is_valid_canvas_size};
use crate::errors::{
    CODE_ACTIVE_LAYER_MISSING, CODE_CANVAS_SIZE_INVALID, CODE_DUP_LAYER_ID, DrawToolError,
    EngineResult,
};
use crate::model::canvas::CanvasInfo;
use crate::model::layer::Layer;

use super::types::{Document, DocumentSpec};

impl Document {
    /// The JS constructor, including its guard **order**:
    /// canvas size → background → viewport → layer count → per-layer checks
    /// (duplicate id, then store size) → the active id.
    pub fn new(spec: DocumentSpec) -> EngineResult<Self> {
        let DocumentSpec {
            id,
            name,
            canvas,
            layers,
            active_layer_id,
            ids,
        } = spec;
        if !is_valid_canvas_size(canvas.width_px, canvas.height_px) {
            return Err(DrawToolError::new(
                CODE_CANVAS_SIZE_INVALID,
                "invalid canvas size",
            ));
        }
        crate::model::canvas::validate_background(&canvas.background)?;
        let viewport = crate::model::canvas::validate_viewport(
            canvas.viewport,
            canvas.width_px,
            canvas.height_px,
        )?;
        if layers.is_empty() || layers.len() > MAX_LAYERS {
            return Err(DrawToolError::new(
                crate::errors::CODE_LAYER_LIMIT,
                "document must hold 1-64 layers",
            ));
        }
        let mut seen: Vec<&str> = Vec::with_capacity(layers.len());
        for layer in &layers {
            // Duplicate id first, then the store size — a document holding both
            // faults reports `DUP_LAYER_ID`, exactly as the JS loop did.
            if seen.contains(&layer.id.as_str()) {
                return Err(DrawToolError::new(
                    CODE_DUP_LAYER_ID,
                    format!("duplicate layer id {}", layer.id),
                ));
            }
            seen.push(&layer.id);
            self_check_store_fits(layer, canvas.width_px, canvas.height_px)?;
        }
        if !seen.contains(&active_layer_id.as_str()) {
            return Err(DrawToolError::new(
                CODE_ACTIVE_LAYER_MISSING,
                format!("active layer {active_layer_id} missing"),
            ));
        }
        Ok(Self {
            id,
            name,
            canvas: CanvasInfo { viewport, ..canvas },
            layers,
            active_layer_id,
            ids,
        })
    }

    /// `_insert(layer, index)`.
    ///
    /// The JS guard order matters: duplicate id, then the 64-layer limit, then
    /// the store-size check, then the index range — so inserting a duplicate
    /// into a full document still reports `DUP_LAYER_ID`.
    pub fn insert(&mut self, layer: Layer, index: usize) -> EngineResult<()> {
        if self.find_layer(&layer.id).is_some() {
            return Err(DrawToolError::new(
                CODE_DUP_LAYER_ID,
                format!("duplicate layer id {}", layer.id),
            ));
        }
        if self.layers.len() >= MAX_LAYERS {
            return Err(DrawToolError::new(
                crate::errors::CODE_LAYER_LIMIT,
                "layer limit 64 reached",
            ));
        }
        check_store_fits(&self.canvas, &layer)?;
        if index > self.layers.len() {
            return Err(DrawToolError::out_of_range(format!(
                "insert index {index} out of range"
            )));
        }
        self.layers.insert(index, layer);
        Ok(())
    }

    /// `_remove(id)` — returns the removed layer and the index it left.
    ///
    /// Removing the active layer re-points `active_layer_id` at
    /// `min(index, len-1)` *after* the splice: the layer that slid into the
    /// hole, or the new last one when the removed one was the bottom of the
    /// stack.
    pub fn remove(&mut self, id: &str) -> EngineResult<(Layer, usize)> {
        let index = self
            .index_of(id)
            .ok_or_else(|| DrawToolError::layer_not_found(id))?;
        if self.layers.len() <= 1 {
            return Err(DrawToolError::invalid_state("cannot remove the last layer"));
        }
        let layer = self.layers.remove(index);
        if self.active_layer_id == id {
            let next = self.layers[index.min(self.layers.len() - 1)].id.clone();
            self.active_layer_id = next;
        }
        Ok((layer, index))
    }

    /// `_move(id, toIndex)`.
    pub fn move_layer(&mut self, id: &str, to_index: usize) -> EngineResult<()> {
        let from = self
            .index_of(id)
            .ok_or_else(|| DrawToolError::layer_not_found(id))?;
        if to_index >= self.layers.len() {
            return Err(DrawToolError::out_of_range(format!(
                "move index {to_index} out of range"
            )));
        }
        let layer = self.layers.remove(from);
        self.layers.insert(to_index, layer);
        Ok(())
    }

    /// `_setActive(id)` — `ACTIVE_LAYER_MISSING`, not a silent no-op.
    pub fn set_active(&mut self, id: &str) -> EngineResult<()> {
        if self.find_layer(id).is_none() {
            return Err(DrawToolError::new(
                CODE_ACTIVE_LAYER_MISSING,
                format!("active layer {id} missing"),
            ));
        }
        self.active_layer_id = id.to_string();
        Ok(())
    }

    /// `_setCanvasSize(w, h)` — only the canvas bookkeeping.
    ///
    /// Resizing the layer stores is [`crate::model::ResizeCanvasCommand`]'s job,
    /// because it has to snapshot what falls off the edge first.
    pub fn set_canvas_size(&mut self, w: u32, h: u32) -> EngineResult<()> {
        if !is_valid_canvas_size(w, h) {
            return Err(DrawToolError::new(
                CODE_CANVAS_SIZE_INVALID,
                format!("invalid canvas size {w}x{h}"),
            ));
        }
        self.canvas.width_px = w;
        self.canvas.height_px = h;
        Ok(())
    }
}

/// The store-size guard for `insert`.
///
/// Takes the canvas rather than `self` so the borrow of `self.layers` from the
/// mutating insert can still proceed.
pub(super) fn check_store_fits(canvas: &CanvasInfo, layer: &Layer) -> EngineResult<()> {
    if layer.store.width_px != canvas.width_px || layer.store.height_px != canvas.height_px {
        return Err(DrawToolError::invalid_state(
            "layer store size must equal canvas size",
        ));
    }
    Ok(())
}

/// The store-size guard, as a free function so both [`Document::new`] and
/// [`Document::insert`] can use it without reborrowing the canvas.
pub fn self_check_store_fits(layer: &Layer, width_px: u32, height_px: u32) -> EngineResult<()> {
    if layer.store.width_px != width_px || layer.store.height_px != height_px {
        return Err(DrawToolError::invalid_state(
            "layer store size must equal canvas size",
        ));
    }
    Ok(())
}
