use super::*;

#[test]
fn pan_gate_is_middle_space_or_hand() {
    let mut c = InputController::new();
    let f = Fixture::base();
    let middle = RawPointer {
        button: Button::Middle,
        ..at(10.0, 10.0)
    };
    let out = c.pointer_down(&middle, &f.cx());
    assert!(out.tool_calls.is_empty());
    assert_eq!(out.capture_pointer, Some(true));
    assert_eq!(out.cursor, PAN_CURSOR);
    assert!(c.is_panning());

    let mut c = InputController::new();
    c.key_down(&space_key(), &f.cx());
    let out = c.pointer_down(&at(10.0, 10.0), &f.cx());
    assert!(c.is_panning());
    assert_eq!(out.cursor, PAN_CURSOR);

    let mut c = InputController::new();
    let hand = f.tool("hand");
    let out = c.pointer_down(&at(10.0, 10.0), &hand.cx());
    assert!(c.is_panning());
    assert!(out.tool_calls.is_empty());

    let mut c = InputController::new();
    let out = c.pointer_down(&at(10.0, 10.0), &f.cx());
    assert!(!c.is_panning());
    assert!(c.is_drawing());
    assert_eq!(out.cursor, CursorKind::Crosshair);
}

/// Right never pans, even under space, because the gate is `button ∈ {L, M}`.
#[test]
fn right_button_never_pans_even_with_space_held() {
    let mut c = InputController::new();
    c.key_down(&space_key(), &Fixture::base().cx());
    let right = RawPointer {
        button: Button::Right,
        ..at(10.0, 10.0)
    };
    let out = c.pointer_down(&right, &Fixture::base().cx());
    assert!(!c.is_panning());
    assert!(!c.is_drawing());
    assert!(out.tool_calls.is_empty());
}

/// Trap 3: deltas are relative to the previous position and the result is
/// clamped against the document + viewport.
#[test]
fn pan_moves_relative_and_clamps() {
    let mut c = InputController::new();
    let f = Fixture::base();
    let middle = RawPointer {
        button: Button::Middle,
        ..at(100.0, 100.0)
    };
    c.pointer_down(&middle, &f.cx());
    let out = c.pointer_move(&with(at(150.0, 130.0), 1), &f.cx());
    let v = out.new_view.expect("view moved");
    assert_eq!((v.offset_x(), v.offset_y()), (50.0, 30.0));
    assert!(out.need_render);
    // Running off the far edge clamps at vw - margin = 200 - 32.
    let out = c.pointer_move(&with(at(9_000.0, 9_000.0), 1), &f.view(v).cx());
    let v = out.new_view.expect("view moved");
    assert_eq!((v.offset_x(), v.offset_y()), (168.0, 168.0));
    // A pointer the pan gesture does not own is not a pan.
    let out = c.pointer_move(&with(at(0.0, 0.0), 2), &f.view(v).cx());
    assert!(out.new_view.is_none());
}

/// Without the document size there is nothing to clamp against.
#[test]
fn pan_skips_clamp_without_a_canvas() {
    let mut c = InputController::new();
    let f = Fixture::base().canvas(None);
    let middle = RawPointer {
        button: Button::Middle,
        ..at(10.0, 10.0)
    };
    c.pointer_down(&middle, &f.cx());
    let out = c.pointer_move(&with(at(9_000.0, 0.0), 1), &f.cx());
    let v = out.new_view.expect("view moved");
    assert_eq!(v.offset_x(), 8_990.0);
}

// ------------------------------------------------------- down/up transitions

/// Trap 4 + (a): a pan gesture ends before the same pointer can start a draw.
#[test]
fn pan_to_draw_transition() {
    let mut c = InputController::new();
    let f = Fixture::base();
    c.key_down(&space_key(), &f.cx());
    assert_eq!(c.pointer_down(&at(5.0, 5.0), &f.cx()).cursor, PAN_CURSOR);
    assert!(c.is_panning());
    let out = c.pointer_up(&at(5.0, 5.0), &f.cx());
    assert_eq!(out.capture_pointer, Some(false));
    assert!(out.tool_calls.is_empty());
    assert!(!c.is_panning());
    // Space is still held, so the ladder still says grab.
    assert_eq!(out.cursor, SPACE_CURSOR);
    c.key_up(&space_key(), &f.cx());
    assert_eq!(
        c.key_up(&space_key(), &f.cx()).cursor,
        CursorKind::Crosshair
    );

    let out = c.pointer_down(&at(5.0, 5.0), &f.cx());
    assert!(c.is_drawing());
    assert_eq!(out.tool_calls.len(), 1);
    assert!(matches!(out.tool_calls[0], ToolCall::Down(_)));
    let up = c.pointer_up(&at(5.0, 5.0), &f.cx());
    assert_eq!(up.tool_calls.len(), 1);
    assert!(matches!(up.tool_calls[0], ToolCall::Up(_)));
}

