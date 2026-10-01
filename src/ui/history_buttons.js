// <META - FILE SUMMARY - Top-bar undo/redo enable state from history caps>
import { EVENTS } from "../core/events.js";

// <META - ROLE : Top-bar undo/redo enable state from history caps, inert under a dialog | L1-46>
export function createHistoryButtons(root, session) {
  const noop = () => {};
  if (!root || !session || typeof document === "undefined") return noop;
  const undoBtn = root.querySelector('.dt-menubar-actions button[data-action="edit.undo"]');
  const redoBtn = root.querySelector('.dt-menubar-actions button[data-action="edit.redo"]');
  if (!undoBtn && !redoBtn) return noop;
  const disposers = [];
  const on = (t, type, fn) => {
    t.addEventListener(type, fn);
    disposers.push(() => t.removeEventListener(type, fn));
  };
  const app = document.getElementById("dt-app");
  const dialogRoot = document.getElementById("dt-dialog-root");
  let observer = null;

  function isBusy() {
    if (dialogRoot && dialogRoot.querySelector("[role='dialog']")) return true;
    return app !== null && app.getAttribute("aria-busy") === "true";
  }
  function sync() {
    const busy = isBusy();
    let u = false;
    let r = false;
    try {
      u = session.history.canUndo();
      r = session.history.canRedo();
    } catch {
      u = false;
      r = false;
    }
    if (undoBtn) undoBtn.disabled = busy || !u;
    if (redoBtn) redoBtn.disabled = busy || !r;
  }
  on(session, EVENTS.HISTORY_CHANGED, sync);
  on(session, EVENTS.DOCUMENT_REPLACED, sync);
  if (typeof MutationObserver === "function") {
    observer = new MutationObserver(sync);
    if (dialogRoot) observer.observe(dialogRoot, { childList: true });
    if (app) observer.observe(app, { attributes: true, attributeFilter: ["aria-busy"] });
  }
  sync();
  return () => {
    if (observer) observer.disconnect();
    for (const d of disposers) {
      try { d(); } catch { /* ignore */ }
    }
    disposers.length = 0;
  };
}
