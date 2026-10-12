//! Manager acceptance: mid-stroke cancellation and id errors.

use crate::input::build_tool_event;
use crate::tools::api::{CursorKind, Tool, ToolCx};
use crate::tools::hand::HandTool;
use crate::tools::manager::ToolManager;

use super::{down, manager_pen_fill, pixel, session64, view1};

/// (g) switching tools mid-stroke cancels the stroke.
#[test]
fn switching_tools_cancels_the_live_stroke() {
    let (mut session, _queue) = session64();
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
            .expect("pen down");
        assert!(cx.session.has_stroke());
        manager.switch_to("fill", &mut cx);
        assert!(!cx.session.has_stroke(), "the stroke was cancelled");
        assert!(!cx.session.is_editing());
        assert_eq!(manager.active_id(), "fill");
    }
    assert_eq!(pixel(&session, 10, 10), 0, "the cancelled dot left nothing");
}

/// (g2) an unregistered active id is `INVALID_STATE`, like the JS throw.
#[test]
fn unregistered_active_is_an_error_not_a_panic() {
    let (mut session, _queue) = session64();
    let mut render = false;
    let mut manager = ToolManager::new();
    manager.set_initial_id("nope");
    let mut cx = ToolCx {
        session: &mut session,
        view: view1(),
        need_render: &mut render,
    };
    assert!(manager.active().is_err());
    assert!(
        manager
            .pointer_down(&build_tool_event(&down(1.0, 1.0), &view1()), &mut cx)
            .is_err()
    );
}

/// The hand tool is an id and a cursor — panning is the controller's job.
#[test]
fn hand_has_no_handlers() {
    let hand = HandTool::new();
    assert_eq!(hand.id(), "hand");
    assert_eq!(hand.cursor(), CursorKind::Grab);
}
