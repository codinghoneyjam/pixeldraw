//! `input_controller.js` + `pointer_bindings.js` — the pointer state machine.
//!
//! The DOM listeners are gone; each method here is one listener body with the
//! host facts injected through [`InputCtx`]. The controller never learns what a
//! concrete tool is: it asks `tool_accepts_button` whether a button is wanted
//! and reads `tool_cursor` back out. What comes out is a list of [`ToolCall`]s
//! for the owner to dispatch (W8), never a call made here.

use crate::input::build_event::{RawPointer, build_tool_event};
use crate::input::cursor::{self, CursorInputs};
use crate::input::event::{Button, HoverEvent, KeyEvent, PointerKind, ToolEvent};
use crate::input::keyboard::is_space;
use crate::input::wheel::{WheelAccumulator, WheelMode};
use crate::tools::CursorKind;
use pixel_engine::view::{View, clamp_view, pan_by, step_zoom, zoom_at};

/// `toolManager`'s hand tool, whose existence alone switches panning on.
pub const HAND_TOOL_ID: &str = "hand";

/// One dispatch the owner must perform, in the order listed.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolCall {
    Down(ToolEvent),
    Move(ToolEvent),
    Up(ToolEvent),
    /// `toolManager.cancel()` — no event, the gesture is abandoned.
    Cancel,
    /// `toolManager.hover(...)`; `None` clears the preview.
    Hover(Option<HoverEvent>),
}

/// What a controller call asks the host to do.
#[derive(Clone, Debug, PartialEq)]
pub struct InputOutcome {
    pub tool_calls: Vec<ToolCall>,
    /// `setView`, already clamped. `None` means the view did not move.
    pub new_view: Option<View>,
    pub cursor: CursorKind,
    pub need_render: bool,
    /// `Some(true)` = capture this pointer, `Some(false)` = the gesture is over.
    pub capture_pointer: Option<bool>,
}

impl Default for InputOutcome {
    fn default() -> Self {
        Self {
            tool_calls: Vec::new(),
            new_view: None,
            cursor: CursorKind::Default,
            need_render: false,
            capture_pointer: None,
        }
    }
}

/// Everything the controller cannot own: the current view, the document and
/// viewport sizes it clamps against, and the active tool's two answers.
pub struct InputCtx<'a> {
    pub view: View,
    pub canvas: Option<(u32, u32)>,
    /// Stand-in for `host.getBoundingClientRect()`: a left/top-anchored size.
    pub viewport: Option<(f64, f64)>,
    pub active_tool_id: &'a str,
    pub tool_accepts_button: &'a dyn Fn(Button) -> bool,
    pub tool_cursor: CursorKind,
}

/// Which gesture [`InputController::end_draw`] closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ended {
    None,
    Pan,
    Draw,
}

/// `InputController` state: which pointer owns pan, which owns the stroke, and
/// the three transient flags that decide the cursor.
pub struct InputController {
    panning: bool,
    pan_pointer_id: Option<u64>,
    pan_last: Option<(f64, f64)>,
    draw_pointer_id: Option<u64>,
    space_down: bool,
    inside: bool,
    /// Set by the host each frame; `egui::Context::wants_keyboard_input` in W8.
    text_input_focused: bool,
    /// Last `InputCtx::tool_cursor`, so the ctx-less entry points can resolve.
    last_tool_cursor: CursorKind,
    wheel: WheelAccumulator,
}

impl Default for InputController {
    fn default() -> Self {
        Self::new()
    }
}

impl InputController {
    pub fn new() -> Self {
        Self {
            panning: false,
            pan_pointer_id: None,
            pan_last: None,
            draw_pointer_id: None,
            space_down: false,
            // JS `_inside = true`: presence starts assumed, `pointerleave` clears.
            inside: true,
            text_input_focused: false,
            last_tool_cursor: CursorKind::Default,
            wheel: WheelAccumulator::new(),
        }
    }

    // ---------------------------------------------------------------- pointer

