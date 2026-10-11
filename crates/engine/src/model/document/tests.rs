use super::types::{Document, DocumentSpec, NewDocumentOptions, new_document};
use crate::model::canvas::CanvasInfo;
use crate::model::layer::Layer;

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
        crate::errors::CODE_CANVAS_SIZE_INVALID
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
    assert_eq!(
        doc.insert(dup, 0).unwrap_err().code,
        crate::errors::CODE_DUP_LAYER_ID
    );
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
        crate::errors::CODE_ACTIVE_LAYER_MISSING
    );
    assert_eq!(
        doc.set_canvas_size(33, 32).unwrap_err().code,
        crate::errors::CODE_CANVAS_SIZE_INVALID
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
        crate::errors::CODE_DUP_LAYER_ID
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
        crate::errors::CODE_LAYER_LIMIT
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
        crate::errors::CODE_ACTIVE_LAYER_MISSING
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
