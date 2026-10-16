//! Pending adjustment — locked layers, handles, nudge, option-bar values.

use crate::features::shape::PendingValue;
use crate::input::build_tool_event;
use crate::tools::api::{Tool, ToolCx};

use super::{drag, key, rect_tool};
use crate::tools::tests::{down, pixel, session64, view1};

/// Locked layer: the pending shape is discarded, nothing paints.
#[test]
fn commit_on_a_locked_layer_discards_pending() {
    let (mut session, _queue) = session64();
    let id = session.doc().expect("doc").active_layer_id.clone();
    let mut tool = rect_tool();
    drag(&mut tool, &mut session, 10.0, 10.0, 20.0, 12.0);
    assert!(tool.has_pending());
    assert!(session.set_layer_locked(&id, true).expect("lock"));
    let mut render = false;
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        assert!(!tool.commit(&mut cx).expect("commit reports false"));
        assert!(!tool.has_pending(), "locked commit discards");
    }
    assert_eq!(pixel(&session, 10, 10), 0);
}

/// Resize handle drag grows the pending box.
///
/// The 6px handle tolerance overlaps on a small box — (20,12) reads as the
/// "n" handle in JS order too — so SE is grabbed from (24,18), outside every
/// other handle's box but inside SE's.
#[test]
fn resize_handle_grows_pending() {
    let (mut session, _queue) = session64();
    let mut tool = rect_tool();
    drag(&mut tool, &mut session, 10.0, 10.0, 20.0, 12.0);
    let mut render = false;
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        tool.pointer_down(&build_tool_event(&down(24.0, 18.0), &view1()), &mut cx);
        tool.pointer_move(&build_tool_event(&down(25.0, 15.0), &view1()), &mut cx);
        tool.pointer_up(&build_tool_event(&down(25.0, 15.0), &view1()), &mut cx);
    }
    match tool.pending_value() {
        Some(PendingValue::Box { x, y, w, h, .. }) => assert_eq!((x, y, w, h), (10, 10, 16, 6)),
        other => panic!("box pending after resize, got {other:?}"),
    }
}

/// Body drag moves the pending box (big box: the centre is clear of handles).
#[test]
fn body_drag_moves_pending() {
    let (mut session, _queue) = session64();
    let mut tool = rect_tool();
    drag(&mut tool, &mut session, 0.0, 0.0, 60.0, 40.0);
    let mut render = false;
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        tool.pointer_down(&build_tool_event(&down(30.0, 20.0), &view1()), &mut cx);
        tool.pointer_move(&build_tool_event(&down(35.0, 25.0), &view1()), &mut cx);
        tool.pointer_up(&build_tool_event(&down(35.0, 25.0), &view1()), &mut cx);
    }
    match tool.pending_value() {
        Some(PendingValue::Box { x, y, w, h, .. }) => assert_eq!((x, y, w, h), (5, 5, 61, 41)),
        other => panic!("box pending after move, got {other:?}"),
    }
}

/// Arrow keys nudge 1px, 32px with shift.
#[test]
fn arrow_keys_nudge_pending() {
    let (mut session, _queue) = session64();
    let mut tool = rect_tool();
    drag(&mut tool, &mut session, 10.0, 10.0, 20.0, 12.0);
    let mut render = false;
    let mut cx = ToolCx {
        session: &mut session,
        view: view1(),
        need_render: &mut render,
    };
    assert!(tool.key_down(&key("ArrowRight", false), &mut cx));
    match tool.pending_value() {
        Some(PendingValue::Box { x, .. }) => assert_eq!(x, 11),
        other => panic!("nudged, got {other:?}"),
    }
    assert!(tool.key_down(&key("ArrowDown", true), &mut cx));
    match tool.pending_value() {
        Some(PendingValue::Box { x, y, .. }) => assert_eq!((x, y), (11, 42)),
        other => panic!("shift-nudged, got {other:?}"),
    }
}

/// `set_pending(None)` discards; a box value installs pending.
#[test]
fn set_pending_round_trip() {
    let (mut session, _queue) = session64();
    let mut tool = rect_tool();
    drag(&mut tool, &mut session, 10.0, 10.0, 20.0, 12.0);
    assert!(tool.has_pending());
    let mut render = false;
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        tool.set_pending(None, &mut cx).expect("discard");
        assert!(!tool.has_pending());
        tool.set_pending(
            Some(PendingValue::Box {
                x: 1,
                y: 2,
                w: 3,
                h: 4,
                radius: Some(0),
            }),
            &mut cx,
        )
        .expect("install");
    }
    match tool.pending_value() {
        Some(PendingValue::Box { x, y, w, h, radius }) => {
            assert_eq!((x, y, w, h, radius), (1, 2, 3, 4, Some(0)));
        }
        other => panic!("installed, got {other:?}"),
    }
}
