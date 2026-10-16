//! Pen and eraser acceptance: stroke painting, shift lines, locks, sampling.

use pixel_engine::model::{NewDocumentOptions, SettingInput};

use crate::features::pen::PenTool;
use crate::input::{RawPointer, build_tool_event};
use crate::tools::api::{Tool, ToolCx};

use super::{active_id, down, manager_pen_fill, pixel, session64, view1};

/// (a) down/move/up paints, one undo restores the canvas.
#[test]
fn pen_stroke_paints_and_undo_restores() {
    let (mut session, _queue) = session64();
    let mut render = false;
    let mut pen = PenTool::pen();
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        pen.pointer_down(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
        assert!(cx.session.has_stroke(), "down opens the stroke");
        let mv = RawPointer {
            coalesced: vec![(12.0, 10.0), (14.0, 10.0)],
            ..down(14.0, 10.0)
        };
        pen.pointer_move(&build_tool_event(&mv, &view1()), &mut cx);
        pen.pointer_up(&build_tool_event(&down(14.0, 10.0), &view1()), &mut cx);
        assert!(!cx.session.has_stroke(), "up commits the stroke");
    }
    assert_ne!(pixel(&session, 10, 10), 0, "the dot landed");
    assert_ne!(pixel(&session, 12, 10), 0, "coalesced points bridge");
    assert!(session.undo().expect("undo"));
    assert_eq!(pixel(&session, 10, 10), 0, "undo restores transparency");
    assert_eq!(pixel(&session, 12, 10), 0);
}

/// (b) shift + last point draws the straight segment, then commits at once.
#[test]
fn shift_click_draws_the_straight_segment() {
    let (mut session, _queue) = session64();
    let mut render = false;
    let mut pen = PenTool::pen();
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        pen.pointer_down(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
        pen.pointer_up(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
        let shift = RawPointer {
            shift: true,
            ..down(20.0, 10.0)
        };
        pen.pointer_down(&build_tool_event(&shift, &view1()), &mut cx);
        assert!(!cx.session.has_stroke(), "shift-click commits immediately");
    }
    assert_ne!(pixel(&session, 15, 10), 0, "the segment bridges the gap");
}

/// (c) the eraser packs 0 over the same layer.
#[test]
fn eraser_clears_what_the_pen_painted() {
    let (mut session, _queue) = session64();
    let mut render = false;
    let mut pen = PenTool::pen();
    let mut eraser = PenTool::eraser();
    assert_eq!(eraser.id(), "eraser");
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        pen.pointer_down(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
        pen.pointer_up(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
    }
    assert_ne!(pixel(&session, 10, 10), 0);
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        eraser.pointer_down(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
        eraser.pointer_up(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
    }
    assert_eq!(pixel(&session, 10, 10), 0, "erase packs transparent");
}

/// (d) a locked layer opens no stroke and reports the JS status message.
#[test]
fn pen_on_a_locked_layer_is_ignored_with_a_status_event() {
    let (mut session, queue) = session64();
    let id = active_id(&session);
    assert!(session.set_layer_locked(&id, true).expect("lock"));
    queue.borrow_mut().clear();
    let mut render = false;
    let mut pen = PenTool::pen();
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        pen.pointer_down(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
        assert!(!cx.session.has_stroke());
        assert!(!cx.session.is_editing());
        pen.pointer_up(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
    }
    assert_eq!(pixel(&session, 10, 10), 0, "nothing was written");
    let warned = queue.borrow().iter().any(|ev| {
        matches!(&ev.detail, pixel_engine::model::event::EventDetail::Status { text, .. }
            if text == "레이어가 잠겨 있습니다")
    });
    assert!(warned, "the lock status message fired");
}

/// (f) alt-click samples the composite into `primaryColor`.
#[test]
fn alt_click_eyedrops_the_composite() {
    let (mut session, _queue) = session64();
    let mut render = false;
    let mut pen = PenTool::pen();
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        pen.pointer_down(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
        pen.pointer_up(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
    }
    // Default primary is black; paint red first via a direct setting.
    assert!(
        session
            .set_setting("primaryColor", SettingInput::Str("#ff0000ff"))
            .expect("red primary")
    );
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        pen.pointer_down(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
        pen.pointer_up(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
    }
    assert!(
        session
            .set_setting("primaryColor", SettingInput::Str("#000000ff"))
            .expect("reset primary")
    );
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        let alt = RawPointer {
            alt: true,
            ..down(10.0, 10.0)
        };
        pen.pointer_down(&build_tool_event(&alt, &view1()), &mut cx);
        assert!(!cx.session.has_stroke(), "alt opens no stroke");
    }
    assert_eq!(session.settings().primary_color, "#ff0000");
}

/// (h) a document swap clears the pen's last point through the event queue.
#[test]
fn document_replace_clears_the_shift_anchor() {
    let (mut session, queue) = session64();
    let mut render = false;
    let mut manager = manager_pen_fill();
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        manager.activate_initial(&mut cx).expect("initial tool");
        manager
            .pointer_down(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx)
            .expect("dot");
        manager
            .pointer_up(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx)
            .expect("commit");
    }
    session
        .new_document(NewDocumentOptions::default().size(64, 64))
        .expect("replace");
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        for ev in queue.borrow_mut().drain(..) {
            manager.on_model_event(&ev, &mut cx);
        }
        // The anchor died with the document: shift-click is a lone dot.
        let shift = RawPointer {
            shift: true,
            ..down(30.0, 30.0)
        };
        manager
            .pointer_down(&build_tool_event(&shift, &view1()), &mut cx)
            .expect("shift-click");
    }
    assert_ne!(pixel(&session, 30, 30), 0, "the lone dot landed");
    assert_eq!(pixel(&session, 20, 20), 0, "no line back to the old anchor");
}