/// Trap 4: a second `pointerdown` while drawing or panning changes nothing.
#[test]
fn pointer_down_is_ignored_while_a_gesture_is_live() {
    let mut c = InputController::new();
    let f = Fixture::base();
    c.pointer_down(&with(at(5.0, 5.0), 1), &f.cx());
    let out = c.pointer_down(&with(at(80.0, 80.0), 2), &f.cx());
    assert!(out.tool_calls.is_empty());
    assert!(out.new_view.is_none());
    assert!(out.capture_pointer.is_none());
    assert!(c.is_drawing());
    let up = c.pointer_up(&with(at(5.0, 5.0), 1), &f.cx());
    assert!(matches!(up.tool_calls[0], ToolCall::Up(_)));

    let mut c = InputController::new();
    let middle = RawPointer {
        button: Button::Middle,
        ..at(5.0, 5.0)
    };
    c.pointer_down(&middle, &f.cx());
    let out = c.pointer_down(&with(at(80.0, 80.0), 2), &f.cx());
    assert!(out.tool_calls.is_empty());
    assert!(out.capture_pointer.is_none());
    assert!(c.is_panning());
}

/// Trap 4: a tool that does not accept the button gets no state at all.
#[test]
fn a_rejected_button_changes_nothing() {
    let mut c = InputController::new();
    let right = RawPointer {
        button: Button::Right,
        ..at(5.0, 5.0)
    };
    let out = c.pointer_down(&right, &Fixture::base().cx());
    assert!(!c.is_drawing());
    assert!(!c.is_panning());
    assert!(out.tool_calls.is_empty());
    assert!(out.capture_pointer.is_none());
    assert_eq!(out.cursor, CursorKind::Crosshair);

    // A tool that also takes Right accepts the same event (Right never pans,
    // so the gate is the tool's answer, not a hardcoded Left).
    let mut c = InputController::new();
    let f = Fixture::base().accepts(&LEFT_OR_RIGHT);
    let out = c.pointer_down(&right, &f.cx());
    assert!(c.is_drawing());
    assert!(matches!(out.tool_calls[0], ToolCall::Down(_)));
}

/// `pointercancel` drops the gesture and tells the tool to abandon it.
#[test]
fn pointer_cancel_abandons_the_draw() {
    let mut c = InputController::new();
    c.pointer_down(&with(at(5.0, 5.0), 1), &Fixture::base().cx());
    let out = c.pointer_cancel(1);
    assert_eq!(out.tool_calls, vec![ToolCall::Cancel]);
    assert_eq!(out.capture_pointer, Some(false));
    assert!(!c.is_drawing());
    // A pointer that owns nothing is a no-op.
    let out = c.pointer_cancel(1);
    assert!(out.tool_calls.is_empty());
}

// --------------------------------------------------------- hover and second

/// Trap 5: hover only flows while nothing is drawing, and the draw pointer's own
/// moves are `Move`, not hover.
#[test]
fn hover_only_while_not_drawing() {
    let mut c = InputController::new();
    let f = Fixture::base();
    let out = c.pointer_move(&at(10.0, 10.0), &f.cx());
    assert_eq!(
        out.tool_calls,
        vec![ToolCall::Hover(Some(HoverEvent {
            x: 10,
            y: 10,
            fx: 10.0,
            fy: 10.0
        }))]
    );

    c.pointer_down(&with(at(5.0, 5.0), 1), &f.cx());
    let out = c.pointer_move(&with(at(6.0, 6.0), 1), &f.cx());
    assert_eq!(out.tool_calls.len(), 1);
    assert!(matches!(out.tool_calls[0], ToolCall::Move(_)));

    // A second mouse with no buttons held still produces nothing at all.
    let out = c.pointer_move(&with(at(6.0, 6.0), 2), &f.cx());
    assert!(out.tool_calls.is_empty());
}

