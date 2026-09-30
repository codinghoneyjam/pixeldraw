// CanvasRenderer — browser-only display renderer (DOM allowed here).
// Contract: docs/render.md (renderer.js). view.js/composite.js stay DOM-free.
// Grid lives in grid.js; background parsing reuses core/pixel.js.

import { visibleChunkRange } from "./view.js";
import { compositeChunk } from "./composite.js";
import { gridLines, paintGrid } from "./grid.js";
import { paintBackground } from "./background.js";
import { CHUNK_PX, chunkKey, chunkCoords } from "../core/constants.js";
import { EVENTS } from "../core/events.js";
import { DrawToolError } from "../core/errors.js";

const FRAME_BUDGET_MS = 6;
const BORDER_STYLE = "rgba(0,0,0,0.6)";

const SUBS = [
  [EVENTS.DOCUMENT_REPLACED, "_onDocReplaced"],
  [EVENTS.PIXELS_CHANGED, "_onPixels"],
  [EVENTS.LAYERS_CHANGED, "_onLayers"],
  [EVENTS.SETTINGS_CHANGED, "_onSettings"],
];

function nowMs() {
  if (typeof performance !== "undefined" && typeof performance.now === "function") return performance.now();
  return Date.now();
}

export class CanvasRenderer {
  constructor({ host, session, getView, getOverlay } = {}) {
    if (!host || typeof host.appendChild !== "function" || typeof getView !== "function") {
      throw new DrawToolError("INVALID_STATE", "CanvasRenderer needs { host, session, getView }");
    }
    if (!session || typeof session.addEventListener !== "function") {
      throw new DrawToolError("INVALID_STATE", "CanvasRenderer needs an EventTarget session");
    }
    this._host = host;
    this._session = session;
    this._getView = getView;
    this._getOverlay = typeof getOverlay === "function" ? getOverlay : () => null;
    this._display = null;
    this._dctx = null;
    this._comp = null;
    this._cctx = null;
    this._img = null;
    this._compW = 0;
    this._compH = 0;
    this._doc = null;
    this._dirty = new Set();
    this._dirtyAll = false;
    this._cancelFrame = null;
    this._disposed = false;
    this._ro = null;
    this._cssW = 0;
    this._cssH = 0;
    this._dpr = 1;
    this._onDocReplaced = (e) => {
      const d = e && e.detail ? e.detail.document : undefined;
      this._setDocument(d !== undefined ? d : this._sessionDoc());
    };
    this._onPixels = (e) => {
      const d = e && e.detail ? e.detail : {};
      if (d.all === true) this.invalidateAll();
      else if (Array.isArray(d.chunks)) this.invalidateChunks(d.chunks);
      this.requestRender();
    };
    this._onLayers = (e) => {
      const d = e && e.detail ? e.detail : {};
      if (d.reason === "active") return;
      if (d.reason === "prop" && d.field === "name") return;
      this.invalidateAll();
      this.requestRender();
    };
    this._onSettings = (e) => {
      const d = e && e.detail ? e.detail : {};
      if (d.key === "gridMode") this.requestRender();
    };
    this._onFrame = () => {
      this._cancelFrame = null;
      this._update();
    };
  }

  attach() {
    if (this._disposed) throw new DrawToolError("INVALID_STATE", "renderer is disposed");
    if (this._display) return this;
    if (typeof document === "undefined") throw new DrawToolError("INVALID_STATE", "attach() needs a browser DOM");
    const c = document.createElement("canvas");
    c.className = "dt-display";
    c.style.position = "absolute";
    c.style.inset = "0";
    c.style.display = "block";
    if (this._host.style && typeof getComputedStyle === "function"
      && getComputedStyle(this._host).position === "static") this._host.style.position = "relative";
    this._host.appendChild(c);
    this._display = c;
    this._dctx = c.getContext("2d");
    this._setDocument(this._sessionDoc());
    for (const [type, name] of SUBS) this._session.addEventListener(type, this[name]);
    if (typeof ResizeObserver !== "undefined") {
      this._ro = new ResizeObserver(() => this.resize());
      this._ro.observe(this._host);
    }
    this.resize();
    this.invalidateAll();
    this.requestRender();
    return this;
  }

