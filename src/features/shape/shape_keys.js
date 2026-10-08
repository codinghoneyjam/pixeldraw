// <META - FILE SUMMARY - ShapeTool keyboard interaction: commit, discard, placing, nudge>
//
// Split out of shape.js: the keyDown state dispatch. Takes the tool and calls
// back into its public verbs (commit/cancel/discardPending) plus the placing
// flow in shape_pending.js, mirroring the shape_render.js convention.

import { finishPlacing } from "./shape_pending.js";

// <META - ROLE : keyboard commit, discard, placing finish, and arrow nudge | L9-60>
export function handleKeyDown(tool, ev) {
  if (ev.key === "Enter") {
    if (tool._mode === "placing") return finishPlacing(tool);
    if (tool._mode === "drawing" || !tool.hasPending()) return false;
    tool._op = null;
    tool._mode = "pending";
    tool.commit();
    return true;
  }
  if (ev.key === "Escape" || ev.key === "Delete" || ev.key === "Backspace") {
    if (tool._mode === "placing" && tool._place) {
      if (ev.key === "Escape" && tool._place.points.length > 1) {
        // Esc steps back one vertex; the last click is what the user regrets.
        tool._place.points.pop();
        tool._emitChanged();
        tool._render();
        return true;
      }
      tool._place = null;
      tool._hoverPt = null;
      tool._mode = tool._pending ? "pending" : "idle";
      tool._emitChanged();
      tool._render();
      return true;
    }
    if ((tool._mode === "drawing" || tool._mode === "resizing" || tool._mode === "moving") && ev.key === "Escape") {
      tool.cancel();
      return true;
    }
    if (tool.hasPending() || tool._draw || tool._op) {
      tool.discardPending();
      return true;
    }
    return false;
  }
  const step = ev.shift ? 32 : 1;
  const d = ev.key === "ArrowLeft" ? [-step, 0] : ev.key === "ArrowRight" ? [step, 0] : ev.key === "ArrowUp" ? [0, -step] : ev.key === "ArrowDown" ? [0, step] : null;
  if (!d || !tool.hasPending() || tool._mode !== "pending") return false;
  const p = tool._pending;
  if (tool._kind === "polygon") {
    tool._pending = { ...p, points: p.points.map((q) => ({ x: q.x + d[0], y: q.y + d[1] })) };
  } else if (tool._kind === "line") {
    tool._pending = { ...p, p0: { x: p.p0.x + d[0], y: p.p0.y + d[1] }, p1: { x: p.p1.x + d[0], y: p.p1.y + d[1] } };
  } else {
    tool._pending = { ...p, bbox: { ...p.bbox, x: p.bbox.x + d[0], y: p.bbox.y + d[1] } };
  }
  tool._emitChanged();
  tool._render();
  return true;
}
