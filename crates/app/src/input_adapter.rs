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
    pub wheel: Option<WheelFrame>,
    pub keys: Vec<(KeyEvent, bool)>,
    pub text_focused: bool,
}

pub struct WheelFrame {
    pub delta_y: f64,
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

/// Translate one egui frame. `origin` is the canvas widget's top-left in
/// screen CSS px; positions come out relative to it (what `build_tool_event`
/// feeds the view math).
pub fn poll_frame_input(ctx: &egui::Context, origin: egui::Pos2) -> FrameInput {
    let mut down = vec![];
    let mut moves = vec![];
    let mut up = vec![];
    let mut wheel = None;
    let mut keys = vec![];
    let text_focused = ctx.egui_wants_keyboard_input();

    ctx.input(|i| {
        let to_local = |p: egui::Pos2| (p.x as f64 - origin.x as f64, p.y as f64 - origin.y as f64);
        for &b in &[
            egui::PointerButton::Primary,
            egui::PointerButton::Secondary,
            egui::PointerButton::Middle,
        ] {
            let Some(button) = egui_button(b) else {
                continue;
            };
            if i.pointer.button_pressed(b)
                && let Some(pos) = i.pointer.hover_pos().map(to_local)
            {
                down.push((button, pos));
            }
            if i.pointer.button_released(b)
                && let Some(pos) = i.pointer.hover_pos().map(to_local)
            {
                up.push((button, pos));
            }
        }
        if let Some(pos) = i.pointer.hover_pos().map(to_local) {
            moves.push(pos);
        }
        // egui scroll-up is +y; the JS `deltaY` it replaces runs the other way.
        let scroll = i.smooth_scroll_delta();
        if scroll.y != 0.0
            && let Some(pos) = i.pointer.hover_pos().map(to_local)
        {
            wheel = Some(((-scroll.y) as f64, i.modifiers.shift, pos));
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
        wheel: wheel.map(|(delta_y, shift, pos)| WheelFrame {
            delta_y,
            shift,
            pos,
        }),
        keys,
        text_focused,
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

pub fn wheel_mode() -> WheelMode {
    // egui reports points; the accumulator's Pixel mode is the match.
    WheelMode::Pixel
}
