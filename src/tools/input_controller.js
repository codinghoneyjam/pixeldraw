// <META - FILE SUMMARY - InputController: DOM pointer plumbing, pan/zoom, tool dispatch>

import { EVENTS } from "../core/events.js";
import { clampView, panBy, pixelAt, screenToCanvas, stepZoom, zoomAt } from "../render/view.js";

/**
 * Input-domain wheel accumulator (TL-DRAW-04 Slice D split).
 * Pure input side of the input/shape boundary: no `core/raster`
 * import, no cursor fns. Shape rasterization lives in
 * `core/raster/shape_raster.js` (TL-DRAW-03 owns it).
 */
export class WheelAccumulator {
  constructor() {
    this._residue = 0;
    this._dir = 0;
  }

  reset() {
    this._residue = 0;
    this._dir = 0;
  }

  /**
   * Accumulate a vertical wheel delta into -step units (pure input math).
   * @param {number} deltaY - Vertical wheel delta in event units.
   * @param {number} [deltaMode=0] - DOM deltaMode (0 px, 1 line x16, 2 page x pageHeight).
   * @param {number} [pageHeight=0] - Viewport height px for deltaMode 2.
   * @returns {number} Negative step count, or 0 when below threshold.
   */
  feed(deltaY, deltaMode = 0, pageHeight = 0) {
    let px = Number(deltaY);
    if (!Number.isFinite(px) || px === 0) return 0;
    if (deltaMode === 1) px *= 16;
    else if (deltaMode === 2) px *= Number(pageHeight) || 0;
    if (px === 0) return 0;
    const dir = Math.sign(px);
    if (this._dir !== 0 && dir !== this._dir) {
      this._residue = px;
    } else {
      this._residue += px;
    }
    this._dir = dir;
    const steps = Math.trunc(this._residue / 60);
    if (steps !== 0) {
      this._residue -= steps * 60;
      return -steps;
    }
    return 0;
  }
}

/**
 * Pixel-path pointer event builder (pan/zoom-scoped contract, frozen).
 * Input side of the input/shape boundary: cursor fns stay here,
 * never dragged into `core/model` or `core/raster`.
 * @param {object} domEv - DOM pointer event (clientX/clientY path only).
 * @param {object} view - Current view (screenToCanvas/pixelAt mapping).
 * @param {object} hostRect - Host bounding rect ({left, top}).
 * @returns {object} ToolEvent with pixel/float/screen coords.
 */
