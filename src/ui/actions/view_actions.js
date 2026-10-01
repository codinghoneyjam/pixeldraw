// <META - FILE SUMMARY - View actions: zoomIn, zoomOut, fit, actual, gridCycle>
import { showResizeCanvasDialog } from "../dialogs.js";

const GRID_CYCLE = ["off", "unit", "tile", "pixel"];

// <META - ROLE : View zoom/fit/actual actions | L1-22>
export function zoomIn(viewStore) { viewStore.zoomIn(); }
export function zoomOut(viewStore) { viewStore.zoomOut(); }
export function fit(viewStore) { viewStore.fit(); }
export function actual(viewStore) { viewStore.actual(); }

export function gridCycle(session) {
  const cur = session.settings.gridMode;
  const next = GRID_CYCLE[(GRID_CYCLE.indexOf(cur) + 1) % GRID_CYCLE.length];
  session.setSetting("gridMode", next);
}

// <META - ROLE : Canvas resize dialog action | L24-32>
export async function canvasResize(session) {
  const doc = session.doc;
  if (!doc) return;
  const res = await showResizeCanvasDialog({ widthPx: doc.canvas.widthPx, heightPx: doc.canvas.heightPx });
  if (res) session.resizeCanvas(res.widthPx, res.heightPx);
}