    /// `pointerdown`: pan wins, otherwise hand the event to the tool.
    pub fn pointer_down(&mut self, raw: &RawPointer, ctx: &InputCtx) -> InputOutcome {
        // A second pointer is ignored outright — no state change, no dispatch.
        if self.draw_pointer_id.is_some() || self.panning {
            return InputOutcome::default();
        }
        self.note_ctx(ctx);
        self.inside = true;
        let mut out = InputOutcome::default();
        // Middle always pans; space-held and the hand tool make Left pan too.
        let pan = self.should_pan(raw, ctx) && matches!(raw.button, Button::Left | Button::Middle);
        if pan {
            self.panning = true;
            self.pan_pointer_id = Some(raw.pointer_id);
            self.pan_last = Some(raw.pos);
            out.capture_pointer = Some(true);
            out.cursor = self.resolve(ctx);
            return out;
        }
        if !(ctx.tool_accepts_button)(raw.button) {
            out.cursor = self.resolve(ctx);
            return out;
        }
        self.draw_pointer_id = Some(raw.pointer_id);
        out.capture_pointer = Some(true);
        out.tool_calls
            .push(ToolCall::Down(build_tool_event(raw, &ctx.view)));
        out.cursor = self.resolve(ctx);
        out
    }

    /// `pointermove`: pan, then stroke, then hover — in that priority.
    pub fn pointer_move(&mut self, raw: &RawPointer, ctx: &InputCtx) -> InputOutcome {
        self.note_ctx(ctx);
        let mut out = InputOutcome::default();
        self.sync_inside(raw, ctx);
        if self.panning && self.pan_pointer_id == Some(raw.pointer_id) {
            let (sx, sy) = raw.pos;
            let (dx, dy) = self
                .pan_last
                .map_or((0.0, 0.0), |(lx, ly)| (sx - lx, sy - ly));
            self.pan_last = Some((sx, sy));
            let mut next = pan_by(ctx.view, dx, dy);
            if let (Some((cw, ch)), Some((vw, vh))) = (ctx.canvas, ctx.viewport) {
                next = clamp_view(next, cw as f64, ch as f64, vw, vh);
            }
            out.new_view = Some(next);
            out.need_render = true;
            out.cursor = self.resolve(ctx);
            return out;
        }
        // A second live pointer only counts while it is a motionless mouse.
        if let Some(owner) = self.draw_pointer_id
            && raw.pointer_id != owner
            && (raw.kind != PointerKind::Mouse || raw.buttons_down != 0)
        {
            out.cursor = self.resolve(ctx);
            return out;
        }
        let ev = build_tool_event(raw, &ctx.view);
        if self.draw_pointer_id == Some(raw.pointer_id) {
            out.tool_calls.push(ToolCall::Move(ev));
        } else if self.draw_pointer_id.is_none() && !self.panning {
            let hover = HoverEvent {
                x: ev.x,
                y: ev.y,
                fx: ev.fx,
                fy: ev.fy,
            };
            out.tool_calls.push(ToolCall::Hover(Some(hover)));
        }
        out.cursor = self.resolve(ctx);
        out
    }

    /// `pointerup`: end whichever gesture this pointer owns.
    pub fn pointer_up(&mut self, raw: &RawPointer, ctx: &InputCtx) -> InputOutcome {
        self.note_ctx(ctx);
        let mut out = InputOutcome::default();
        match self.end_draw(raw.pointer_id) {
            Ended::None => {}
            Ended::Pan => out.capture_pointer = Some(false),
            Ended::Draw => {
                out.capture_pointer = Some(false);
                out.tool_calls
                    .push(ToolCall::Up(build_tool_event(raw, &ctx.view)));
                if !self.inside {
                    out.tool_calls.push(ToolCall::Hover(None));
                }
            }
        }
        out.cursor = self.resolve(ctx);
        out
    }

