//! `features/hand/hand.js` — the pan tool.
//!
//! Eleven lines in the JS, none of them behaviour: panning itself lives in the
//! `InputController` gate, so the tool is an id and a cursor.

use crate::tools::api::{CursorKind, Tool};

pub struct HandTool;

impl HandTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HandTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for HandTool {
    fn id(&self) -> &'static str {
        "hand"
    }

    fn cursor(&self) -> CursorKind {
        CursorKind::Grab
    }
}