/// Trap 5: another pointer's move is dropped when it is not a mouse, or when
/// buttons are held.
#[test]
fn a_second_pointer_is_filtered_by_kind_and_buttons() {
    let mut c = InputController::new();
    let f = Fixture::base();
    c.pointer_down(&with(at(5.0, 5.0), 1), &f.cx());
    let touch = RawPointer {
        kind: PointerKind::Touch,
        ..at(6.0, 6.0)
    };
    assert!(
        c.pointer_move(&with(touch, 2), &f.cx())
            .tool_calls
            .is_empty()
    );

    let pen = RawPointer {
        buttons_down: 1,
        ..at(6.0, 6.0)
    };
    assert!(c.pointer_move(&with(pen, 2), &f.cx()).tool_calls.is_empty());

    // Same pointer, still drawing: the tool keeps getting Move.
    let out = c.pointer_move(&with(at(7.0, 7.0), 1), &f.cx());
    assert!(matches!(out.tool_calls[0], ToolCall::Move(_)));
}

/// Trap 6: leaving drops presence and the preview, unless a gesture is captured.
#[test]
fn pointer_leave_keeps_a_captured_preview() {
    let mut c = InputController::new();
    let f = Fixture::base();
    let out = c.pointer_leave();
    assert_eq!(out.tool_calls, vec![ToolCall::Hover(None)]);
    assert_eq!(out.cursor, DEFAULT_CURSOR);
    assert!(!c.inside());

    // While drawing, the leave is silent and the preview survives.
    let mut c = InputController::new();
    c.pointer_down(&with(at(5.0, 5.0), 1), &f.cx());
    let out = c.pointer_leave();
    assert!(out.tool_calls.is_empty());
    assert_eq!(out.cursor, DEFAULT_CURSOR);
    // ...and the pointer coming back re-arms presence.
    let out = c.pointer_move(&with(at(5.0, 5.0), 1), &f.cx());
    assert!(c.inside());
    assert!(matches!(out.tool_calls[0], ToolCall::Move(_)));
    assert_eq!(out.cursor, CursorKind::Crosshair);
}

/// Trap 7: a captured pointer re-derives presence from the viewport rect, and a
/// pointer released outside also clears the preview.
#[test]
fn capture_recomputes_inside_from_the_viewport() {
    let mut c = InputController::new();
    let f = Fixture::base();
    c.pointer_down(&with(at(5.0, 5.0), 1), &f.cx());
    // Inside the 200×200 viewport but past the 32×32 document: still present.
    let out = c.pointer_move(&with(at(150.0, 150.0), 1), &f.cx());
    assert_eq!(out.tool_calls.len(), 1);
    assert!(c.inside());
    let out = c.pointer_move(&with(at(250.0, 250.0), 1), &f.cx());
    assert!(!c.inside());
    assert!(matches!(out.tool_calls[0], ToolCall::Move(_)));
    assert_eq!(out.cursor, DEFAULT_CURSOR);

    // Releasing outside clears the stale preview right after the Up.
    let out = c.pointer_up(&with(at(250.0, 250.0), 1), &f.cx());
    assert_eq!(
        out.tool_calls,
        vec![
            ToolCall::Up(build_tool_event(&with(at(250.0, 250.0), 1), &f.view)),
            ToolCall::Hover(None)
        ]
    );
    assert_eq!(out.cursor, DEFAULT_CURSOR);
}

/// Without a viewport there is no rect to test against, so presence is left as
/// the last move said.
#[test]
fn capture_without_a_viewport_keeps_the_last_presence() {
    let mut c = InputController::new();
    let f = Fixture::base().viewport(None);
    c.pointer_down(&with(at(5.0, 5.0), 1), &f.cx());
    c.pointer_leave();
    assert!(!c.inside());
    c.pointer_move(&with(at(5.0, 5.0), 1), &f.cx());
    assert!(!c.inside());
}

// -------------------------------------------------------------------- wheel

/// Trap 8: Shift+wheel pans x through `-deltaY`; `deltaX` never exists here.
#[test]
fn shift_wheel_pans_x_and_ignores_delta_x() {
    let mut c = InputController::new();
    let f = Fixture::base().view(View::new(1.0, 100.0, 100.0).expect("valid"));
    let out = c.wheel(40.0, WheelMode::Pixel, true, (10.0, 10.0), &f.cx());
    let v = out.new_view.expect("view moved");
    assert_eq!((v.offset_x(), v.offset_y()), (60.0, 100.0));
    assert!(out.need_render);
    let out = c.wheel(40.0, WheelMode::Pixel, true, (10.0, 10.0), &f.view(v).cx());
    let v = out.new_view.expect("view moved");
    assert_eq!((v.offset_x(), v.offset_y()), (20.0, 100.0));
    // And it is clamped exactly like a pan is.
    let out = c.wheel(
        1_000.0,
        WheelMode::Pixel,
        true,
        (10.0, 10.0),
        &f.view(v).cx(),
    );
    let v = out.new_view.expect("view moved");
    assert_eq!((v.offset_x(), v.offset_y()), (0.0, 100.0));
}
