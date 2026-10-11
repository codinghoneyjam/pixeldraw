use super::core::Session;
use crate::model::document::NewDocumentOptions;
use crate::model::edit_session::EditOptions;
use crate::model::settings::Settings;

// Keeps the imports honest: `SetLayerPropCommand`/`PaintCommand`/
// `MoveLayerCommand`/`ResizeCanvasCommand` all belong to the surface the
// session re-exports through `crate::model`.
#[test]
fn sessions_start_empty_and_settings_start_at_the_defaults() {
    let mut s = Session::new();
    assert!(s.doc().is_none());
    assert!(!s.is_editing());
    assert_eq!(s.settings(), Settings::default());
    assert!(s.require_doc().is_err());
    // No document and nothing to undo: the JS returned `false` here too.
    assert!(!s.undo().expect("no document to undo into"));
    assert!(!s.redo().expect("no document to redo into"));
    assert_eq!(s.history().state(), crate::model::HistoryState::default());
}

#[test]
fn new_document_dispatches_replaced_then_history() {
    let mut s = Session::new();
    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::<&'static str>::new()));
    let a = log.clone();
    s.subscribe(move |e| {
        a.borrow_mut().push(e.name);
        Ok(())
    });
    s.new_document(NewDocumentOptions::default().size(32, 32))
        .expect("new");
    assert_eq!(
        &*log.borrow(),
        &[
            crate::events::EVENT_DOCUMENT_REPLACED,
            crate::events::EVENT_HISTORY_CHANGED,
        ]
    );
    assert!(s.doc().is_some());
    assert!(!s.history().can_undo());
    assert_eq!(s.history().state(), crate::model::HistoryState::default());
    assert_eq!(s.listener_count(), 1);
}

#[test]
fn stroke_begin_then_commit_matches_the_edit_handle_flow() {
    let mut s = Session::new();
    s.new_document(NewDocumentOptions::default().size(32, 32))
        .unwrap();
    let layer_id = s.doc().unwrap().active_layer_id.clone();

    s.stroke_begin(EditOptions::on_layer(&layer_id).labeled("연필"))
        .expect("stroke begin");
    assert!(s.has_stroke());
    assert_eq!(s.stroke_layer_id(), Some(layer_id.as_str()));

    let changed = s
        .stroke_with(|writer| {
            writer.set(0, 0, 0xff0000ff);
            writer.get(0, 0)
        })
        .expect("stroke_with");
    assert_eq!(changed, 0xff0000ff);

    assert!(!s.stroke_flush().is_empty());
    assert!(s.stroke_commit(None).expect("stroke commit"));
    assert!(!s.is_editing());
    assert!(!s.has_stroke());
}

#[test]
fn stroke_begin_rejects_hidden_layers_and_tracks_editing_guard() {
    let mut s = Session::new();
    s.new_document(NewDocumentOptions::default().size(32, 32))
        .unwrap();
    let layer_id = s.doc().unwrap().active_layer_id.clone();
    s.set_layer_visible(&layer_id, false).unwrap();

    let err = s
        .stroke_begin(EditOptions::on_layer(&layer_id))
        .unwrap_err();
    assert_eq!(err.code, crate::errors::CODE_LAYER_HIDDEN);
    assert!(!s.has_stroke());

    s.set_layer_visible(&layer_id, true).unwrap();
    s.stroke_begin(EditOptions::on_layer(&layer_id)).unwrap();
    assert!(s.is_editing());
    assert!(s.stroke_begin(EditOptions::on_layer(&layer_id)).is_err());
    s.stroke_cancel();
    assert!(!s.is_editing());
    assert!(!s.has_stroke());
}

#[test]
fn undo_and_redo_walk_the_stroke_and_property_stacks() {
    let mut s = Session::new();
    s.new_document(NewDocumentOptions::default().size(32, 32))
        .expect("new");
    let id = s.doc().unwrap().active_layer_id.clone();
    s.set_layer_visible(&id, false).expect("visible");
    assert!(
        s.begin_edit(EditOptions::default()).is_err(),
        "a hidden layer refuses an edit"
    );
    s.set_layer_visible(&id, true).expect("visible");
    let mut edit = s.begin_edit(EditOptions::default()).expect("edit");
    assert!(edit.is_active());
    edit.writer().expect("editing").set(1, 1, 0xff0000ff);
    assert!(edit.commit(None).expect("commit"));
    assert!(!s.is_editing());
    // Three entries: the two visibility toggles and the stroke.
    let mut n = 0;
    while s.undo().expect("undo") {
        n += 1;
    }
    assert_eq!(n, 3);
    assert!(s.redo().expect("redo"));
    assert!(s.redo().expect("redo"));
    assert!(s.redo().expect("redo"));
    assert!(!s.redo().expect("nothing left"));
}