    /// `pointercancel` — the same tail, but the tool gets `cancel()` instead.
    pub fn pointer_cancel(&mut self, pointer_id: u64) -> InputOutcome {
        // No event survives a cancel, so there is nothing that needs an `InputCtx`.
        let mut out = InputOutcome::default();
        match self.end_draw(pointer_id) {
            Ended::None => {}
            Ended::Pan => out.capture_pointer = Some(false),
            Ended::Draw => {
                out.capture_pointer = Some(false);
                out.tool_calls.push(ToolCall::Cancel);
                if !self.inside {
                    out.tool_calls.push(ToolCall::Hover(None));
                }
            }
        }
        out.cursor = self.resolve_cached();
        out
    }

    /// `pointerleave`: drop presence, and the preview with it unless captured.
    pub fn pointer_leave(&mut self) -> InputOutcome {
        self.inside = false;
        let mut out = InputOutcome {
            cursor: self.resolve_cached(),
            ..Default::default()
        };
        if self.panning || self.draw_pointer_id.is_some() {
            return out;
        }
        out.tool_calls.push(ToolCall::Hover(None));
        out
    }

    // ------------------------------------------------------------------ wheel

    /// `wheel`. Shift+wheel pans on x — `deltaX` is deliberately unread (the JS
    /// comment TL-DRAW-04 Q2). Anything else accumulates into integer steps and
    /// walks the zoom ladder around `pos`.
    pub fn wheel(
        &mut self,
        delta_y: f64,
        delta_mode: WheelMode,
        shift: bool,
        pos: (f64, f64),
        ctx: &InputCtx,
    ) -> InputOutcome {
        self.note_ctx(ctx);
        let mut out = InputOutcome {
            cursor: self.resolve(ctx),
            ..Default::default()
        };
        if shift {
            // `-(e.deltaY || 0)`: a non-finite delta contributes nothing.
            let dx = if delta_y.is_finite() { -delta_y } else { 0.0 };
            out.new_view = Some(self.clamped(ctx, pan_by(ctx.view, dx, 0.0)));
            out.need_render = true;
            return out;
        }
        let page_h = ctx.viewport.map_or(0.0, |(_, h)| h);
        let steps = self.wheel.feed(delta_y, delta_mode, page_h);
        if steps == 0 {
            return out;
        }
        // `step_zoom` only ever returns a ladder rung, so this cannot fail.
        let nz = step_zoom(ctx.view.zoom(), steps);
        let zoomed = zoom_at(ctx.view, pos.0, pos.1, nz).unwrap_or(ctx.view);
        out.new_view = Some(self.clamped(ctx, zoomed));
        out.need_render = true;
        out
    }

    // -------------------------------------------------------------- keyboard

    /// `keydown`. Space is the only key that changes state here, and a focused
    /// text field swallows it first (`keyboard_bindings.js` L19-22).
    pub fn key_down(&mut self, k: &KeyEvent, ctx: &InputCtx) -> InputOutcome {
        self.key_down_focused(k, ctx, self.text_input_focused)
    }

    /// [`Self::key_down`] with the host's text-focus flag passed explicitly.
    pub fn key_down_focused(
        &mut self,
        k: &KeyEvent,
        ctx: &InputCtx,
        text_input_focused: bool,
    ) -> InputOutcome {
        self.note_ctx(ctx);
        let mut out = InputOutcome::default();
        if !text_input_focused && is_space(k) && !self.space_down {
            self.space_down = true;
        }
        out.cursor = self.resolve(ctx);
        out
    }

    /// `keyup`. The JS handler carries **no** target guard, so neither does this.
    pub fn key_up(&mut self, k: &KeyEvent, ctx: &InputCtx) -> InputOutcome {
        self.note_ctx(ctx);
        if is_space(k) {
            self.space_down = false;
        }
        InputOutcome {
            cursor: self.resolve(ctx),
            ..Default::default()
        }
    }

    /// `SETTINGS_CHANGED(activeTool)`: the tool cursor changed, re-resolve.
    pub fn active_tool_changed(&mut self, ctx: &InputCtx) -> InputOutcome {
        self.note_ctx(ctx);
        InputOutcome {
            cursor: self.resolve(ctx),
            ..Default::default()
        }
    }

