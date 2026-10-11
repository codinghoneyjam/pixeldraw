//! Shared input event payloads used across all native tools.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind {
    Mouse,
    Pen,
    Touch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolEvent {
    pub x: i32,
    pub y: i32,
    pub fx: f64,
    pub fy: f64,
    pub sx: f64,
    pub sy: f64,
    pub button: Button,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub pointer_id: u64,
    pub pointer_kind: PointerKind,
    pub coalesced: Vec<(i32, i32)>,
    pub time_ms: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoverEvent {
    pub x: i32,
    pub y: i32,
    pub fx: f64,
    pub fy: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyEvent {
    pub key: String,
    pub code: String,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Default for ToolEvent {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            fx: 0.0,
            fy: 0.0,
            sx: 0.0,
            sy: 0.0,
            button: Button::Left,
            shift: false,
            ctrl: false,
            alt: false,
            pointer_id: 0,
            pointer_kind: PointerKind::Mouse,
            coalesced: vec![],
            time_ms: 0.0,
        }
    }
}
