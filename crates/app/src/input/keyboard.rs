//! `src/tools/keyboard_bindings.js` — the key policy the state machine runs on.
//!
//! The window/document listeners are gone. What survives is the two decisions
//! they made: which key counts as Space, and that a focused text field
//! swallows keys. The host reports focus as a flag, not as a DOM target.

use crate::input::event::KeyEvent;

pub const SPACE_KEY: &str = " ";
pub const SPACE_CODE: &str = "Space";

/// `e.code === "Space" || e.key === " "`.
pub fn is_space(k: &KeyEvent) -> bool {
    k.code == SPACE_CODE || k.key == SPACE_KEY
}

/// `keyboard_bindings.js` L19-22: `<input>` / `<textarea>` / contentEditable
/// targets swallow the keydown — Space-pan included.
pub fn text_target_swallows_key(tag: &str, content_editable: bool) -> bool {
    let tag = tag.to_ascii_lowercase();
    tag == "input" || tag == "textarea" || content_editable
}
