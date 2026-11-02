// <META - FILE SUMMARY - View actions: zoom/fit/actual + grid overlay cycle>
// Menu/shortcut commands for what the canvas shows. They receive the store or
// the session as an argument, so the action table wires them without knowing
// which of the two a command needs.
import { GRID_MODES } from "../../core/constants.js";

// <META - ROLE : Zoom one step in/out around the viewport centre | L9-10>
export function zoomIn(viewStore) { viewStore.zoomIn(); }

// <META - ROLE : Zoom one step out | L12-13>
export function zoomOut(viewStore) { viewStore.zoomOut(); }

// <META - ROLE : Fit the document into the viewport | L15-16>
export function fit(viewStore) { viewStore.fit(); }

// <META - ROLE : Restore 100% (1 device px per canvas px) | L18-19>
export function actual(viewStore) { viewStore.actual(); }

// <META - ROLE : Cycle the grid overlay off -> unit -> tile -> pixel | L21-26>
export function gridCycle(session) {
  const cur = session.settings.gridMode;
  const next = GRID_MODES[(GRID_MODES.indexOf(cur) + 1) % GRID_MODES.length];
  session.setSetting("gridMode", next);
}
