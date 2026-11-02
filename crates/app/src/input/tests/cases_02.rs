use super::*;

#[test]
fn shift_wheel_with_a_non_finite_delta_pans_nothing() {
    let mut c = InputController::new();
    let f = Fixture::base();
    let out = c.wheel(f64::NAN, WheelMode::Pixel, true, (10.0, 10.0), &f.cx());
    let v = out.new_view.expect("view moved");
    assert_eq!(v.offset_x(), 0.0);
}

/// Trap 8: a normal wheel accumulates, walks the ladder, and anchors the zoom.
#[test]
fn wheel_accumulates_into_zoom_steps() {
    let mut c = InputController::new();
    let f = Fixture::base();
    // 120px is exactly two steps down from 1× → 0.5×.
    let out = c.wheel(120.0, WheelMode::Pixel, false, (40.0, 40.0), &f.cx());
    let v = out.new_view.expect("view moved");
    assert_eq!(v.zoom(), 0.5);
    // The anchor holds: (40, 40) maps to the same canvas point before and after.
    let before = pixel_at(f.view, 40.0, 40.0);
    let after = pixel_at(v, 40.0, 40.0);
    assert_eq!(before.x, after.x);
    assert_eq!(before.y, after.y);

    // Below the threshold nothing happens and not even a render is requested.
    let out = c.wheel(30.0, WheelMode::Pixel, false, (40.0, 40.0), &f.view(v).cx());
    assert!(out.new_view.is_none());
    assert!(!out.need_render);

    // The residue carries over, so 30 + 30 is one step.
    let out = c.wheel(30.0, WheelMode::Pixel, false, (40.0, 40.0), &f.view(v).cx());
    assert_eq!(out.new_view.expect("view moved").zoom(), 0.25);
}

#[test]
fn wheel_zoom_uses_the_page_height_for_page_mode() {
    let mut c = InputController::new();
    let f = Fixture::base().viewport(Some((200.0, 100.0)));
    let out = c.wheel(-1.0, WheelMode::Page, false, (10.0, 10.0), &f.cx());
    assert_eq!(out.new_view.expect("view moved").zoom(), 2.0);
}

// ---------------------------------------------------------------- keyboard

/// Trap 9: space latches on the way down and clears on the way up, and the
/// cursor follows the ladder in between.
#[test]
fn space_down_and_up_walk_the_cursor_ladder() {
    let mut c = InputController::new();
    let f = Fixture::base();
    assert_eq!(
        c.key_down(&other_key(), &f.cx()).cursor,
        CursorKind::Crosshair
    );
    assert!(!c.space_down());

    let out = c.key_down(&space_key(), &f.cx());
    assert!(c.space_down());
    assert_eq!(out.cursor, SPACE_CURSOR);
    // A repeat is not a second latch.
    c.key_down(&space_key(), &f.cx());
    assert!(c.space_down());

    let out = c.key_up(&space_key(), &f.cx());
    assert!(!c.space_down());
    assert_eq!(out.cursor, CursorKind::Crosshair);
    // Any other keyup is inert.
    assert!(!c.key_up(&other_key(), &f.cx()).need_render);
}

/// Trap 10: a focused text field swallows the keys, Space included.
#[test]
fn a_focused_text_field_swallows_space() {
    let mut c = InputController::new();
    let f = Fixture::base();
    assert!(!c.text_input_focused());
    let out = c.key_down(&space_key(), &f.cx());
    assert!(c.space_down());
    assert_eq!(out.cursor, SPACE_CURSOR);
    c.reset_transient();

    let out = c.key_down_focused(&space_key(), &f.cx(), true);
    assert!(!c.space_down());
    assert_eq!(out.cursor, CursorKind::Crosshair);
    // The flag also flows through the stored setter.
    c.set_text_input_focused(true);
    c.key_down(&space_key(), &f.cx());
    assert!(!c.space_down());
    c.set_text_input_focused(false);
    c.key_down(&space_key(), &f.cx());
    assert!(c.space_down());

    assert!(text_target_swallows_key("INPUT", false));
    assert!(text_target_swallows_key("textarea", false));
    assert!(text_target_swallows_key("div", true));
    assert!(!text_target_swallows_key("CANVAS", false));
    assert!(!text_target_swallows_key("span", false));
}

/// `blur` / tab-hidden: everything transient drops and the tool is cancelled.
#[test]
fn reset_transient_clears_state_and_cancels() {
    let mut c = InputController::new();
    let f = Fixture::base();
    c.key_down(&space_key(), &f.cx());
    c.pointer_down(&at(5.0, 5.0), &f.cx());
    assert!(c.is_panning());

    let out = c.reset_transient();
    assert_eq!(out.tool_calls, vec![ToolCall::Cancel]);
    assert!(!c.space_down());
    assert!(!c.is_panning());
    assert!(!c.is_drawing());
    assert_eq!(out.cursor, CursorKind::Crosshair);

    // Already outside, so the stale preview goes too.
    c.key_down(&space_key(), &f.cx());
    c.pointer_leave();
    let out = c.reset_transient();
    assert_eq!(
        out.tool_calls,
        vec![ToolCall::Cancel, ToolCall::Hover(None)]
    );
}

/// `SETTINGS_CHANGED(activeTool)` re-resolves from the new tool cursor.
#[test]
fn active_tool_changed_reapplies_the_cursor() {
    let mut c = InputController::new();
    let before = Fixture::base().cursor(CursorKind::Pen);
    c.key_down(&space_key(), &before.cx());
    let after = Fixture::base().cursor(CursorKind::Fill);
    let out = c.active_tool_changed(&after.cx());
    assert_eq!(out.cursor, SPACE_CURSOR);
    c.key_up(&space_key(), &after.cx());
    assert_eq!(c.active_tool_changed(&after.cx()).cursor, CursorKind::Fill);
}
