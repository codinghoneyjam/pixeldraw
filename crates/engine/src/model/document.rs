//! `Document` — canvas invariants, layer ownership and the constructor.
//!
//! Ported 1:1 from `src/features/document/document.js`. The thing that matters
//! most here is the **guard order** in [`Document::new`]: which check fires first
//! decides the error code a caller sees, and the JS order is not the order a
//! fresh reader would guess.
//!
//! The canvas triple and the viewport live in [`crate::model::canvas`], the id
//! counter in [`crate::model::ids`], both split out so this module stays readable.

use crate::constants::{
    DEFAULT_DOC_BACKGROUND, DEFAULT_DOC_HEIGHT_PX, DEFAULT_DOC_WIDTH_PX, MAX_LAYERS,
    is_valid_canvas_size,
};
use crate::errors::{
    CODE_ACTIVE_LAYER_MISSING, CODE_CANVAS_SIZE_INVALID, CODE_DUP_LAYER_ID, CODE_LAYER_LIMIT,
    DrawToolError, EngineResult,
};
use crate::model::canvas::{CanvasInfo, validate_background, validate_viewport};
use crate::model::ids::{IdGen, next_doc_id};
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
        validate_background(&canvas.background)?;
        let viewport = validate_viewport(canvas.viewport, canvas.width_px, canvas.height_px)?;
        if layers.is_empty() || layers.len() > MAX_LAYERS {
            return Err(DrawToolError::new(
                CODE_LAYER_LIMIT,
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

    fn check_store_fits(&self, layer: &Layer) -> EngineResult<()> {
        self_check_store_fits(layer, self.canvas.width_px, self.canvas.height_px)
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
                CODE_LAYER_LIMIT,
                "layer limit 64 reached",
            ));
        }
        self.check_store_fits(&layer)?;
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

    pub fn rename(&mut self, name: &str) -> EngineResult<()> {
        validate_layer_name(name)?;
        self.name = name.to_string();
        Ok(())
    }
}

