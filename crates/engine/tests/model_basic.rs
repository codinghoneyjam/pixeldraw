//! `<META - FILE SUMMARY - B5.1/B5.2/B5.3/B5.5, ported from tests/model_basic.test.mjs>`
//!
//! The document-model gate. Every assertion below is the JS one, rewritten
//! against the Rust API; the `<META - ROLE>` comments name the JS case so a
//! reader can diff the two.
//!
//! **Two adaptations the borrow checker forced**, both noted inline:
//!
//! 1. A listener has to be installed *before* `begin_edit`, because the handle
//!    borrows the whole `Session` for its lifetime. The JS could call
//!    `addEventListener` between `writer.set` and `commit`; here that would be a
//!    second `&mut` borrow. Nothing dispatches in that window anyway.
//! 2. `edit.commit()` after `edit.cancel()` is a **compile error**: `cancel`
//!    consumes the handle, so the JS's runtime `"edit is not active"` check has
//!    nothing left to guard.

use pixel_engine::chunkstore::{ChunkStore, PixelWriter};
use pixel_engine::constants::CHUNK_LEN;
use pixel_engine::events::{
    EVENT_DOCUMENT_REPLACED, EVENT_HISTORY_CHANGED, EVENT_LAYERS_CHANGED, EVENT_PIXELS_CHANGED,
    EVENT_SETTINGS_CHANGED, EVENT_STATUS_MESSAGE,
};
use pixel_engine::model::{
    AddLayerCommand, Command, Document, EditOptions, EventDetail, IdGen, Layer, LayerField,
    LayerPropValue, MoveLayerCommand, NewDocumentOptions, PaintCommand, RemoveLayerCommand,
    ResizeCanvasCommand, Session, SetLayerPropCommand, Subscription, add_layer, new_document,
};
use pixel_engine::pixel::pack_rgba;
use std::cell::RefCell;
use std::rc::Rc;

/// `assertDrawError(fn, code)`.
macro_rules! assert_draw_error {
    ($code:literal, $body:expr) => {
        match $body {
            Err(e) => assert_eq!(
                e.code, $code,
                "expected DrawToolError({}), got {}: {}",
                $code, e.code, e.message
            ),
            Ok(_) => panic!("expected DrawToolError({}), got Ok", $code),
        }
    };
}

/// `snapshotStore(store)` — deep copy of every populated chunk.
fn snapshot_store(store: &ChunkStore) -> Vec<(u32, u32, Vec<u8>)> {
    let mut out = Vec::new();
    store.for_each_chunk(|cx, cy, data| out.push((cx, cy, data.to_vec())));
    out
}

/// `paintPixel(doc, layerId, x, y, packed)`.
fn paint_pixel(doc: &mut Document, layer_id: &str, x: u32, y: u32, packed: u32) -> PaintCommand {
    let store = &mut doc.get_layer_mut(layer_id).expect("layer").store;
    let mut writer = PixelWriter::new(store, layer_id);
    writer.set(x, y, packed);
    let cs = writer.finish().expect("expected non-null changeset");
    PaintCommand::with_default_label(cs)
}

/// `collectEvents(session, names)` — an ordered `(name, detail)` recorder.
///
/// The subscription handle is handed back so the owner can unsubscribe; the
/// JS `done()` teardown works the same way.
struct Recorder {
    log: Rc<RefCell<Vec<(&'static str, EventDetail)>>>,
}

impl Recorder {
    fn new(session: &mut Session) -> (Self, Subscription) {
        let log: Rc<RefCell<Vec<(&'static str, EventDetail)>>> = Rc::new(RefCell::new(Vec::new()));
        let inner = Rc::clone(&log);
        let sub = session.subscribe(move |e| {
            inner.borrow_mut().push((e.name, e.detail.clone()));
            Ok(())
        });
        (Self { log }, sub)
    }

    /// The names in dispatch order.
    fn kinds(&self) -> Vec<&'static str> {
        self.log.borrow().iter().map(|(n, _)| *n).collect()
    }

