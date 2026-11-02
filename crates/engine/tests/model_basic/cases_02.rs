use super::*;

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
