//! `Document` and its constructor arguments — the data, not the guards.
//!
//! The guard sequences live in [`super::validate`]; this module owns the shapes
//! and the accessors, which carry no ordering surprises (`docs/model_internals.md` §5).

use crate::constants::{
    DEFAULT_DOC_BACKGROUND, DEFAULT_DOC_HEIGHT_PX, DEFAULT_DOC_WIDTH_PX, is_valid_canvas_size,
};
use crate::errors::{CODE_CANVAS_SIZE_INVALID, DrawToolError, EngineResult};
use crate::model::canvas::{CanvasInfo, validate_background, validate_viewport};
use crate::model::ids::IdGen;
use crate::model::ids::next_doc_id;
use crate::model::layer::{Layer, validate_layer_name};

/// The JS constructor's destructured argument object.
#[derive(Debug, Clone, Default)]
pub struct DocumentSpec {
    pub id: String,
    pub name: String,
    pub canvas: CanvasInfo,
    pub layers: Vec<Layer>,
    pub active_layer_id: String,
    pub ids: IdGen,
}

/// A document: one canvas, 1..=64 bottom-to-top layers, one active layer.
#[derive(Debug, Clone)]
pub struct Document {
    pub id: String,
    pub name: String,
    pub canvas: CanvasInfo,
    pub layers: Vec<Layer>,
    pub active_layer_id: String,
    pub ids: IdGen,
}

impl Document {
    /// The layer with `id`, or `LAYER_NOT_FOUND`.
    pub fn get_layer(&self, id: &str) -> EngineResult<&Layer> {
        self.find_layer(id)
            .ok_or_else(|| DrawToolError::layer_not_found(id))
    }

    pub fn get_layer_mut(&mut self, id: &str) -> EngineResult<&mut Layer> {
        self.layers
            .iter_mut()
            .find(|l| l.id == id)
            .ok_or_else(|| DrawToolError::layer_not_found(id))
    }

    pub fn find_layer(&self, id: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// Position in the bottom-to-top array, `None` when absent.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    /// The JS `get activeLayer()` getter.
    pub fn active_layer(&self) -> EngineResult<&Layer> {
        self.get_layer(&self.active_layer_id)
    }

    pub fn active_layer_mut(&mut self) -> EngineResult<&mut Layer> {
        let id = self.active_layer_id.clone();
        self.get_layer_mut(&id)
    }

    /// Position of the active layer.
    ///
    /// The document invariant guarantees the active id is present, so the
    /// impossible branch yields 0 rather than panicking.
    pub fn active_index(&self) -> usize {
        self.index_of(&self.active_layer_id).unwrap_or(0)
    }

    /// The compositor's view of the whole stack, bottom → top.
    ///
    /// This is what makes `model` consumable by [`crate::composite`] without the
    /// compositor knowing a `Document` exists.
    pub fn layer_sources(&self) -> Vec<crate::composite::LayerSource<'_>> {
        self.layers.iter().map(Layer::as_source).collect()
    }

    pub fn rename(&mut self, name: &str) -> EngineResult<()> {
        validate_layer_name(name)?;
        self.name = name.to_string();
        Ok(())
    }

    pub fn set_background(&mut self, v: &str) -> EngineResult<()> {
        validate_background(v)?;
        self.canvas.background = v.to_string();
        Ok(())
    }

    /// `_setViewport(v)` — re-validated against the **current** canvas size, so
    /// a resize that leaves the viewport out of range must clear or move it
    /// first.
    pub fn set_viewport(
        &mut self,
        v: Option<crate::model::Viewport>,
    ) -> EngineResult<Option<crate::model::Viewport>> {
        let vp = validate_viewport(v, self.canvas.width_px, self.canvas.height_px)?;
        self.canvas.viewport = vp;
        Ok(vp)
    }
}

/// `NewDocumentOptions` — the JS `newDocument({...})` argument, with its
/// defaults filled in lazily so an option the caller omitted still reads as the
/// JS default.
#[derive(Debug, Clone)]
pub struct NewDocumentOptions {
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
    pub background: Option<String>,
    pub viewport: Option<crate::model::Viewport>,
    pub name: Option<String>,
    pub id: Option<String>,
}

impl Default for NewDocumentOptions {
    fn default() -> Self {
        Self {
            width_px: Some(DEFAULT_DOC_WIDTH_PX),
            height_px: Some(DEFAULT_DOC_HEIGHT_PX),
            background: Some(DEFAULT_DOC_BACKGROUND.to_string()),
            viewport: None,
            name: Some("Untitled".to_string()),
            id: None,
        }
    }
}

impl NewDocumentOptions {
    pub fn size(mut self, width_px: u32, height_px: u32) -> Self {
        self.width_px = Some(width_px);
        self.height_px = Some(height_px);
        self
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn background(mut self, background: &str) -> Self {
        self.background = Some(background.to_string());
        self
    }

    pub fn viewport(mut self, viewport: crate::model::Viewport) -> Self {
        self.viewport = Some(viewport);
        self
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }
}

/// `newDocument({widthPx=512, heightPx=512, background="transparent",
/// viewport=null, name="Untitled", id})`.
///
/// The first layer is `Layer 1` and is active; the returned `Document` owns a
/// fresh [`IdGen`] already advanced past it.
pub fn new_document(opts: NewDocumentOptions) -> EngineResult<Document> {
    let width_px = opts.width_px.unwrap_or(DEFAULT_DOC_WIDTH_PX);
    let height_px = opts.height_px.unwrap_or(DEFAULT_DOC_HEIGHT_PX);
    if !is_valid_canvas_size(width_px, height_px) {
        return Err(DrawToolError::new(
            CODE_CANVAS_SIZE_INVALID,
            format!("invalid canvas size {width_px}x{height_px}"),
        ));
    }
    let background = opts
        .background
        .unwrap_or_else(|| DEFAULT_DOC_BACKGROUND.to_string());
    validate_background(&background)?;
    validate_viewport(opts.viewport, width_px, height_px)?;
    let name = opts.name.unwrap_or_else(|| "Untitled".to_string());

    let mut ids = IdGen::new();
    let first_id = ids.next();
    let layer = Layer::create(&first_id, "Layer 1", width_px, height_px)?;
    let id = opts.id.unwrap_or_else(next_doc_id);
    Document::new(DocumentSpec {
        id,
        name,
        canvas: CanvasInfo {
            width_px,
            height_px,
            background,
            viewport: opts.viewport,
        },
        layers: vec![layer],
        active_layer_id: first_id,
        ids,
    })
}
