//! `ResizeCanvasCommand` + the snapshot types the cut edge needs.
//!
//! Ported 1:1 from `commands_pixel.js`'s resize half. The interesting asymmetry:
//! `do` resizes each store to the new size and records what dropped out, while
//! `undo` resizes back and puts it in again. Resizing is not a round-trip in
//! general — a chunk that was partly outside both sizes is restored only from
//! the snapshot, never from a re-scan (`docs/model_internals.md` §4).

use crate::chunkstore::Chunk;
use crate::constants::{CHUNK_LEN, is_valid_canvas_size};
use crate::errors::{CODE_CANVAS_SIZE_INVALID, DrawToolError, EngineResult};
use crate::model::command::{Command, CommandEffects, PixelEffect};
use crate::model::document::Document;

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
