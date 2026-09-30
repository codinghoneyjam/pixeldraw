// Event catalog — signal contract (docs/core.md, events.js).

export const EVENTS = Object.freeze({
  DOCUMENT_REPLACED: "document-replaced",
  LAYERS_CHANGED: "layers-changed",
  PIXELS_CHANGED: "pixels-changed",
  HISTORY_CHANGED: "history-changed",
  SETTINGS_CHANGED: "settings-changed",
  STATUS_MESSAGE: "status-message",
  TOOL_STATE: "tool-state",
});

export const SETTING_KEYS = Object.freeze([
  "primaryColor",
  "secondaryColor",
  "penSize",
  "activeTool",
  "gridMode",
  "snapUnit",
  "shapeFill",
  "shapeRadius",
  "shapeLockAspect",
]);
