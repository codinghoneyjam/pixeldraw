//! Input controller messages and host context.

use crate::input::event::{Button, HoverEvent, ToolEvent};
use crate::tools::CursorKind;
use pixel_engine::view::View;

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
