use super::*;

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
    drop(edit);
    assert!(session.is_editing());

    let (rec, hist_sub) = Recorder::new(&mut session);
    assert!(!session.undo().expect("undo"));
    assert!(rec.kinds().is_empty());
    session.unsubscribe(hist_sub);
}
