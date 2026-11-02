//! Host input → controller input (W8 glue, egui side).
//!
//! egui reports per-frame facts; this module turns one frame's facts into the
//! [`RawPointer`]/wheel/key values the pure [`InputController`] consumes. It
//! knows nothing about tools or the session — `canvas_view` dispatches what
//! comes out. Only tool-relevant keys are translated (Space for pan, shape
//! editing keys); the full shortcut map is Phase 3.1.

use crate::input::{Button, KeyEvent, PointerKind, RawPointer, WheelMode};

/// One frame of translated host input, positions in canvas-widget CSS px.
pub struct FrameInput {
    pub down: Vec<RawPointer>,
    pub moves: Vec<RawPointer>,
    pub up: Vec<RawPointer>,
    /// Every raw wheel event in the frame, in arrival order. Keeping the list
    /// (instead of one coalesced delta) is what makes a fast wheel move the
    /// zoom ladder at the same rate as the JS per-event handler.
    pub wheels: Vec<WheelFrame>,
    pub keys: Vec<(KeyEvent, bool)>,
    pub text_focused: bool,
    /// The pointer is off the canvas with no captured gesture: `pointerleave`.
    pub leaving: bool,
}

pub struct WheelFrame {
    pub delta_y: f64,
    pub mode: WheelMode,
    pub shift: bool,
    pub pos: (f64, f64),
}

fn egui_button(b: egui::PointerButton) -> Option<Button> {
    match b {
        egui::PointerButton::Primary => Some(Button::Left),
        egui::PointerButton::Secondary => Some(Button::Right),
        egui::PointerButton::Middle => Some(Button::Middle),
        _ => None,
    }
}

fn raw(pos: (f64, f64), button: Button, down_mask: u8, ctx: &egui::Context) -> RawPointer {
    let (shift, ctrl, alt) = ctx.input(|i| {
        let m = i.modifiers;
        (m.shift, m.ctrl || m.command, m.alt)
    });
    let time = ctx.input(|i| i.time);
    RawPointer {
        pos,
        coalesced: vec![],
        button,
        buttons_down: down_mask,
        shift,
        ctrl,
        alt,
        pointer_id: 1,
        kind: PointerKind::Mouse,
        time_ms: time * 1000.0,
    }
}

/// Which host event kinds need the pointer on the canvas itself.
///
/// `pointer_bindings.js` registered the listeners on the canvas host element,
/// so a press or a wheel anywhere else belongs to that other widget; only
/// `up`/`move` also flow while a gesture holds the pointer
/// (`setPointerCapture`), which is how a released-anywhere stroke still ends.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HostEvent {
    Down,
    Up,
    Move,
    Wheel,
}

fn host_wants(kind: HostEvent, inside: bool, captured: bool) -> bool {
    match kind {
        HostEvent::Down | HostEvent::Wheel => inside,
        HostEvent::Up | HostEvent::Move => inside || captured,
    }
}

