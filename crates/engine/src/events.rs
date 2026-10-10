//! Event catalog — signal contract (docs/core.md, events.js).
//!
//! The JS tree dispatched these as `CustomEvent` on an `EventTarget` Session.
//! In the native port the app layer subscribes to the same seven signals; the
//! engine only needs the names, which is all this module holds.

/// The seven signals, frozen.
pub const EVENT_DOCUMENT_REPLACED: &str = "document-replaced";
pub const EVENT_LAYERS_CHANGED: &str = "layers-changed";
pub const EVENT_PIXELS_CHANGED: &str = "pixels-changed";
pub const EVENT_HISTORY_CHANGED: &str = "history-changed";
pub const EVENT_SETTINGS_CHANGED: &str = "settings-changed";
pub const EVENT_STATUS_MESSAGE: &str = "status-message";
pub const EVENT_TOOL_STATE: &str = "tool-state";

pub const EVENTS: [&str; 7] = [
    EVENT_DOCUMENT_REPLACED,
    EVENT_LAYERS_CHANGED,
    EVENT_PIXELS_CHANGED,
    EVENT_HISTORY_CHANGED,
    EVENT_SETTINGS_CHANGED,
    EVENT_STATUS_MESSAGE,
    EVENT_TOOL_STATE,
];

/// The nine settings keys that carry a change notification, frozen.
pub const SETTING_KEYS: [&str; 9] = [
    "primaryColor",
    "secondaryColor",
    "penSize",
    "activeTool",
    "gridMode",
    "snapUnit",
    "shapeFill",
    "shapeRadius",
    "shapeLockAspect",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_shape() {
        assert_eq!(EVENTS.len(), 7);
        assert_eq!(SETTING_KEYS.len(), 9);
        assert!(EVENTS.contains(&EVENT_PIXELS_CHANGED));
        assert!(SETTING_KEYS.contains(&"shapeLockAspect"));
    }
}