    // ------------------------------------------------------------- transient

    /// `blur` / `visibilitychange`: drop every transient flag and abandon the
    /// gesture the tool is in the middle of.
    pub fn reset_transient(&mut self) -> InputOutcome {
        self.space_down = false;
        self.panning = false;
        self.pan_pointer_id = None;
        self.pan_last = None;
        self.draw_pointer_id = None;
        let mut out = InputOutcome::default();
        out.tool_calls.push(ToolCall::Cancel);
        // The visibility handler also cleared the preview when already outside.
        if !self.inside {
            out.tool_calls.push(ToolCall::Hover(None));
        }
        out.cursor = self.resolve_cached();
        out
    }

    // ----------------------------------------------------------- accessors

    pub fn set_text_input_focused(&mut self, focused: bool) {
        self.text_input_focused = focused;
    }

    pub fn text_input_focused(&self) -> bool {
        self.text_input_focused
    }

    pub fn is_panning(&self) -> bool {
        self.panning
    }

    pub fn is_drawing(&self) -> bool {
        self.draw_pointer_id.is_some()
    }

    pub fn space_down(&self) -> bool {
        self.space_down
    }

    pub fn inside(&self) -> bool {
        self.inside
    }

    // -------------------------------------------------------------- private

    fn note_ctx(&mut self, ctx: &InputCtx) {
        self.last_tool_cursor = ctx.tool_cursor;
    }

    /// `_shouldPan`: middle button, space held, or the hand tool.
    fn should_pan(&self, raw: &RawPointer, ctx: &InputCtx) -> bool {
        if raw.button == Button::Middle {
            return true;
        }
        if self.space_down {
            return true;
        }
        ctx.active_tool_id == HAND_TOOL_ID
    }

    /// `_markInside(inside = true)`.
    fn mark_inside(&mut self, inside: bool) {
        self.inside = inside;
    }

    /// `_syncInside`: with nothing captured, presence is whatever the host says;
    /// while captured, re-derive it from the host rect because the browser
    /// suppresses the boundary events during capture.
    fn sync_inside(&mut self, raw: &RawPointer, ctx: &InputCtx) {
        if !self.panning && self.draw_pointer_id.is_none() {
            self.mark_inside(true);
            return;
        }
        let Some((vw, vh)) = ctx.viewport else {
            return;
        };
        let (x, y) = raw.pos;
        self.mark_inside(!(x < 0.0 || x >= vw || y < 0.0 || y >= vh));
    }

    /// `endDraw`'s head: close whichever gesture `pointer_id` owned.
    fn end_draw(&mut self, pointer_id: u64) -> Ended {
        if self.panning && self.pan_pointer_id == Some(pointer_id) {
            self.panning = false;
            self.pan_pointer_id = None;
            self.pan_last = None;
            return Ended::Pan;
        }
        if self.draw_pointer_id != Some(pointer_id) {
            return Ended::None;
        }
        self.draw_pointer_id = None;
        Ended::Draw
    }

    /// `clampView`, but only when the document and viewport sizes are both known.
    fn clamped(&self, ctx: &InputCtx, v: View) -> View {
        match (ctx.canvas, ctx.viewport) {
            (Some((cw, ch)), Some((vw, vh))) => clamp_view(v, cw as f64, ch as f64, vw, vh),
            _ => v,
        }
    }

    fn resolve(&self, ctx: &InputCtx) -> CursorKind {
        cursor::resolve_cursor(self.inputs(ctx.tool_cursor))
    }

    /// The ctx-less entry points lean on the last cursor the host reported.
    fn resolve_cached(&self) -> CursorKind {
        cursor::resolve_cursor(self.inputs(self.last_tool_cursor))
    }

    fn inputs(&self, tool_cursor: CursorKind) -> CursorInputs {
        CursorInputs {
            inside: self.inside,
            panning: self.panning,
            space_down: self.space_down,
            tool_cursor: Some(tool_cursor),
        }
    }
}
