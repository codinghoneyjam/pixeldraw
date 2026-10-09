// <META - FILE SUMMARY - Cross-cutting actions: undo/redo with pending guard, canvas resize>
// Only the commands that span features stay here. The tool-specific ones moved
// next to their feature: features/viewport/view_actions.js (zoom, grid),
// features/color/color_actions.js (swap, reset), features/pen/brush_actions.js
// (pen size) and features/layers/layer_actions.js (layer ops).
import { showResizeCanvasDialog } from "../shared/dialogs.js";

// <META - ROLE : Undo, unless a tool holds a pending shape - then discard it first | L8-17>
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

// <META - ROLE : Redo, blocked while a shape is still pending | L19-26>
export function requestRedo(session, toolManager) {
  try {
    const t = toolManager ? toolManager.active : null;
    if (t && typeof t.hasPending === "function" && t.hasPending()) return;
  } catch { /* fall through */ }
  session.redo();
}

// <META - ROLE : Canvas resize dialog action | L28-34>
export async function canvasResize(session) {
  const doc = session.doc;
  if (!doc) return;
  const res = await showResizeCanvasDialog({ widthPx: doc.canvas.widthPx, heightPx: doc.canvas.heightPx });
  if (res) session.resizeCanvas(res.widthPx, res.heightPx);
}
