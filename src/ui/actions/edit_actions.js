// <META - FILE SUMMARY - Merged edit/tool/view actions: undo/redo, tool & color, view commands>

import { showResizeCanvasDialog } from "../dialogs.js";

const GRID_CYCLE = ["off", "unit", "tile", "pixel"];

// <META - ROLE : Undo with pending tool check | L8-17>
export function requestUndo(session, toolManager) {
  try {
    const t = toolManager ? toolManager.active : null;
    if (t && typeof t.hasPending === "function" && t.hasPending()) {
      t.discardPending();
      return;
    }
  } catch { /* fall through */ }
  session.undo();
}

// <META - ROLE : Redo with pending tool check | L20-26>
export function requestRedo(session, toolManager) {
  try {
    const t = toolManager ? toolManager.active : null;
    if (t && typeof t.hasPending === "function" && t.hasPending()) return;
  } catch { /* fall through */ }
  session.redo();
}

// <META - ROLE : Brush size step and color swap/reset actions | L29-43>
export function brushStep(session, delta) {
  const cur = session.settings.penSize;
  session.setSetting("penSize", Math.max(1, Math.min(64, cur + delta)));
}

export function colorSwap(session) {
  const s = session.settings;
  session.setSetting("primaryColor", s.secondaryColor);
  session.setSetting("secondaryColor", s.primaryColor);
}

export function colorReset(session) {
  session.setSetting("primaryColor", "#000000");
  session.setSetting("secondaryColor", "#ffffff");
}

// <META - ROLE : View zoom/fit/actual actions | L46-49>
export function zoomIn(viewStore) { viewStore.zoomIn(); }
export function zoomOut(viewStore) { viewStore.zoomOut(); }
export function fit(viewStore) { viewStore.fit(); }
export function actual(viewStore) { viewStore.actual(); }

export function gridCycle(session) {
  const cur = session.settings.gridMode;
  const next = GRID_CYCLE[(GRID_CYCLE.indexOf(cur) + 1) % GRID_CYCLE.length];
  session.setSetting("gridMode", next);
}

// <META - ROLE : Canvas resize dialog action | L57-63>
export async function canvasResize(session) {
  const doc = session.doc;
  if (!doc) return;
  const res = await showResizeCanvasDialog({ widthPx: doc.canvas.widthPx, heightPx: doc.canvas.heightPx });
  if (res) session.resizeCanvas(res.widthPx, res.heightPx);
}