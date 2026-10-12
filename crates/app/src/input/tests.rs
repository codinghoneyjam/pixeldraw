//! Unit tests for the input state machine.
//!
//! Everything here is hand-constructed: no DOM, no egui, no window. The JS
//! originals are `tests/tools_manager.test.mjs` (wheel + cursor ladder) and
//! `tests/tool_integrity_check.mjs` (wheel accumulator); those cases are ported
//! into the per-module `#[cfg(test)]` blocks.

use super::*;
use crate::tools::CursorKind;
use pixel_engine::view::{View, pixel_at};

/// `Tool::accepts_button` stand-ins. Function items, so a `&'static` reference
/// coerces straight into `&dyn Fn(Button) -> bool`.
static LEFT_ONLY: fn(Button) -> bool = |b| b == Button::Left;
static LEFT_OR_RIGHT: fn(Button) -> bool = |b| matches!(b, Button::Left | Button::Right);

/// A 32×32 document in a 200×200 viewport at 1× with the pen tool active.
#[derive(Clone, Copy)]
struct Fixture {
    view: View,
    canvas: Option<(u32, u32)>,
    viewport: Option<(f64, f64)>,
    tool_id: &'static str,
    accepts: &'static dyn Fn(Button) -> bool,
    cursor: CursorKind,
}

impl Fixture {
    fn base() -> Self {
        Self {
            view: View::new(1.0, 0.0, 0.0).expect("valid view"),
            canvas: Some((32, 32)),
            viewport: Some((200.0, 200.0)),
            tool_id: "pen",
            accepts: &LEFT_ONLY,
            cursor: CursorKind::Crosshair,
        }
    }

    fn canvas(mut self, c: Option<(u32, u32)>) -> Self {
        self.canvas = c;
        self
    }

    fn viewport(mut self, v: Option<(f64, f64)>) -> Self {
        self.viewport = v;
        self
    }

    fn view(mut self, v: View) -> Self {
        self.view = v;
        self
    }

    fn tool(mut self, id: &'static str) -> Self {
        self.tool_id = id;
        self
    }

    fn accepts(mut self, a: &'static dyn Fn(Button) -> bool) -> Self {
        self.accepts = a;
        self
    }

    fn cursor(mut self, c: CursorKind) -> Self {
        self.cursor = c;
        self
    }

    fn cx(&self) -> InputCtx<'_> {
        InputCtx {
            view: self.view,
            canvas: self.canvas,
            viewport: self.viewport,
            active_tool_id: self.tool_id,
            tool_accepts_button: self.accepts,
            tool_cursor: self.cursor,
        }
    }
}

fn at(x: f64, y: f64) -> RawPointer {
    RawPointer::new((x, y))
}

fn with(p: RawPointer, id: u64) -> RawPointer {
    RawPointer {
        pointer_id: id,
        ..p
    }
}

fn space_key() -> KeyEvent {
    KeyEvent {
        key: SPACE_KEY.to_string(),
        code: SPACE_CODE.to_string(),
        shift: false,
        ctrl: false,
        alt: false,
    }
}

fn other_key() -> KeyEvent {
    KeyEvent {
        key: "a".to_string(),
        code: "KeyA".to_string(),
        shift: false,
        ctrl: false,
        alt: false,
    }
}

// ---------------------------------------------------------------- pan gate

/// Trap 2: middle always pans, space makes left pan, the hand tool makes left
/// pan, and a plain left on the pen tool does not.
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
