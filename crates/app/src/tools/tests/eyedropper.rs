//! Eyedropper acceptance: the two colour slots by button.

use crate::features::eyedropper::EyedropperTool;
use crate::features::pen::PenTool;
use crate::input::{Button, RawPointer, build_tool_event};
use crate::tools::api::{Tool, ToolCx};

use super::{down, session64, view1};

/// (f2) the eyedropper tool: left → primary, right → secondary.
#[test]
fn eyedropper_routes_buttons_to_the_two_slots() {
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
    let mut dropper = EyedropperTool::new();
    assert!(dropper.accepts_button(Button::Left));
    assert!(dropper.accepts_button(Button::Right));
    assert!(!dropper.accepts_button(Button::Middle));
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        dropper.pointer_down(&build_tool_event(&down(10.0, 10.0), &view1()), &mut cx);
    }
    assert_eq!(session.settings().primary_color, "#000000");
    {
        let mut cx = ToolCx {
            session: &mut session,
            view: view1(),
            need_render: &mut render,
        };
        let right = RawPointer {
            button: Button::Right,
            ..down(10.0, 10.0)
        };
        dropper.pointer_down(&build_tool_event(&right, &view1()), &mut cx);
    }
    assert_eq!(session.settings().secondary_color, "#000000");
}