/// The store-size guard, as a free function so both [`Document::new`] and
/// [`Document::insert`] can use it without reborrowing the canvas.
fn self_check_store_fits(layer: &Layer, width_px: u32, height_px: u32) -> EngineResult<()> {
    if layer.store.width_px != width_px || layer.store.height_px != height_px {
        return Err(DrawToolError::invalid_state(
            "layer store size must equal canvas size",
        ));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    // <META - ROLE : parity with tests/model_basic.test.mjs B5.1 defaults | L19-20>
    #[test]
    fn new_document_defaults() {
        let doc = new_document(NewDocumentOptions::default()).expect("valid");
        assert_eq!(doc.canvas.width_px, 512);
        assert_eq!(doc.canvas.height_px, 512);
        assert_eq!(doc.canvas.background, "transparent");
        assert_eq!(doc.canvas.viewport, None);
        assert_eq!(doc.name, "Untitled");
        assert_eq!(doc.layers.len(), 1);
        assert_eq!(doc.layers[0].name, "Layer 1");
        assert_eq!(doc.active_layer_id, doc.layers[0].id);
        assert_eq!(doc.ids.counter(), 1);
        // The document counter is module-global and monotonic, so only the
        // shape is pinned here (other tests bump it in parallel).
        assert!(doc.id.starts_with("doc-"));
        assert!(doc.id["doc-".len()..].chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn new_document_rejects_a_bad_canvas_and_background() {
        assert_eq!(
            new_document(NewDocumentOptions::default().size(31, 32))
                .unwrap_err()
                .code,
            CODE_CANVAS_SIZE_INVALID
        );
        assert!(new_document(NewDocumentOptions::default().background("nope")).is_err());
        assert!(new_document(NewDocumentOptions::default().background("#ff0000")).is_ok());
        let vp = crate::model::Viewport {
            x: 0,
            y: 0,
            w: 64,
            h: 64,
        };
        assert!(new_document(NewDocumentOptions::default().size(32, 32).viewport(vp)).is_err());
        assert!(new_document(NewDocumentOptions::default().id("doc-x")).is_ok());
    }

    #[test]
    fn remove_re_points_the_active_layer_the_way_the_js_did() {
        let mut doc = new_document(NewDocumentOptions::default().size(32, 32)).expect("valid");
        let second = Layer::create(doc.ids.next(), "Second", 32, 32).expect("valid");
        doc.insert(second.clone(), 1).expect("ok");
        doc.set_active(&second.id).expect("present");
        let (removed, index) = doc.remove(&second.id).expect("removed");
        assert_eq!(removed.id, second.id);
        assert_eq!(index, 1);
        // The layer that slid into the hole becomes active.
        assert_eq!(doc.active_layer_id, doc.layers[0].id);
        let last = doc.active_layer_id.clone();
        assert!(doc.remove(&last).is_err());
    }

    #[test]
    fn insert_guard_order_matches_the_js() {
        let mut doc = new_document(NewDocumentOptions::default().size(32, 32)).expect("valid");
        let dup = Layer::create(doc.layers[0].id.as_str(), "dup", 32, 32).expect("valid");
        assert_eq!(doc.insert(dup, 0).unwrap_err().code, CODE_DUP_LAYER_ID);
        let wrong_size = Layer::create(doc.ids.next(), "big", 64, 64).expect("valid");
        assert_eq!(
            doc.insert(wrong_size, 0).unwrap_err().code,
            crate::errors::CODE_INVALID_STATE
        );
        // Appending at exactly `layers.len()` is the JS "index === undefined"
        // equivalent; past the end is an `OUT_OF_RANGE`, not a clamp.
        let good = Layer::create(doc.ids.next(), "good", 32, 32).expect("valid");
        assert!(doc.insert(good, doc.layers.len()).is_ok());
        assert_eq!(doc.layers.len(), 2);
        let too_far = Layer::create(doc.ids.next(), "far", 32, 32).expect("valid");
        assert_eq!(
            doc.insert(too_far, 9).unwrap_err().code,
            crate::errors::CODE_OUT_OF_RANGE
        );
    }

    #[test]
    fn set_active_and_canvas_size_guards() {
        let mut doc = new_document(NewDocumentOptions::default().size(32, 32)).expect("valid");
        assert_eq!(
            doc.set_active("layer-9999").unwrap_err().code,
            CODE_ACTIVE_LAYER_MISSING
        );
        assert_eq!(
            doc.set_canvas_size(33, 32).unwrap_err().code,
            CODE_CANVAS_SIZE_INVALID
        );
        doc.set_canvas_size(64, 32).expect("valid");
        assert_eq!(doc.canvas.width_px, 64);
        assert_eq!(
            doc.move_layer("nope", 0).unwrap_err().code,
            crate::errors::CODE_LAYER_NOT_FOUND
        );
        let any = doc.active_layer_id.clone();
        assert!(doc.move_layer(&any, 9).is_err());
        doc.rename("renamed").expect("valid");
        assert_eq!(doc.name, "renamed");
        assert!(doc.rename("  ").is_err());
        doc.set_background("#00ff00").expect("valid");
        assert_eq!(doc.canvas.background, "#00ff00");
    }

    // <META - ROLE : parity with document.js L84-95 per-layer guard order | L84-95>
    #[test]
    fn the_document_constructor_checks_layers_before_the_active_id() {
        let canvas = CanvasInfo {
            width_px: 32,
            height_px: 32,
            ..CanvasInfo::default()
        };
        let l0 = Layer::create("layer-0001", "L0", 32, 32).expect("valid");
        let duplicate = Layer::create("layer-0001", "dup", 32, 32).expect("valid");
        // A duplicate id is reported before a missing active layer is looked for.
        assert_eq!(
            Document::new(DocumentSpec {
                active_layer_id: "layer-9999".into(),
                layers: vec![l0.clone(), duplicate],
                canvas: canvas.clone(),
                ..DocumentSpec::default()
            })
            .unwrap_err()
            .code,
            CODE_DUP_LAYER_ID
        );
        // A wrong-sized store is caught per layer.
        let big = Layer::create("layer-0002", "big", 64, 64).expect("valid");
        assert_eq!(
            Document::new(DocumentSpec {
                active_layer_id: "layer-0002".into(),
                layers: vec![big],
                canvas: canvas.clone(),
                ..DocumentSpec::default()
            })
            .unwrap_err()
            .code,
            crate::errors::CODE_INVALID_STATE
        );
        // Then the layer count, then the active id.
        assert_eq!(
            Document::new(DocumentSpec {
                active_layer_id: "layer-0001".into(),
                layers: vec![],
                canvas: canvas.clone(),
                ..DocumentSpec::default()
            })
            .unwrap_err()
            .code,
            CODE_LAYER_LIMIT
        );
        assert_eq!(
            Document::new(DocumentSpec {
                active_layer_id: "layer-9999".into(),
                layers: vec![l0],
                canvas,
                ..DocumentSpec::default()
            })
            .unwrap_err()
            .code,
            CODE_ACTIVE_LAYER_MISSING
        );
    }

    #[test]
    fn layer_sources_mirror_the_bottom_to_top_order() {
        let mut doc = new_document(NewDocumentOptions::default().size(32, 32)).expect("valid");
        let extra = Layer::create(doc.ids.next(), "top", 32, 32).expect("valid");
        doc.insert(extra, 1).expect("ok");
        doc.layers[1].blend = "screen";
        let sources = doc.layer_sources();
        assert_eq!(sources.len(), 2);
        // Array order = the compositor's order, so the last entry paints last.
        assert_eq!(sources[0].mode, "normal");
        assert_eq!(sources[1].mode, "screen");
        assert!(doc.active_layer().is_ok());
        assert!(doc.active_layer_mut().is_ok());
        // `insert` does not move the active layer.
        assert_eq!(doc.active_index(), 0);
        assert_eq!(doc.index_of(&doc.layers[1].id.clone()), Some(1));
    }
}
