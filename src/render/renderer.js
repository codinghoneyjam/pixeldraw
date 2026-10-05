// CanvasRenderer — browser-only display renderer (DOM allowed here).
// Contract: docs/render.md (renderer.js). view.js/composite.js stay DOM-free.
// Grid lives in grid.js; background parsing reuses core/pixel.js.

import { CHUNK_PX, chunkKey } from "../core/constants.js";
import { EVENTS } from "../core/events.js";
import { DrawToolError } from "../core/errors.js";
import { ensureComp, paintDirty } from "./renderer_composite.js";
import { drawDisplay } from "./renderer_display.js";

const SUBS = [
  [EVENTS.DOCUMENT_REPLACED, "_onDocReplaced"],
  [EVENTS.PIXELS_CHANGED, "_onPixels"],
  [EVENTS.LAYERS_CHANGED, "_onLayers"],
  [EVENTS.SETTINGS_CHANGED, "_onSettings"],
];

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
      const newW = Math.max(1, Math.round(w * this._dpr));
      const newH = Math.max(1, Math.round(h * this._dpr));
      // Canvas width/height assignment clears the bitmap even when the value is
      // identical, so guard it — this prevents a ResizeObserver-triggered resize
      // (caused by option-bar section visibility changes on tool switch) from
      // blanking the canvas for one frame and producing a flicker.
      if (this._display.width !== newW || this._display.height !== newH) {
        this._display.width = newW;
        this._display.height = newH;
        this.invalidateAll();
      }
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
      // no settings bus (node / pre-attach): fall through to the default
    }
    return "off";
  }

  _setDocument(doc) {
    this._doc = doc || null;
    ensureComp(this);
    this.invalidateAll();
    this.requestRender();
  }

  _update() {
    if (this._disposed || !this._dctx || !this._doc) return;
    const doc = this._doc;
    const w = doc.canvas.widthPx;
    const h = doc.canvas.heightPx;
    if (this._compW !== w || this._compH !== h) ensureComp(this);
    const view = this._getView();
    if (this._dirtyAll) {
      this._dirty.clear();
      this._dirtyAll = false;
      for (let cy = 0; cy < h / CHUNK_PX; cy++) {
        for (let cx = 0; cx < w / CHUNK_PX; cx++) this._dirty.add(chunkKey(cx, cy));
      }
    }
    if (this._dirty.size > 0 && this._cctx && this._img) paintDirty(this, doc, view, w, h);
    drawDisplay(this, doc, view, w, h);
    if (this._dirty.size > 0 || this._dirtyAll) this.requestRender();
  }
}