  dispose() {
    if (this._disposed) return;
    this._disposed = true;
    if (this._cancelFrame) {
      this._cancelFrame();
      this._cancelFrame = null;
    }
    for (const [type, name] of SUBS) this._session.removeEventListener(type, this[name]);
    if (this._ro) {
      this._ro.disconnect();
      this._ro = null;
    }
    if (this._display && this._display.parentNode) this._display.parentNode.removeChild(this._display);
    this._display = null;
    this._dctx = null;
    this._comp = null;
    this._cctx = null;
    this._img = null;
    this._dirty.clear();
  }

  resize() {
    let w = this._host.clientWidth || 0;
    let h = this._host.clientHeight || 0;
    if ((!w || !h) && typeof this._host.getBoundingClientRect === "function") {
      const r = this._host.getBoundingClientRect();
      w = Math.round(r.width);
      h = Math.round(r.height);
    }
    if (w <= 0 || h <= 0) return;
    this._cssW = w;
    this._cssH = h;
    this._dpr = typeof window !== "undefined" && window.devicePixelRatio ? window.devicePixelRatio : 1;
    if (this._display) {
      this._display.width = Math.max(1, Math.round(w * this._dpr));
      this._display.height = Math.max(1, Math.round(h * this._dpr));
      this._display.style.width = `${w}px`;
      this._display.style.height = `${h}px`;
    }
    this.requestRender();
  }

  requestRender() {
    if (this._disposed || this._cancelFrame) return;
    if (typeof requestAnimationFrame !== "undefined") {
      const id = requestAnimationFrame(this._onFrame);
      this._cancelFrame = () => cancelAnimationFrame(id);
    } else {
      const id = setTimeout(this._onFrame, 16);
      this._cancelFrame = () => clearTimeout(id);
    }
  }

  invalidateAll() {
    this._dirtyAll = true;
  }

  invalidateChunks(chunks) {
    for (const c of chunks) {
      if (!c || !Number.isInteger(c.cx) || !Number.isInteger(c.cy) || c.cx < 0 || c.cy < 0) continue;
      this._dirty.add(chunkKey(c.cx, c.cy));
    }
  }

  pixelToDevice(x, y) {
    const v = this._getView();
    const d = this._dpr || 1;
    return { x: (v.offsetX + x * v.zoom) * d, y: (v.offsetY + y * v.zoom) * d };
  }

  _sessionDoc() {
    try {
      const s = this._session;
      if (!s) return null;
      if (s.doc !== undefined) return s.doc;
      return s.document !== undefined ? s.document : null;
    } catch {
      return null;
    }
  }

  _gridMode() {
    try {
      const s = this._session.settings;
      if (s && typeof s.gridMode === "string") return s.gridMode;
    } catch {
      // fall through
    }
    return "off";
  }

  _setDocument(doc) {
    this._doc = doc || null;
    this._ensureComp();
    this.invalidateAll();
    this.requestRender();
  }

  _ensureComp() {
    const doc = this._doc;
    if (!doc) return;
    const w = doc.canvas.widthPx;
    const h = doc.canvas.heightPx;
    if (this._comp && this._compW === w && this._compH === h) return;
    let c = null;
    let ctx = null;
    if (typeof OffscreenCanvas !== "undefined") {
      c = new OffscreenCanvas(w, h);
      ctx = c.getContext("2d");
    }
    if (!ctx && typeof document !== "undefined") {
      c = document.createElement("canvas");
      c.width = w;
      c.height = h;
      ctx = c.getContext("2d");
    }
    if (!ctx) return;
    c.width = w;
    c.height = h;
    this._comp = c;
    this._cctx = ctx;
    this._compW = w;
    this._compH = h;
    this._img = ctx.createImageData(CHUNK_PX, CHUNK_PX);
    this._dirtyAll = true;
  }