/// Translate one egui frame. `rect` is the canvas widget in screen CSS px — the
/// port's stand-in for the canvas host element the JS bound to — and positions
/// come out relative to it (what `build_tool_event` feeds the view math).
/// `captured` is a pan or stroke already in flight: the JS
/// `setPointerCapture` window, during which moves and the release keep flowing
/// even when the pointer leaves the canvas. `busy` (a dialog is open) covers
/// the host the way the JS dialog overlay did: nothing new starts behind it.
pub fn poll_frame_input(
    ctx: &egui::Context,
    rect: egui::Rect,
    captured: bool,
    busy: bool,
) -> FrameInput {
    let mut down = vec![];
    let mut moves = vec![];
    let mut up = vec![];
    let mut wheels = vec![];
    let mut keys = vec![];
    let mut leaving = false;
    let text_focused = ctx.egui_wants_keyboard_input();

    ctx.input(|i| {
        let to_local = |p: egui::Pos2| {
            (
                p.x as f64 - rect.min.x as f64,
                p.y as f64 - rect.min.y as f64,
            )
        };
        // A dialog is a floating window over the host, so "on the host" means
        // inside the rect and not behind a dialog.
        let inside = |p: egui::Pos2| rect.contains(p) && !busy;
        let scoped = |kind: HostEvent| {
            i.pointer
                .hover_pos()
                .filter(|p| host_wants(kind, inside(*p), captured))
        };
        leaving = i
            .pointer
            .hover_pos()
            .is_none_or(|p| !inside(p) && !captured);
        for &b in &[
            egui::PointerButton::Primary,
            egui::PointerButton::Secondary,
            egui::PointerButton::Middle,
        ] {
            let Some(button) = egui_button(b) else {
                continue;
            };
            if i.pointer.button_pressed(b)
                && let Some(pos) = scoped(HostEvent::Down).map(to_local)
            {
                down.push((button, pos));
            }
            if i.pointer.button_released(b)
                && let Some(pos) = scoped(HostEvent::Up).map(to_local)
            {
                up.push((button, pos));
            }
        }
        if let Some(pos) = scoped(HostEvent::Move).map(to_local) {
            moves.push(pos);
        }
        // egui/JS content movement is +y; the JS `deltaY` the accumulator
        // expects runs the other way. Read the *raw* events so each wheel notch
        // reaches the controller separately. `smooth_scroll_delta` deliberately
        // spreads one notch over several frames, which made fast flicks lag
        // behind the pointer.
        if let Some(pos) = scoped(HostEvent::Wheel).map(to_local) {
            for ev in &i.events {
                if let egui::Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } = ev
                {
                    let (mode, delta_y) = match unit {
                        egui::MouseWheelUnit::Point => (WheelMode::Pixel, -f64::from(delta.y)),
                        egui::MouseWheelUnit::Line => (WheelMode::Line, -f64::from(delta.y)),
                        egui::MouseWheelUnit::Page => (WheelMode::Page, -f64::from(delta.y)),
                    };
                    if delta_y.is_finite() && delta_y != 0.0 {
                        wheels.push(WheelFrame {
                            delta_y,
                            mode,
                            shift: modifiers.shift,
                            pos,
                        });
                    }
                }
            }
            // Backends that do not expose raw wheel events still get the
            // smoothed path as a fallback.
            if wheels.is_empty() {
                let scroll = i.smooth_scroll_delta();
                if scroll.y != 0.0 {
                    wheels.push(WheelFrame {
                        delta_y: -f64::from(scroll.y),
                        mode: WheelMode::Pixel,
                        shift: i.modifiers.shift,
                        pos,
                    });
                }
            }
        }
        for ev in &i.events {
            if let egui::Event::Key { key, pressed, .. } = ev
                && let Some((k, code)) = translate_key(*key)
            {
                keys.push((
                    KeyEvent {
                        key: k.to_string(),
                        code: code.to_string(),
                        shift: i.modifiers.shift,
                        ctrl: i.modifiers.ctrl || i.modifiers.command,
                        alt: i.modifiers.alt,
                    },
                    *pressed,
                ));
            }
        }
    });
    let down = down
        .into_iter()
        .map(|(button, pos)| raw(pos, button, 1, ctx))
        .collect();
    let moves = moves
        .into_iter()
        .map(|pos| {
            let mask = ctx.input(|i| {
                if [
                    egui::PointerButton::Primary,
                    egui::PointerButton::Secondary,
                    egui::PointerButton::Middle,
                ]
                .iter()
                .any(|&b| i.pointer.button_down(b))
                {
                    1
                } else {
                    0
                }
            });
            raw(pos, Button::Left, mask, ctx)
        })
        .collect();
    let up = up
        .into_iter()
        .map(|(button, pos)| raw(pos, button, 0, ctx))
        .collect();
    FrameInput {
        down,
        moves,
        up,
        wheels,
        keys,
        text_focused,
        leaving,
    }
}

/// Tool-relevant keys only. Returns JS `(key, code)` strings.
fn translate_key(key: egui::Key) -> Option<(&'static str, &'static str)> {
    Some(match key {
        egui::Key::Space => (" ", "Space"),
        egui::Key::Enter => ("Enter", "Enter"),
        egui::Key::Escape => ("Escape", "Escape"),
        egui::Key::Delete => ("Delete", "Delete"),
        egui::Key::Backspace => ("Backspace", "Backspace"),
        egui::Key::ArrowLeft => ("ArrowLeft", "ArrowLeft"),
        egui::Key::ArrowRight => ("ArrowRight", "ArrowRight"),
        egui::Key::ArrowUp => ("ArrowUp", "ArrowUp"),
        egui::Key::ArrowDown => ("ArrowDown", "ArrowDown"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The JS bound every listener to the canvas host element: a press or a
    /// wheel outside it is another widget's event (the panel drags and zooms
    /// the user reported), while a captured gesture still gets its moves and
    /// its release so a stroke can end anywhere.
    #[test]
    fn host_events_are_scoped_to_the_canvas_element() {
        assert!(host_wants(HostEvent::Down, true, false));
        assert!(!host_wants(HostEvent::Down, false, false));
        assert!(host_wants(HostEvent::Wheel, true, false));
        assert!(!host_wants(HostEvent::Wheel, false, false));
        // Capture (`setPointerCapture`) keeps a gesture alive past the edge.
        assert!(host_wants(HostEvent::Move, false, true));
        assert!(host_wants(HostEvent::Up, false, true));
        // But it never invents a new press or a new wheel outside.
        assert!(!host_wants(HostEvent::Down, false, true));
        assert!(!host_wants(HostEvent::Wheel, false, true));
    }
}
