import { brushFootprint, strokePoint, strokeSegment } from "../core/brush.js";
import { EVENTS } from "../core/events.js";
import { packRGBA, parseHex, toHex } from "../core/pixel.js";
import { samplePixel } from "../render/composite.js";
import { Tool } from "./tool_base.js";

const PEN_CURSOR =
  "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24'%3E%3Ccircle cx='12' cy='12' r='7' fill='none' stroke='white' stroke-width='3'/%3E%3Ccircle cx='12' cy='12' r='7' fill='none' stroke='black' stroke-width='1.5'/%3E%3Ccircle cx='12' cy='12' r='1.6' fill='black' stroke='white' stroke-width='0.6'/%3E%3C/svg%3E\") 12 12, crosshair";
const ERASER_CURSOR =
  "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24'%3E%3Crect x='6' y='6' width='12' height='12' fill='rgba(255,255,255,0.25)' stroke='white' stroke-width='3'/%3E%3Crect x='6' y='6' width='12' height='12' fill='none' stroke='black' stroke-width='1.5'/%3E%3C/svg%3E\") 12 12, cell";

export class PenTool extends Tool {
  constructor(env = {}, { mode = "draw" } = {}) {
    super(env);
    if (mode !== "draw" && mode !== "erase") throw new Error(`unknown pen mode ${mode}`);
    this.mode = mode;
    this._edit = null;
    this._drawing = false;
    this._ignoreStroke = false;
    this._size = 1;
    this._packed = 0;
    this._lastPoint = null;
    this._hover = null;
    this._onDocReplaced = () => {
      this._lastPoint = null;
    };
    this._onLayers = (e) => {
      if ((e.detail ?? {}).reason === "active") this._lastPoint = null;
    };
    if (this.session && typeof this.session.addEventListener === "function") {
      this.session.addEventListener(EVENTS.DOCUMENT_REPLACED, this._onDocReplaced);
      this.session.addEventListener(EVENTS.LAYERS_CHANGED, this._onLayers);
    }
  }

  get id() {
    return this.mode === "draw" ? "pen" : "eraser";
  }

  get cursor() {
    return this.mode === "draw" ? PEN_CURSOR : ERASER_CURSOR;
  }

  get label() {
    return this.mode === "draw" ? "연필" : "지우개";
  }

  activate() {
    this._lastPoint = null;
  }

  deactivate() {
    if (this._edit) {
      try {
        this._edit.cancel();
      } catch {
        // ignore
      }
      this._edit = null;
    }
    this._drawing = false;
    this._ignoreStroke = false;
    this._lastPoint = null;
  }

  _pickColor() {
    if (this.mode === "erase") return 0;
    const hex = this.session.settings.primaryColor;
    const c = parseHex(hex);
    if (!c) return packRGBA(0, 0, 0, 255);
    return packRGBA(c.r, c.g, c.b, 255);
  }

  _eyedrop(ev) {
    const doc = this.session.doc;
    if (!doc) return;
    const rgba = samplePixel(doc, ev.x, ev.y);
    if (rgba[3] === 0) {
      this.session.notify("info", "투명 픽셀");
      return;
    }
    this.session.setSetting("primaryColor", toHex(rgba[0], rgba[1], rgba[2]));
  }

  pointerDown(ev) {
    if (!ev || ev.button !== 0) return;
    if (this._drawing) return;
    if (ev.alt === true) {
      this._eyedrop(ev);
      return;
    }
    let edit = null;
    try {
      edit = this.session.beginEdit({ label: this.label });
    } catch {
      this._ignoreStroke = true;
      this._drawing = false;
      this._edit = null;
      return;
    }
    this._edit = edit;
    this._drawing = true;
    this._ignoreStroke = false;
    this._size = this.session.settings.penSize;
    this._packed = this._pickColor();
    if (ev.shift === true && this._lastPoint) {
      strokeSegment(edit.writer, this._lastPoint.x, this._lastPoint.y, ev.x, ev.y, this._size, this._packed);
      this._lastPoint = { x: ev.x, y: ev.y };
      edit.flush();
      this._finishUp();
      return;
    }
    strokePoint(edit.writer, ev.x, ev.y, this._size, this._packed);
    this._lastPoint = { x: ev.x, y: ev.y };
    this._hover = { x: ev.x, y: ev.y };
    edit.flush();
  }

  pointerMove(ev) {
    if (this._ignoreStroke || !this._drawing || !this._edit) return;
    const pts = ev && Array.isArray(ev.coalesced) && ev.coalesced.length > 0 ? ev.coalesced : [{ x: ev.x, y: ev.y }];
    for (const pt of pts) {
      strokeSegment(this._edit.writer, this._lastPoint.x, this._lastPoint.y, pt.x, pt.y, this._size, this._packed);
      this._lastPoint = { x: pt.x, y: pt.y };
    }
    this._hover = { x: this._lastPoint.x, y: this._lastPoint.y };
    this._edit.flush();
  }

