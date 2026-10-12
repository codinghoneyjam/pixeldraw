//! Fill acceptance: same-colour notice and region painting.

use crate::input::build_tool_event;
use crate::tools::api::{Tool, ToolCx};
use crate::tools::fill::FillTool;
use crate::tools::pen::PenTool;

use super::{down, pixel, session64, view1};

/// (e) fill on its own colour notifies and cancels the opened stroke.
#[test]
fn fill_with_the_seed_colour_notifies_and_paints_nothing() {
    let (mut session, queue) = session64();
    // Default primary is opaque black: stamp one black dot, then fill it.
    let mut render = false;
    let mut pen = PenTool::pen();
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        pen.pointer_down(&build_tool_event(&down(30.0, 30.0), &view1()), &mut cx);
        pen.pointer_up(&build_tool_event(&down(30.0, 30.0), &view1()), &mut cx);
    }
    assert_ne!(pixel(&session, 30, 30), 0);
    queue.borrow_mut().clear();
    let mut fill = FillTool::new();
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        fill.pointer_down(&build_tool_event(&down(30.0, 30.0), &view1()), &mut cx);
        assert!(!cx.session.has_stroke(), "same-colour cancels the stroke");
    }
    assert_ne!(pixel(&session, 30, 30), 0, "the dot survives untouched");
    let told = queue.borrow().iter().any(|ev| {
        matches!(&ev.detail, pixel_engine::model::event::EventDetail::Status { text, .. }
            if text == "이미 같은 색입니다")
    });
    assert!(told, "the same-colour notice fired");
}

/// (e2) fill on a different colour fills the region in one stroke.
#[test]
fn fill_paints_the_region() {
    let (mut session, _queue) = session64();
    let mut render = false;
    let mut fill = FillTool::new();
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        fill.pointer_down(&build_tool_event(&down(5.0, 5.0), &view1()), &mut cx);
        assert!(!cx.session.has_stroke(), "fill commits inside pointer_down");
    }
    assert_ne!(pixel(&session, 5, 5), 0, "black over transparency");
    assert_ne!(
        pixel(&session, 63, 63),
        0,
        "the whole empty canvas is one region"
    );
    assert!(session.undo().expect("undo"));
    assert_eq!(pixel(&session, 5, 5), 0, "undo restores");
}