  _update() {
    if (this._disposed || !this._dctx || !this._doc) return;
    const doc = this._doc;
    const w = doc.canvas.widthPx;
    const h = doc.canvas.heightPx;
    if (this._compW !== w || this._compH !== h) this._ensureComp();
    const view = this._getView();
    if (this._dirtyAll) {
      this._dirty.clear();
      this._dirtyAll = false;
      for (let cy = 0; cy < h / CHUNK_PX; cy++) {
        for (let cx = 0; cx < w / CHUNK_PX; cx++) this._dirty.add(chunkKey(cx, cy));
      }
    }
    if (this._dirty.size > 0 && this._cctx && this._img) this._paintDirty(doc, view, w, h);
    this._drawDisplay(doc, view, w, h);
    if (this._dirty.size > 0 || this._dirtyAll) this.requestRender();
  }

  _paintDirty(doc, view, w, h) {
    const vis = visibleChunkRange(view, w, h, this._cssW, this._cssH);
    const inVis = (cx, cy) => vis !== null
      && cx >= vis.cx0 && cx <= vis.cx1 && cy >= vis.cy0 && cy <= vis.cy1;
    const paint = (key) => {
      const { cx, cy } = chunkCoords(key);
      compositeChunk(doc, cx, cy, this._img.data);
      this._cctx.putImageData(this._img, cx * CHUNK_PX, cy * CHUNK_PX);
      this._dirty.delete(key);
    };
    const keys = [...this._dirty];
    for (const key of keys) {
      const { cx, cy } = chunkCoords(key);
      if (inVis(cx, cy)) paint(key);
    }
    const t0 = nowMs();
    for (const key of keys) {
      if (!this._dirty.has(key)) continue;
      if (nowMs() - t0 > FRAME_BUDGET_MS) break;
      paint(key);
    }
  }

  _drawDisplay(doc, view, w, h) {
    const ctx = this._dctx;
    const dpr = this._dpr || 1;
    const devW = Math.max(1, Math.round(this._cssW * dpr));
    const devH = Math.max(1, Math.round(this._cssH * dpr));
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalCompositeOperation = "source-over";
    ctx.clearRect(0, 0, devW, devH);
    const dx = Math.round(view.offsetX * dpr);
    const dy = Math.round(view.offsetY * dpr);
    const dw = Math.round(w * view.zoom * dpr);
    const dh = Math.round(h * view.zoom * dpr);
    ctx.save();
    ctx.beginPath();
    ctx.rect(dx, dy, dw, dh);
    ctx.clip();
    paintBackground(ctx, doc.canvas.background, dx, dy, dw, dh, dpr, { vw: devW, vh: devH });
    ctx.imageSmoothingEnabled = false;
    if (this._comp) this._blitVisible(ctx, view, w, h, dpr);
    ctx.restore();
    paintGrid(ctx, gridLines(view, w, h, this._cssW, this._cssH, this._gridMode()),
      view, dx, dy, dw, dh, devW, devH, dpr);
    if (dw > 0 && dh > 0) {
      ctx.fillStyle = BORDER_STYLE;
      ctx.fillRect(dx - 1, dy - 1, dw + 2, 1);
      ctx.fillRect(dx - 1, dy + dh, dw + 2, 1);
      ctx.fillRect(dx - 1, dy, 1, dh);
      ctx.fillRect(dx + dw, dy, 1, dh);
    }
    const overlay = this._getOverlay();
    if (overlay) overlay(ctx, { view, dpr, pixelToDevice: (x, y) => this.pixelToDevice(x, y) });
  }

  _blitVisible(ctx, view, w, h, dpr) {
    const x0 = Math.max(0, Math.floor((0 - view.offsetX) / view.zoom));
    const y0 = Math.max(0, Math.floor((0 - view.offsetY) / view.zoom));
    const sw = Math.min(w, Math.ceil((this._cssW - view.offsetX) / view.zoom)) - x0;
    const sh = Math.min(h, Math.ceil((this._cssH - view.offsetY) / view.zoom)) - y0;
    if (sw <= 0 || sh <= 0) return;
    const ddw = Math.round(sw * view.zoom * dpr);
    const ddh = Math.round(sh * view.zoom * dpr);
    if (ddw <= 0 || ddh <= 0) return;
    ctx.drawImage(
      this._comp, x0, y0, sw, sh,
      Math.round((view.offsetX + x0 * view.zoom) * dpr),
      Math.round((view.offsetY + y0 * view.zoom) * dpr),
      ddw, ddh,
    );
  }
}
