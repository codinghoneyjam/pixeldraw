// <META - FILE SUMMARY - buildToolEvent: DOM pointer event -> ToolEvent (pixel/float/screen)>
//
// Extracted from input_controller.js: pure mapping, no listener wiring.

import { pixelAt, screenToCanvas } from "../features/viewport/view.js";

/**
 * Pixel-path pointer event builder (pan/zoom-scoped contract, frozen).
 * Input side of the input/shape boundary: cursor fns stay here,
 * never dragged into `core/model` or `core/raster`.
 * @param {object} domEv - DOM pointer event (clientX/clientY path only).
 * @param {object} view - Current view (screenToCanvas/pixelAt mapping).
 * @param {object} hostRect - Host bounding rect ({left, top}).
 * @returns {object} ToolEvent with pixel/float/screen coords.
 */
export function buildToolEvent(domEv, view, hostRect) {
  const sx = domEv.clientX - hostRect.left;
  const sy = domEv.clientY - hostRect.top;
  const f = screenToCanvas(view, sx, sy);
  const p = pixelAt(view, sx, sy);
  let rawList = [];
  try {
    const coalesced = domEv.getCoalescedEvents?.();
    if (Array.isArray(coalesced) && coalesced.length > 0) rawList = coalesced;
    else rawList = [domEv];
  } catch {
    rawList = [domEv];
  }
  const pts = [];
  for (const e of rawList) {
    const ex = typeof e.clientX === "number" ? e.clientX - hostRect.left : sx;
    const ey = typeof e.clientY === "number" ? e.clientY - hostRect.top : sy;
    const ef = screenToCanvas(view, ex, ey);
    const ep = { x: Math.floor(ef.x), y: Math.floor(ef.y) };
    const last = pts[pts.length - 1];
    if (last && last.x === ep.x && last.y === ep.y) continue;
    pts.push(ep);
  }
  if (pts.length === 0) pts.push({ x: p.x, y: p.y });
  else {
    const last = pts[pts.length - 1];
    if (last.x !== p.x || last.y !== p.y) pts.push({ x: p.x, y: p.y });
  }
  return {
    x: p.x,
    y: p.y,
    fx: f.x,
    fy: f.y,
    sx,
    sy,
    button: domEv.button ?? 0,
    shift: domEv.shiftKey === true,
    ctrl: domEv.ctrlKey === true || domEv.metaKey === true,
    alt: domEv.altKey === true,
    pointerId: domEv.pointerId,
    pointerType: domEv.pointerType ?? "mouse",
    coalesced: pts,
    timeStamp: domEv.timeStamp ?? Date.now(),
  };
}