  pointerUp(ev) {
    if (this._ignoreStroke) {
      this._ignoreStroke = false;
      this._drawing = false;
      this._edit = null;
      return;
    }
    if (!this._drawing || !this._edit) return;
    if (ev && Number.isInteger(ev.x) && Number.isInteger(ev.y) && this._lastPoint) {
      if (ev.x !== this._lastPoint.x || ev.y !== this._lastPoint.y) {
        strokeSegment(this._edit.writer, this._lastPoint.x, this._lastPoint.y, ev.x, ev.y, this._size, this._packed);
        this._lastPoint = { x: ev.x, y: ev.y };
      }
      this._hover = { x: ev.x, y: ev.y };
    }
    this._finishUp();
  }

  _finishUp() {
    const edit = this._edit;
    this._edit = null;
    this._drawing = false;
    this._ignoreStroke = false;
    if (!edit) return;
    try {
      edit.commit(this.label);
    } catch {
      // commit failure (e.g. stale edit) must not break input
    }
  }

  cancel() {
    if (this._edit) {
      try {
        this._edit.cancel();
      } catch {
        // ignore
      }
      this._edit = null;
    }
    this._drawing = false;
    this._ignoreStroke = false;
  }

  hover(ev) {
    this._hover = ev ? { x: ev.x, y: ev.y } : null;
    try {
      this.requestRender();
    } catch {
      // ignore
    }
  }

  overlay(ctx, info) {
    if (!ctx || !info) return;
    // Only fall back to the last committed point while a stroke is live. Falling back
    // unconditionally made the preview re-appear at a stale pixel after the pointer left
    // the canvas instead of disappearing, and lingered there until the next pointermove.
    const pos = this._drawing ? (this._hover ?? this._lastPoint) : this._hover;
    if (!pos) return;
    if (!Number.isInteger(pos.x) || !Number.isInteger(pos.y)) return;
    const view = info.view;
    const dpr = info.dpr ?? 1;
    const toDevice = info.pixelToDevice;
    if (!view) return;
    const n = this._drawing ? this._size : this._safePenSize();
    const zoom = view.zoom;
    if (zoom >= 2) {
      let fp = null;
      try {
        fp = brushFootprint(n);
      } catch {
        return;
      }
      const stepX = zoom * dpr;
      const stepY = zoom * dpr;
      try {
        ctx.save?.();
      } catch {
        // ignore
      }
      try {
        ctx.fillStyle = "rgba(255,255,255,0.35)";
        for (let by = 0; by < n; by++) {
          for (let bx = 0; bx < n; bx++) {
            if (fp.mask[by * n + bx] !== 1) continue;
            const cx = pos.x - fp.offset + bx;
            const cy = pos.y - fp.offset + by;
            const p = typeof toDevice === "function" ? toDevice(cx, cy) : { x: cx * zoom * dpr, y: cy * zoom * dpr };
            ctx.fillRect(p.x, p.y, Math.max(1, stepX), Math.max(1, stepY));
          }
        }
        ctx.globalCompositeOperation = "difference";
        if (this.mode === "erase" && typeof ctx.setLineDash === "function") ctx.setLineDash([4 * dpr, 2 * dpr]);
        ctx.strokeStyle = "rgba(255,255,255,1)";
        ctx.lineWidth = 1;
        const o = typeof toDevice === "function" ? toDevice(pos.x - fp.offset, pos.y - fp.offset) : { x: 0, y: 0 };
        const w = n * stepX;
        const h = n * stepY;
        if (typeof ctx.strokeRect === "function") ctx.strokeRect(o.x + 0.5, o.y + 0.5, Math.max(1, w), Math.max(1, h));
        if (typeof ctx.setLineDash === "function") ctx.setLineDash([]);
        ctx.globalCompositeOperation = "source-over";
      } finally {
        try {
          ctx.restore?.();
        } catch {
          // ignore
        }
      }
      return;
    }
    try {
      ctx.save?.();
    } catch {
      // ignore
    }
    try {
      ctx.globalCompositeOperation = "difference";
      ctx.strokeStyle = "rgba(255,255,255,1)";
      ctx.lineWidth = 1;
      const c = typeof toDevice === "function" ? toDevice(pos.x + 0.5, pos.y + 0.5) : null;
      if (c && typeof ctx.beginPath === "function") {
        const r = 5 * dpr;
        ctx.beginPath();
        ctx.moveTo(c.x - r, c.y);
        ctx.lineTo(c.x + r, c.y);
        ctx.moveTo(c.x, c.y - r);
        ctx.lineTo(c.x, c.y + r);
        ctx.stroke();
      }
      ctx.globalCompositeOperation = "source-over";
    } finally {
      try {
        ctx.restore?.();
      } catch {
        // ignore
      }
    }
  }

  _safePenSize() {
    try {
      const n = this.session.settings.penSize;
      return Number.isInteger(n) ? n : 1;
    } catch {
      return 1;
    }
  }
}