    fn detail(&self, i: usize) -> EventDetail {
        self.log.borrow()[i].1.clone()
    }
}

fn doc_64() -> Document {
    new_document(NewDocumentOptions::default().size(64, 64)).expect("valid")
}

fn session_32() -> Session {
    let mut s = Session::new();
    s.new_document(NewDocumentOptions::default().size(32, 32))
        .expect("new");
    s
}

// <META - ROLE : B5.1 document invariants | L17-45>
#[test]
fn document_invariants() {
    let mut doc = new_document(NewDocumentOptions::default()).expect("valid");
    let id = doc.active_layer_id.clone();
    // Rejects removing the last layer.
    assert_draw_error!(
        "INVALID_STATE",
        RemoveLayerCommand::new(id.as_str()).apply(&mut doc)
    );
    assert_eq!(doc.layers.len(), 1);

    // Rejects the 65th layer and duplicate ids.
    let mut doc = new_document(NewDocumentOptions::default()).expect("valid");
    for i in 1..64 {
        let layer = Layer::create(doc.ids.next(), format!("L{i}"), 512, 512).expect("valid");
        doc.insert(layer, doc.layers.len()).expect("insert");
    }
    assert_eq!(doc.layers.len(), 64);
    let extra = Layer::create(doc.ids.next(), "X", 512, 512).expect("valid");
    assert_draw_error!("LAYER_LIMIT", doc.insert(extra, 64));
    let dup = Layer::create(doc.layers[0].id.as_str(), "dup", 512, 512).expect("valid");
    assert_draw_error!("DUP_LAYER_ID", doc.insert(dup, 0));

    // Rejects an unknown active layer and a bad canvas.
    let mut doc = new_document(NewDocumentOptions::default()).expect("valid");
    assert_draw_error!("ACTIVE_LAYER_MISSING", doc.set_active("layer-9999"));
    assert_draw_error!("CANVAS_SIZE_INVALID", doc.set_canvas_size(33, 32));
    assert_draw_error!(
        "CANVAS_SIZE_INVALID",
        new_document(NewDocumentOptions::default().size(31, 32))
    );
    assert_draw_error!("INVALID_STATE", Layer::create("a", "   ", 32, 32));
    assert_draw_error!("INVALID_STATE", Layer::create("a", "x".repeat(129), 32, 32));
}

// <META - ROLE : B5.2 IdGen | L47-59>
#[test]
fn idgen_seeds_and_pads() {
    let mut idgen = IdGen::new();
    idgen.seed(["layer-0007", "background", "layer-0003", "shape-9"]);
    assert_eq!(idgen.next(), "layer-0008");
    assert_eq!(idgen.next(), "layer-0009");

    let mut idgen = IdGen::new();
    idgen.seed(["layer-9999"]);
    assert_eq!(idgen.next(), "layer-10000");
}

// <META - ROLE : B5.3 commands do/undo/do byte restore, paint case | L62-72>
#[test]
fn paint_restores_the_bytes_exactly() {
    let mut doc = doc_64();
    let id = doc.active_layer_id.clone();
    let mut cmd = paint_pixel(&mut doc, &id, 40, 10, pack_rgba(255.0, 0.0, 0.0, 255.0));
    let after_apply = snapshot_store(&doc.get_layer(&id).expect("layer").store);
    cmd.revert(&mut doc).expect("undo");
    assert_eq!(
        snapshot_store(&doc.get_layer(&id).expect("layer").store),
        []
    );
    cmd.apply(&mut doc).expect("do");
    assert_eq!(
        snapshot_store(&doc.get_layer(&id).expect("layer").store),
        after_apply
    );
    assert!(cmd.byte_size() >= CHUNK_LEN);
}

// <META - ROLE : B5.3 remove restores the same object at the same index | L74-87>
#[test]
fn remove_restores_the_same_object_at_the_same_index() {
    let mut doc = new_document(NewDocumentOptions::default()).expect("valid");
    let second = Layer::create(doc.ids.next(), "Second", 512, 512).expect("valid");
    doc.insert(second.clone(), 1).expect("insert");
    let mut cmd = RemoveLayerCommand::new(second.id.as_str());
    cmd.apply(&mut doc).expect("do");
    assert_eq!(doc.layers.len(), 1);
    cmd.revert(&mut doc).expect("undo");
    assert_eq!(doc.layers.len(), 2);
    assert_eq!(doc.layers[1].id, second.id);
    assert_eq!(doc.layers[1].name, second.name);
    cmd.apply(&mut doc).expect("do");
    assert_eq!(doc.layers.len(), 1);
}

// <META - ROLE : B5.3 add/move/prop/resize round-trip | L89-122>
#[test]
fn add_move_prop_resize_round_trip() {
    let mut doc = doc_64();
    let before = snapshot_store(&doc.get_layer(&doc.active_layer_id).expect("layer").store);
    let layer = Layer::create(doc.ids.next(), "Top", 64, 64).expect("valid");
    let layer_id = layer.id.clone();
    let mut add = AddLayerCommand::with_defaults(layer, 1);
    add.apply(&mut doc).expect("do");
    add.revert(&mut doc).expect("undo");
    assert_eq!(
        snapshot_store(&doc.get_layer(&doc.active_layer_id).expect("layer").store),
        before
    );
    assert_eq!(doc.layers.len(), 1);
    add.apply(&mut doc).expect("do");
    assert_eq!(doc.layers.len(), 2);

    let mut mv = MoveLayerCommand::new(layer_id.as_str(), 1, 0);
    mv.apply(&mut doc).expect("do");
    assert_eq!(doc.layers[0].id, layer_id);
    mv.revert(&mut doc).expect("undo");
    assert_eq!(doc.layers[1].id, layer_id);

    let mut prop = SetLayerPropCommand::with_label(
        layer_id.as_str(),
        LayerField::Name,
        LayerPropValue::Name("Top".into()),
        LayerPropValue::Name("Renamed".into()),
        "x",
    );
    prop.apply(&mut doc).expect("do");
    assert_eq!(doc.get_layer(&layer_id).expect("layer").name, "Renamed");
    prop.revert(&mut doc).expect("undo");
    assert_eq!(doc.get_layer(&layer_id).expect("layer").name, "Top");

    let mut paint = paint_pixel(
        &mut doc,
        &layer_id,
        60,
        60,
        pack_rgba(0.0, 255.0, 0.0, 255.0),
    );
    paint.apply(&mut doc).expect("do");
    let mut resize = ResizeCanvasCommand::new(32, 32).expect("valid");
    resize.apply(&mut doc).expect("shrink");
    assert_eq!(doc.canvas.width_px, 32);
    assert_eq!(
        doc.get_layer(&layer_id)
            .expect("layer")
            .store
            .get_pixel(60, 60),
        0
    );
    resize.revert(&mut doc).expect("undo");
    assert_eq!(doc.canvas.width_px, 64);
    assert_eq!(
        doc.get_layer(&layer_id)
            .expect("layer")
            .store
            .get_pixel(60, 60),
        pack_rgba(0.0, 255.0, 0.0, 255.0)
    );
}

// <META - ROLE : B5.5 emits pixels -> pixels -> history order on record | L126-138>
#[test]
fn commit_emits_pixels_then_pixels_then_history() {
    let mut session = session_32();
    let (rec, sub) = Recorder::new(&mut session);
    let active = session.doc().expect("doc").active_layer_id.clone();
    let mut edit = session.begin_edit(EditOptions::default()).expect("edit");
    edit.writer()
        .expect("editing")
        .set(1, 1, pack_rgba(255.0, 0.0, 0.0, 255.0));
    assert!(edit.commit(None).expect("commit"));
    session.unsubscribe(sub);
    assert_eq!(
        rec.kinds(),
        vec![
            EVENT_PIXELS_CHANGED,
            EVENT_PIXELS_CHANGED,
            EVENT_HISTORY_CHANGED,
        ]
    );
    // `log[1].detail.layerId` and `log[1].detail.chunks`.
    match rec.detail(1) {
        EventDetail::PixelsChunks { layer_id, chunks } => {
            assert_eq!(layer_id, active);
            assert!(!chunks.is_empty());
        }
        other => panic!("expected PixelsChunks, got {other:?}"),
    }
    assert!(session.history().can_undo());
}

// <META - ROLE : B5.5 duplicate of a painted layer emits in order | L140-152>
#[test]
fn duplicate_of_a_painted_layer_emits_in_order() {
    let mut session = session_32();
    let mut edit = session.begin_edit(EditOptions::default()).expect("edit");
    edit.writer()
        .expect("editing")
        .set(0, 0, pack_rgba(1.0, 2.0, 3.0, 255.0));
    edit.commit(None).expect("commit");

    let (rec, sub) = Recorder::new(&mut session);
    let active = session.doc().expect("doc").active_layer_id.clone();
    session.duplicate_layer(&active).expect("dup");
    session.unsubscribe(sub);
    assert_eq!(
        rec.kinds(),
        vec![
            EVENT_LAYERS_CHANGED,
            EVENT_PIXELS_CHANGED,
            EVENT_HISTORY_CHANGED,
        ]
    );
    // `log[0].detail.reason === "add"` and `log[1].detail.all === true`.
    match rec.detail(0) {
        EventDetail::Layers { reason, layer_ids } => {
            assert_eq!(reason.as_str(), "add");
            assert_eq!(layer_ids.len(), 1);
        }
        other => panic!("expected Layers, got {other:?}"),
    }
    assert!(matches!(rec.detail(1), EventDetail::PixelsAll { .. }));
}

// <META - ROLE : B5.5 load/new emit only replaced + history | L154-172>
#[test]
fn load_and_new_emit_only_replaced_then_history() {
    let mut session = Session::new();
    session
        .new_document(NewDocumentOptions::default())
        .expect("new");
    let (rec, sub) = Recorder::new(&mut session);
    session
        .new_document(NewDocumentOptions::default())
        .expect("new");
    session
        .load_document(
            new_document(NewDocumentOptions::default()).expect("valid"),
            true,
        )
        .expect("load");
    session.unsubscribe(sub);
    assert_eq!(
        rec.kinds(),
        vec![
            EVENT_DOCUMENT_REPLACED,
            EVENT_HISTORY_CHANGED,
            EVENT_DOCUMENT_REPLACED,
            EVENT_HISTORY_CHANGED,
        ]
    );
}

// <META - ROLE : B5.5 locked/hidden beginEdit notifies and throws | L174-194>
#[test]
fn locked_and_hidden_begin_edit_notifies_then_throws() {
    let mut session = session_32();
    let errors = Rc::new(RefCell::new(Vec::<String>::new()));
    let inner = Rc::clone(&errors);
    let status_sub = session.subscribe(move |e| {
        if e.name == EVENT_STATUS_MESSAGE
            && let EventDetail::Status { code, .. } = &e.detail
        {
            inner.borrow_mut().push((*code).to_string());
        }
        Ok(())
    });

    let id = session.doc().expect("doc").active_layer_id.clone();
    session.set_layer_locked(&id, true).expect("lock");
    assert_draw_error!("LAYER_LOCKED", session.begin_edit(EditOptions::default()));
    assert_eq!(
        errors.borrow().last().map(String::as_str),
        Some("LAYER_LOCKED")
    );

    session.set_layer_locked(&id, false).expect("unlock");
    session.set_layer_visible(&id, false).expect("hide");
    assert_draw_error!("LAYER_HIDDEN", session.begin_edit(EditOptions::default()));
    assert_eq!(
        errors.borrow().last().map(String::as_str),
        Some("LAYER_HIDDEN")
    );
    session.set_layer_visible(&id, true).expect("show");
    session.unsubscribe(status_sub);

    let edit = session.begin_edit(EditOptions::default()).expect("edit");
    assert!(edit.is_active());
    // The JS kept the object alive and called `session.undo()` against it.
    // Dropping the handle is the Rust equivalent of abandoning the JS object:
    // the session stays `editing` and refuses the undo, emitting nothing.
    drop(edit);
    assert!(session.is_editing());

    let (rec, hist_sub) = Recorder::new(&mut session);
    assert!(!session.undo().expect("undo"));
    assert!(rec.kinds().is_empty());
    session.unsubscribe(hist_sub);
}

// <META - ROLE : B5.5 validates settings and skips the event when unchanged | L196-211>
#[test]
fn settings_validate_and_skip_unchanged() {
    use pixel_engine::model::SettingInput;
    let mut session = Session::new();
    let count = Rc::new(RefCell::new(0usize));
    let inner = Rc::clone(&count);
    let sub = session.subscribe(move |e| {
        if e.name == EVENT_SETTINGS_CHANGED {
            *inner.borrow_mut() += 1;
        }
        Ok(())
    });

    assert_draw_error!(
        "OUT_OF_RANGE",
        session.set_setting("penSize", SettingInput::Num(200.0))
    );
    assert_draw_error!(
        "OUT_OF_RANGE",
        session.set_setting("primaryColor", SettingInput::Str("red"))
    );
    assert_draw_error!(
        "OUT_OF_RANGE",
        session.set_setting("activeTool", SettingInput::Str("airbrush"))
    );
    assert_draw_error!(
        "INVALID_STATE",
        session.set_setting("nope", SettingInput::Num(1.0))
    );

    assert!(
        !session
            .set_setting("penSize", SettingInput::Num(1.0))
            .expect("same")
    );
    assert_eq!(*count.borrow(), 0);
    assert!(
        session
            .set_setting("penSize", SettingInput::Num(8.0))
            .expect("change")
    );
    assert_eq!(*count.borrow(), 1);
    assert_eq!(session.settings().pen_size, 8);

    session
        .set_setting("primaryColor", SettingInput::Str("#FF0000"))
        .expect("change");
    assert_eq!(session.settings().primary_color, "#ff0000");
    session.unsubscribe(sub);
}

// <META - ROLE : B5.5 cancel restores pixels | L213-226>
#[test]
fn cancel_restores_the_pixels() {
    let mut session = session_32();
    let (rec, sub) = Recorder::new(&mut session);
    let mut edit = session.begin_edit(EditOptions::default()).expect("edit");
    edit.writer()
        .expect("editing")
        .set(5, 5, pack_rgba(9.0, 9.0, 9.0, 255.0));
    edit.cancel();
    session.unsubscribe(sub);
    let id = session.doc().expect("doc").active_layer_id.clone();
    assert!(!session.is_editing());
    assert_eq!(
        session
            .doc()
            .expect("doc")
            .get_layer(&id)
            .expect("layer")
            .store
            .get_pixel(5, 5),
        0
    );
    assert_eq!(rec.kinds(), vec![EVENT_PIXELS_CHANGED]);
    // `edit.commit()` here is a borrow error: `cancel` consumed the handle, so
    // the JS runtime "edit is not active" is a compile-time fact in Rust.
}

// <META - ROLE : B5.5 opacity drag merges until final | L228-239>
#[test]
fn opacity_drag_merges_until_final() {
    let mut session = session_32();
    let id = session.doc().expect("doc").active_layer_id.clone();
    session.set_layer_opacity(&id, 0.5, false).expect("set");
    session.set_layer_opacity(&id, 0.3, false).expect("set");
    session.set_layer_opacity(&id, 0.1, true).expect("set");
    assert_eq!(
        session
            .doc()
            .expect("doc")
            .get_layer(&id)
            .expect("layer")
            .opacity,
        0.1
    );
    assert!(session.undo().expect("undo"));
    assert_eq!(
        session
            .doc()
            .expect("doc")
            .get_layer(&id)
            .expect("layer")
            .opacity,
        1.0
    );
    assert!(!session.history().can_undo());
}

// <META - ROLE : the JS `addLayer` default name, layer_operations.js L16-26 | L16-26>
#[test]
fn added_layers_get_the_js_default_name() {
    let mut s = session_32();
    assert_eq!(add_layer(&mut s, None).expect("add"), "layer-0002");
    let id = add_layer(&mut s, None).expect("add");
    let doc = s.doc().expect("doc");
    assert_eq!(doc.layers.len(), 3);
    assert_eq!(doc.get_layer(&id).expect("layer").name, "Layer 3");
}

// <META - ROLE : the history state the JS `_historyState()` published | L71-79>
#[test]
fn history_state_matches_the_js_detail_shape() {
    let mut session = session_32();
    let state = session.history().state();
    assert!(!state.can_undo);
    assert!(!state.can_redo);
    assert_eq!(state.undo_label, None);
    assert_eq!(state.redo_label, None);
    assert!(!state.dirty);
    session
        .set_layer_opacity(
            &session.doc().expect("doc").active_layer_id.clone(),
            0.5,
            false,
        )
        .expect("set");
    let state = session.history().state();
    assert!(state.can_undo);
    assert_eq!(state.undo_label.as_deref(), Some("레이어 불투명도"));
    assert!(state.dirty);
}
