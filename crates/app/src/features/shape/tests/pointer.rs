//! Drag, commit, undo and the line tool.

use pixel_engine::raster::shape_raster::ShapeKind;

use crate::features::shape::ShapeTool;
use crate::input::{RawPointer, build_tool_event};
use crate::tools::api::{Tool, ToolCx};

use super::{drag, key, rect_tool};
use crate::tools::tests::{down, pixel, session64, view1};

/// Rect drag becomes pending, commits one history entry, undoes cleanly.
#[test]
fn rect_drag_commit_and_undo() {
    let (mut session, _queue) = session64();
    let mut tool = rect_tool();
    drag(&mut tool, &mut session, 10.0, 10.0, 20.0, 12.0);
    assert!(tool.has_pending(), "pointer-up leaves a pending shape");
    assert_eq!(
        tool.pending_value(),
        Some(crate::features::shape::PendingValue::Box {
            x: 10,
            y: 10,
            w: 11,
            h: 3,
            radius: Some(8)
        }),
        "pending accessor round-trips the drag box"
    );
    let mut render = false;
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        assert!(tool.commit(&mut cx).expect("commit"));
        assert!(!tool.has_pending());
    }
    assert_ne!(pixel(&session, 10, 10), 0, "outline corner painted");
    assert_eq!(
        pixel(&session, 15, 11),
        0,
        "outline leaves the middle empty"
    );
    assert!(session.undo().expect("undo"));
    assert_eq!(pixel(&session, 10, 10), 0, "undo clears the shape");
}

/// A zero-area drag drops the draw without pending.
#[test]
fn click_without_drag_leaves_nothing() {
    let (mut session, _queue) = session64();
    let mut tool = rect_tool();
    drag(&mut tool, &mut session, 10.0, 10.0, 10.0, 10.0);
    assert!(!tool.has_pending());
    assert_eq!(pixel(&session, 10, 10), 0);
}

/// Escape while drawing abandons the rubber band.
#[test]
fn escape_while_drawing_cancels() {
    let (mut session, _queue) = session64();
    let mut tool = rect_tool();
    let mut render = false;
    let mut cx = ToolCx {
        session: &mut session,
        view: view1(),
        need_render: &mut render,
    };
    tool.pointer_down(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
    assert!(tool.key_down(&key("Escape", false), &mut cx));
    assert!(!tool.has_pending());
    assert_eq!(pixel(&session, 10, 10), 0);
}

/// Line drag paints the diagonal; shift snaps it straight.
#[test]
fn line_drag_and_shift_snap() {
    let (mut session, _queue) = session64();
    let mut tool = ShapeTool::new(ShapeKind::Line);
    drag(&mut tool, &mut session, 5.0, 5.0, 9.0, 9.0);
    let mut render = false;
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        assert!(tool.commit(&mut cx).expect("commit"));
    }
    assert_ne!(pixel(&session, 7, 7), 0, "diagonal pixel painted");

    let (mut session, _queue) = session64();
    let mut tool = ShapeTool::new(ShapeKind::Line);
    let mut render = false;
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        tool.pointer_down(&build_tool_event(&down(5.0, 5.0), &view1()), &mut cx);
        let shift = RawPointer {
            shift: true,
            ..down(9.0, 6.0)
        };
        tool.pointer_move(&build_tool_event(&shift, &view1()), &mut cx);
        tool.pointer_up(&build_tool_event(&shift, &view1()), &mut cx);
        assert!(tool.commit(&mut cx).expect("commit"));
    }
    // 15° snap of a near-horizontal drag: the far endpoint stays on the row.
    assert_ne!(pixel(&session, 5, 5), 0);
}