function buildToolEvent(domEv, view, hostRect) {
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

export class InputController {
  constructor({ host, session, toolManager, getView, setView, getViewportSize, onHover = () => {}, requestRender = () => {} } = {}) {
    this.host = host;
    this.session = session;
    this.toolManager = toolManager;
    this.getView = getView;
    this.setView = setView;
    this.getViewportSize = getViewportSize;
    this.onHover = onHover;
    this.requestRender = requestRender;
    this._attached = false;
    this._panning = false;
    this._panPointerId = null;
    this._panLast = null;
    this._drawPointerId = null;
    this._spaceDown = false;
    this._inside = true;
    this._wheel = new WheelAccumulator();
    this._bound = {};
  }

  // <META - ROLE : pan gating: middle button, space-held, or the hand tool | L106-115>
  _shouldPan(domEv) {
    if (domEv.button === 1) return true;
    if (this._spaceDown) return true;
    try {
      if (this.toolManager.active.id === "hand") return true;
    } catch {
      // ignore
    }
    return false;
  }

  // <META - ROLE : ask the active tool which buttons it wants; left-only fallback | L118-131>
  _toolAcceptsButton(button) {
    let tool = null;
    try {
      tool = this.toolManager?.active ?? null;
    } catch {
      return button === 0;
    }
    if (!tool || typeof tool.acceptsButton !== "function") return button === 0;
    try {
      return tool.acceptsButton(button) === true;
    } catch {
      return false;
    }
  }

  _hostRect() {
    try {
      return this.host.getBoundingClientRect();
    } catch {
      return { left: 0, top: 0 };
    }
  }

  _resolveToolCursor() {
    try {
      const c = this.toolManager?.cursor;
      return typeof c === "string" && c.length > 0 ? c : "default";
    } catch {
      return "default";
    }
  }

  // <META - ROLE : cursor state machine - outside wins over panning/space/tool | L152-166>
  _applyCursor() {
    if (!this.host) return;
    let next;
    if (!this._inside) next = "default";
    else if (this._panning) next = "grabbing";
    else if (this._spaceDown) next = "grab";
    else next = this._resolveToolCursor();
    try {
      this.host.style.cursor = next;
    } catch {
      // ignore
    }
  }

  // <META - ROLE : pointer presence flag; every transition re-applies the cursor | L166-172>
  _markInside(inside = true) {
    this._inside = inside;
    this._applyCursor();
  }

  // <META - ROLE : shared "no preview" path so leave/cancel never leave stale hover | L182-191>
  _clearHover() {
    this.toolManager.hover(null);
    try {
      this.onHover(null);
    } catch {
      // ignore
    }
  }

  // <META - ROLE : while outside, decide from the rect whether a captured move came back | L193-205>
  _syncInside(domEv, rect) {
    if (!this._panning && this._drawPointerId === null) {
      this._markInside();
      return;
    }
    if (typeof rect.right !== "number" || typeof rect.bottom !== "number") return;
    const { clientX: x, clientY: y } = domEv;
    if (x < rect.left || x >= rect.right || y < rect.top || y >= rect.bottom) this._markInside(false);
    else this._markInside();
  }

  attach() {
    if (this._attached || !this.host) return;
    this._attached = true;
    const h = this.host;
    try {
      h.style.touchAction = "none";
    } catch {
      // ignore
    }
    const on = (type, fn, opts) => {
      h.addEventListener(type, fn, opts);
      this._bound[type] = fn;
    };
    this._bound._win = {};
    const won = (type, fn, opts) => {
      window.addEventListener(type, fn, opts);
      this._bound._win[type] = fn;
    };

    on("contextmenu", (e) => e.preventDefault());

    // <META - ROLE : pointer down: pan wins, else dispatch to the tool if it accepts the button | L185-210>
    on("pointerdown", (e) => {
      if (this._drawPointerId !== null || this._panning) return;
      this._markInside();
      const view = this.getView();
      const rect = this._hostRect();
      if (e.button === 1) e.preventDefault?.();
      if (this._shouldPan(e) && (e.button === 0 || e.button === 1)) {
        this._panning = true;
        this._panPointerId = e.pointerId;
        this._panLast = { x: e.clientX - rect.left, y: e.clientY - rect.top };
        try {
          h.setPointerCapture(e.pointerId);
        } catch {
          // ignore
        }
        this._applyCursor();
        return;
      }
      if (!this._toolAcceptsButton(e.button)) return;
      this._drawPointerId = e.pointerId;
      try {
        h.setPointerCapture(e.pointerId);
      } catch {
        // ignore
      }
      this.toolManager.pointerDown(buildToolEvent(e, view, rect));
    });

    on("pointerenter", () => this._markInside());
    on("pointermove", (e) => {
      const view = this.getView();
      const rect = this._hostRect();
      // <META - ROLE : capture suppresses boundary events, so re-derive in/out from the rect | L242-243>
      this._syncInside(e, rect);
      if (this._panning && e.pointerId === this._panPointerId) {
        const sx = e.clientX - rect.left;
        const sy = e.clientY - rect.top;
        const dx = sx - this._panLast.x;
        const dy = sy - this._panLast.y;
        this._panLast = { x: sx, y: sy };
        const doc = this.session.doc;
        const vp = this.getViewportSize();
        let next = panBy(view, dx, dy);
        if (doc && vp) next = clampView(next, doc.canvas.widthPx, doc.canvas.heightPx, vp.w, vp.h);
        this.setView(next);
        this.requestRender();
        return;
      }
      if (e.pointerId !== this._drawPointerId && this._drawPointerId !== null) {
        if (e.pointerType !== "mouse" || (e.buttons ?? 0) !== 0) return;
      }
      const te = buildToolEvent(e, view, rect);
      if (this._drawPointerId !== null && e.pointerId === this._drawPointerId) {
        this.toolManager.pointerMove(te);
        return;
      }
      if (this._drawPointerId === null && !this._panning) {
        this.toolManager.hover({ x: te.x, y: te.y, fx: te.fx, fy: te.fy });
        try {
          this.onHover({ x: te.x, y: te.y });
        } catch {
          // ignore
        }
      }
    });

    // <META - ROLE : end a pan/stroke, then re-derive the cursor from current state | L273-296>
    const endDraw = (e, kind) => {
      if (this._panning && e.pointerId === this._panPointerId) {
        this._panning = false;
        this._panPointerId = null;
        this._panLast = null;
        this._applyCursor();
        return;
      }
      if (e.pointerId !== this._drawPointerId) return;
      const view = this.getView();
      const rect = this._hostRect();
      const te = buildToolEvent(e, view, rect);
      this._drawPointerId = null;
      if (kind === "up") this.toolManager.pointerUp(te);
      else this.toolManager.cancel();
      if (!this._inside) this._clearHover();
      this._applyCursor();
    };
    on("pointerup", (e) => endDraw(e, "up"));
    on("pointercancel", (e) => endDraw(e, "cancel"));
    on("lostpointercapture", (e) => {
      if (this._panning && e.pointerId === this._panPointerId) {
        this._panning = false;
        this._panPointerId = null;
        this._panLast = null;
        this._applyCursor();
        return;
      }
      if (e.pointerId !== this._drawPointerId) return;
      this._drawPointerId = null;
      this.toolManager.cancel();
      if (!this._inside) this._clearHover();
      this._applyCursor();
    });
    // <META - ROLE : leave => default arrow + no stale preview; re-enter re-applies tool | L308-315>
    on("pointerleave", () => {
      this._markInside(false);
      if (this._panning || this._drawPointerId !== null) return;
      this._clearHover();
    });

    on(
      "wheel",
      (e) => {
        e.preventDefault();
        const view = this.getView();
        const rect = this._hostRect();
        const sx = e.clientX - rect.left;
        const sy = e.clientY - rect.top;
        // <META - ROLE : Q2 confirmed gap: deltaX never read; trackpad horizontal scroll ignored, Shift+wheel maps -deltaY to x-pan; additive deltaX branch only, never silent rewrite | L332-340>
        // NOTE (TL-DRAW-04 Q2): `deltaX` is intentionally unread here — trackpad
        // horizontal scroll is ignored/misrouted via the Shift+wheel `-deltaY`
        // mapping below. Any future `deltaX` support MUST land as an
        // explicitly-flagged additive branch, not a silent rewrite of `panBy`.
        if (e.shiftKey === true) {
          const doc = this.session.doc;
          const vp = this.getViewportSize();
          let next = panBy(view, -(e.deltaY || 0), 0);
          if (doc && vp) next = clampView(next, doc.canvas.widthPx, doc.canvas.heightPx, vp.w, vp.h);
          this.setView(next);
          this.requestRender();
          return;
        }
        const pageH = this.getViewportSize?.()?.h ?? 0;
        const steps = this._wheel.feed(e.deltaY ?? 0, e.deltaMode ?? 0, pageH);
        if (steps === 0) return;
        const doc = this.session.doc;
        const vp = this.getViewportSize();
        const nz = stepZoom(view.zoom, steps);
        let next = zoomAt(view, sx, sy, nz);
        if (doc && vp) next = clampView(next, doc.canvas.widthPx, doc.canvas.heightPx, vp.w, vp.h);
        this.setView(next);
        this.requestRender();
      },
      { passive: false },
    );

    won("keydown", (e) => {
      const t = e.target;
      const tag = t && t.tagName ? String(t.tagName).toLowerCase() : "";
      if (tag === "input" || tag === "textarea" || t?.isContentEditable === true) return;
      if (e.code === "Space" || e.key === " ") {
        if (!this._spaceDown) {
          this._spaceDown = true;
          if (!this._panning && this._drawPointerId === null) {
            this._applyCursor();
          }
        }
        e.preventDefault?.();
      }
    });
    won("keyup", (e) => {
      if (e.code === "Space" || e.key === " ") {
        this._spaceDown = false;
        if (!this._panning) {
          this._applyCursor();
        }
      }
    });
    won("blur", () => {
      this._spaceDown = false;
      this._panning = false;
      this._panPointerId = null;
      this._panLast = null;
      this._drawPointerId = null;
      this.toolManager.cancel();
      this._applyCursor();
    });
    const onVis = () => {
      if (document.visibilityState === "hidden") {
        this._spaceDown = false;
        this._panning = false;
        this._panPointerId = null;
        this._panLast = null;
        this._drawPointerId = null;
        this.toolManager.cancel();
        if (!this._inside) this._clearHover();
        this._applyCursor();
      }
    };
    try {
      document.addEventListener("visibilitychange", onVis);
      this._bound._vis = onVis;
    } catch {
      // ignore (node)
    }
    this._onSettings = (e) => {
      const { key } = e.detail ?? {};
      if (key !== "activeTool") return;
      this._applyCursor();
    };
    try {
      this.session?.addEventListener(EVENTS.SETTINGS_CHANGED, this._onSettings);
    } catch {
      // ignore (no session bus)
    }
    this._applyCursor();
  }

  dispose() {
    if (!this._attached) return;
    this._attached = false;
    const h = this.host;
    for (const [type, fn] of Object.entries(this._bound)) {
      if (type === "_win" || type === "_vis") continue;
      try {
        h.removeEventListener(type, fn);
      } catch {
        // ignore
      }
    }
    const win = this._bound._win ?? {};
    for (const [type, fn] of Object.entries(win)) {
      try {
        window.removeEventListener(type, fn);
      } catch {
        // ignore
      }
    }
    if (this._bound._vis) {
      try {
        document.removeEventListener("visibilitychange", this._bound._vis);
      } catch {
        // ignore
      }
    }
    if (this._onSettings) {
      try {
        this.session?.removeEventListener(EVENTS.SETTINGS_CHANGED, this._onSettings);
      } catch {
        // ignore
      }
      this._onSettings = null;
    }
    this._bound = {};
  }
}
