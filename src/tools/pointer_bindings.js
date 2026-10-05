// <META - FILE SUMMARY - Host pointer/wheel listener wiring for InputController (pan, stroke, hover)>
//
// Extracted from input_controller.js: listener registration only. Every decision
// (should-pan, button acceptance, cursor) stays on the controller instance.

import { clampView, panBy, stepZoom, zoomAt } from "../render/view.js";
import { buildToolEvent } from "./pointer_event.js";

/**
 * Register every host-scoped pointer and wheel listener on `ctrl`.
 * @param {object} ctrl InputController instance
 * @returns {{dispose:function}} handle that removes what was registered
 */
export function bindPointerInput(ctrl) {
  const h = ctrl.host;
  const bound = {};
  try {
    h.style.touchAction = "none";
  } catch {
    // ignore
  }
  const on = (type, fn, opts) => {
    h.addEventListener(type, fn, opts);
    bound[type] = fn;
  };

  on("contextmenu", (e) => e.preventDefault());

  // <META - ROLE : pointer down: pan wins, else dispatch to the tool | L29-54>
  on("pointerdown", (e) => {
    if (ctrl._drawPointerId !== null || ctrl._panning) return;
    ctrl._markInside();
    const view = ctrl.getView();
    const rect = ctrl._hostRect();
    if (e.button === 1) e.preventDefault?.();
    if (ctrl._shouldPan(e) && (e.button === 0 || e.button === 1)) {
      ctrl._panning = true;
      ctrl._panPointerId = e.pointerId;
      ctrl._panLast = { x: e.clientX - rect.left, y: e.clientY - rect.top };
      try {
        h.setPointerCapture(e.pointerId);
      } catch {
        // ignore
      }
      ctrl._applyCursor();
      return;
    }
    if (!ctrl._toolAcceptsButton(e.button)) return;
    ctrl._drawPointerId = e.pointerId;
    try {
      h.setPointerCapture(e.pointerId);
    } catch {
      // ignore
    }
    ctrl.toolManager.pointerDown(buildToolEvent(e, view, rect));
  });

  on("pointerenter", () => ctrl._markInside());
  on("pointermove", (e) => {
    const view = ctrl.getView();
    const rect = ctrl._hostRect();
    // <META - ROLE : capture suppresses boundary events, so re-derive in/out from rect | L62-63>
    ctrl._syncInside(e, rect);
    if (ctrl._panning && e.pointerId === ctrl._panPointerId) {
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;
      const dx = sx - ctrl._panLast.x;
      const dy = sy - ctrl._panLast.y;
      ctrl._panLast = { x: sx, y: sy };
      const doc = ctrl.session.doc;
      const vp = ctrl.getViewportSize();
      let next = panBy(view, dx, dy);
      if (doc && vp) next = clampView(next, doc.canvas.widthPx, doc.canvas.heightPx, vp.w, vp.h);
      ctrl.setView(next);
      ctrl.requestRender();
      return;
    }
    if (e.pointerId !== ctrl._drawPointerId && ctrl._drawPointerId !== null) {
      if (e.pointerType !== "mouse" || (e.buttons ?? 0) !== 0) return;
    }
    const te = buildToolEvent(e, view, rect);
    if (ctrl._drawPointerId !== null && e.pointerId === ctrl._drawPointerId) {
      ctrl.toolManager.pointerMove(te);
      return;
    }
    if (ctrl._drawPointerId === null && !ctrl._panning) {
      ctrl.toolManager.hover({ x: te.x, y: te.y, fx: te.fx, fy: te.fy });
      try {
        ctrl.onHover({ x: te.x, y: te.y });
      } catch {
        // ignore
      }
    }
  });

  // <META - ROLE : end a pan/stroke, then re-derive the cursor from current state | L100-118>
  const endDraw = (e, kind) => {
    if (ctrl._panning && e.pointerId === ctrl._panPointerId) {
      ctrl._panning = false;
      ctrl._panPointerId = null;
      ctrl._panLast = null;
      ctrl._applyCursor();
      return;
    }
    if (e.pointerId !== ctrl._drawPointerId) return;
    const view = ctrl.getView();
    const rect = ctrl._hostRect();
    const te = buildToolEvent(e, view, rect);
    ctrl._drawPointerId = null;
    if (kind === "up") ctrl.toolManager.pointerUp(te);
    else ctrl.toolManager.cancel();
    if (!ctrl._inside) ctrl._clearHover();
    ctrl._applyCursor();
  };
  on("pointerup", (e) => endDraw(e, "up"));
  on("pointercancel", (e) => endDraw(e, "cancel"));
  on("lostpointercapture", (e) => {
    if (ctrl._panning && e.pointerId === ctrl._panPointerId) {
      ctrl._panning = false;
      ctrl._panPointerId = null;
      ctrl._panLast = null;
      ctrl._applyCursor();
      return;
    }
    if (e.pointerId !== ctrl._drawPointerId) return;
    ctrl._drawPointerId = null;
    ctrl.toolManager.cancel();
    if (!ctrl._inside) ctrl._clearHover();
    ctrl._applyCursor();
  });
  // <META - ROLE : leave => default arrow + no stale preview; re-enter re-applies tool | L135-139>
  on("pointerleave", () => {
    ctrl._markInside(false);
    if (ctrl._panning || ctrl._drawPointerId !== null) return;
    ctrl._clearHover();
  });

  on(
    "wheel",
    (e) => {
      e.preventDefault();
      const view = ctrl.getView();
      const rect = ctrl._hostRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;
      // <META - ROLE : wheel: Shift+wheel x-pan, otherwise accumulate -> zoom | L146-172>
      // NOTE (TL-DRAW-04 Q2): `deltaX` is intentionally unread here -- trackpad
      // horizontal scroll is ignored/misrouted via the Shift+wheel `-deltaY`
      // mapping below. Any future `deltaX` support MUST land as an
      // explicitly-flagged additive branch, not a silent rewrite of `panBy`.
      if (e.shiftKey === true) {
        const doc = ctrl.session.doc;
        const vp = ctrl.getViewportSize();
        let next = panBy(view, -(e.deltaY || 0), 0);
        if (doc && vp) next = clampView(next, doc.canvas.widthPx, doc.canvas.heightPx, vp.w, vp.h);
        ctrl.setView(next);
        ctrl.requestRender();
        return;
      }
      const pageH = ctrl.getViewportSize?.()?.h ?? 0;
      const steps = ctrl._wheel.feed(e.deltaY ?? 0, e.deltaMode ?? 0, pageH);
      if (steps === 0) return;
      const doc = ctrl.session.doc;
      const vp = ctrl.getViewportSize();
      const nz = stepZoom(view.zoom, steps);
      let next = zoomAt(view, sx, sy, nz);
      if (doc && vp) next = clampView(next, doc.canvas.widthPx, doc.canvas.heightPx, vp.w, vp.h);
      ctrl.setView(next);
      ctrl.requestRender();
    },
    { passive: false },
  );

  return {
    dispose() {
      for (const [type, fn] of Object.entries(bound)) {
        try {
          h.removeEventListener(type, fn);
        } catch {
          // ignore
        }
      }
    },
  };
}