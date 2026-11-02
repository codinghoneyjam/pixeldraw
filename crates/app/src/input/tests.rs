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
mod cases_01;
mod cases_02;
