// <META - FILE SUMMARY - ShapeTool: pending state machine, commit, overlay glue>
import { applyShape } from "../core/raster/shape_raster.js";
import { DrawToolError } from "../core/errors.js";
import { EVENTS } from "../core/events.js";
import { dragGeom, buildSpec, drawSpec, geomFromSpec, hitHandle, insideShape, layoutHandles, moveGeom, parseBoxGeom, parseLineGeom, resizeGeom } from "./shape_geom.js";
import { computePaintLayers, packOf, renderOverlay } from "./shape_overlay.js";

// <META - ROLE : kind table, labels, live settings keys | L8-11>
const KINDS = ["line", "rect", "rrect", "ellipse"];
const LABELS = { line: "직선", rect: "사각형", rrect: "둥근 사각형", ellipse: "타원" };
// Colours are deliberately NOT live keys. A pending shape owns the colour it was drawn
// with, so changing the foreground afterwards must not repaint it (that was the
// "직전 객체 색이 연동되어 변한다" bug).
const LIVE_KEYS = ["penSize", "shapeFill", "shapeRadius", "snapUnit", "shapeLockAspect"];

// <META - ROLE : integer field guard | L13-17>
function asInt(v, what) {
  if (!Number.isInteger(v)) throw new DrawToolError("OUT_OF_RANGE", `invalid ${what}`);
  return v;
}

export class ShapeTool {
  // <META - ROLE : construct per-kind tool and subscribe session events | L20-55>
  constructor(env, kind = "rect") {
    if (typeof env === "string") {
      const swap = kind;
      kind = env;
      env = swap;
    }
    if (!KINDS.includes(kind)) throw new DrawToolError("OUT_OF_RANGE", `unknown shape kind ${String(kind)}`);
    this._env = env ?? {};
    this._session = this._env.session;
    this._kind = kind;
    this._mode = "idle";
    this._pending = null;
    this._draw = null;
    this._op = null;
    this._emitted = undefined;
    this._masks = { key: null, layers: null };
    this._tmp = {};
    this._session.addEventListener(EVENTS.SETTINGS_CHANGED, (e) => this._settingsChanged(e));
    this._session.addEventListener(EVENTS.LAYERS_CHANGED, (e) => this._layersChanged(e));
    this._session.addEventListener(EVENTS.DOCUMENT_REPLACED, () => this.discardPending());
  }
  _settingsChanged(e) {
    if (!LIVE_KEYS.includes(e.detail.key)) return;
    if (this._kind === "rrect" && this._pending && e.detail.key === "shapeRadius") this._pending.radius = e.detail.value;
    if (this._pending || this._draw) {
      this._emitChanged();
      this._render();
    }
  }
  _layersChanged(e) {
    if (!this.hasPending()) return;
    if (e.detail.reason === "active") this.commit();
    else if (e.detail.reason === "remove" || e.detail.reason === "replace") {
      try {
        this._session.doc.getLayer(this._pending.layerId);
      } catch {
        this.discardPending();
      }
    }
  }

  // <META - ROLE : Tool identity, lifecycle, pending primitives | L57-97>
  get id() {
    return this._kind;
  }
  get cursor() {
    return "crosshair";
  }
  activate() {
    this._render();
  }
  deactivate() {
    if (this.hasPending()) this.commit();
    this._draw = null;
    this._op = null;
    this._mode = "idle";
  }
  hover() {}
  hasPending() {
    return this._pending !== null;
  }
  discardPending() {
    this._pending = null;
    this._draw = null;
    this._op = null;
    this._mode = "idle";
    this._emitChanged();
    this._render();
  }
  cancel() {
    if (this._mode === "drawing") {
      this._draw = null;
      this._mode = this._pending ? "pending" : "idle";
      this._emitChanged();
      this._render();
    } else if (this._mode === "resizing" || this._mode === "moving") {
      this._pending = this._op.orig;
      this._op = null;
      this._mode = "pending";
      this._emitChanged();
      this._render();
    }
  }

  // <META - ROLE : view, render, emit, and pointer flag plumbing | L99-130>
  _view() {
    try {
      return this._env.getView ? this._env.getView() : null;
    } catch {
      return null;
    }
  }
  _zoom() {
    const v = this._view();
    return v && Number.isFinite(v.zoom) && v.zoom > 0 ? v.zoom : 1;
  }
  _render() {
    try {
      this._env.requestRender && this._env.requestRender();
    } catch {}
  }
  _emitChanged() {
    const cur = JSON.stringify(this.getPending());
    if (cur === this._emitted) return;
    this._emitted = cur;
    this._session.emitToolState(this._kind, this.getPending());
  }
  _flags(ev) {
    const s = this._session.settings;
    return { shift: !!ev.shift, lock: !!ev.shift || s.shapeLockAspect, center: !!ev.alt, snap: s.snapUnit };
  }

  // <META - ROLE : pointer down: handles, move, or commit-plus-new | L132-158>
  pointerDown(ev) {
    if (ev.button !== 0 && ev.button !== undefined) return;
    if (!this._session.doc || this._mode !== "idle" && this._mode !== "pending") return;
    const p = { x: asInt(ev.x, "x"), y: asInt(ev.y, "y") };
    if (this._mode === "pending" && this._pending) {
      const h = hitHandle(this._kind, this._pending, ev.fx ?? p.x, ev.fy ?? p.y, 6 / this._zoom());
      if (h) {
        this._op = { type: "resize", handle: h.id, end: h.end, orig: structuredClone(this._pending) };
        this._mode = "resizing";
        this._resizeTo(ev);
        return;
      }
      const grab = Math.max(this._session.settings.penSize / 2, 4 / this._zoom());
      if (insideShape(this._kind, this._pending, ev.fx ?? p.x, ev.fy ?? p.y, grab)) {
        this._op = { type: "move", ox: p.x, oy: p.y, orig: structuredClone(this._pending) };
        this._mode = "moving";
        return;
      }
      this.commit();
    }
    this._draw = { p0: p, layerId: this._session.doc.activeLayerId, shift: !!ev.shift, alt: !!ev.alt };
    this._mode = "drawing";
    this._render();
  }

  // <META - ROLE : pointer move: drawing, resizing, or moving preview | L160-175>
  pointerMove(ev) {
    if (this._mode === "drawing" && this._draw) {
      this._draw.cur = { x: ev.x, y: ev.y };
      this._draw.shift = !!ev.shift;
      this._draw.alt = !!ev.alt;
      this._render();
    } else if (this._mode === "resizing" && this._op) {
      this._resizeTo(ev);
    } else if (this._mode === "moving" && this._op) {
      this._pending = moveGeom(this._kind, this._op.orig, ev.x - this._op.ox, ev.y - this._op.oy, this._session.settings.snapUnit);
      this._emitChanged();
      this._render();
    }
  }
  _resizeTo(ev) {
    const p = { x: asInt(ev.x, "x"), y: asInt(ev.y, "y") };
    this._pending = resizeGeom(this._kind, this._op.orig, this._op.handle, this._op.end, p, this._flags(ev), !!ev.shift);
    this._emitChanged();
    this._render();
  }

  // <META - ROLE : pointer up: drawing becomes pending, ops settle | L177-197>
  pointerUp(ev) {
    if (this._mode === "drawing" && this._draw) {
      const p0 = this._draw.p0;
      const p = { x: asInt(ev.x, "x"), y: asInt(ev.y, "y") };
      const idle = this._pending ? "pending" : "idle";
      if (p.x === p0.x && p.y === p0.y) {
        this._draw = null;
        this._mode = idle;
      } else {
        const g = dragGeom(this._kind, p0, p, this._flags(ev), this._session.settings.shapeRadius);
        // Snapshot the foreground colour the shape is being DRAWN with. The pending
        // shape outlives pointer-up so it stays resizable, and without this snapshot a
        // later colour change would recolour this already-made object.
        this._pending = {
          layerId: this._draw.layerId,
          color: this._session.settings.primaryColor,
          ...g,
        };
        this._draw = null;
        this._mode = "pending";
      }
      this._emitChanged();
      this._render();
    } else if ((this._mode === "resizing" || this._mode === "moving") && this._op) {
      this._op = null;
      this._mode = "pending";
      this._emitChanged();
      this._render();
    }
  }

  // <META - ROLE : keyboard commit, discard, and arrow nudge | L199-235>
  keyDown(ev) {
    if (ev.key === "Enter") {
      if (this._mode === "drawing" || !this.hasPending()) return false;
      this._op = null;
      this._mode = "pending";
      this.commit();
      return true;
    }
    if (ev.key === "Escape" || ev.key === "Delete" || ev.key === "Backspace") {
      if ((this._mode === "drawing" || this._mode === "resizing" || this._mode === "moving") && ev.key === "Escape") {
        this.cancel();
        return true;
      }
      if (this.hasPending() || this._draw || this._op) {
        this.discardPending();
        return true;
      }
      return false;
    }
    const step = ev.shift ? 32 : 1;
    const d = ev.key === "ArrowLeft" ? [-step, 0] : ev.key === "ArrowRight" ? [step, 0] : ev.key === "ArrowUp" ? [0, -step] : ev.key === "ArrowDown" ? [0, step] : null;
    if (!d || !this.hasPending() || this._mode !== "pending") return false;
    const p = this._pending;
    this._pending = this._kind === "line"
      ? { ...p, p0: { x: p.p0.x + d[0], y: p.p0.y + d[1] }, p1: { x: p.p1.x + d[0], y: p.p1.y + d[1] } }
      : { ...p, bbox: { ...p.bbox, x: p.bbox.x + d[0], y: p.bbox.y + d[1] } };
    this._emitChanged();
    this._render();
    return true;
  }

  // <META - ROLE : numeric pending accessors for the option bar | L237-275>
  getPending() {
    if (!this._pending) return null;
    const p = this._pending;
    if (this._kind === "line") return { x0: p.p0.x, y0: p.p0.y, x1: p.p1.x, y1: p.p1.y };
    return { x: p.bbox.x, y: p.bbox.y, w: p.bbox.w, h: p.bbox.h, radius: p.radius };
  }
  setPending(v) {
    if (v === null || v === undefined) {
      this.discardPending();
      return;
    }
    if (!this._session.doc) throw new DrawToolError("INVALID_STATE", "no document loaded");
    const layerId = this._pending ? this._pending.layerId : this._session.doc.activeLayerId;
    // Creating a pending shape snapshots the foreground; updating an EXISTING pending
    // shape keeps the colour it already had, so nudging the bbox in the option bar
    // after a colour change still cannot recolour it.
    const color = (this._pending && this._pending.color) || this._session.settings.primaryColor;
    const geom = this._kind === "line" ? parseLineGeom(v) : parseBoxGeom(v, this._session.settings.shapeRadius);
    this._pending = { layerId, color, ...geom };
    this._draw = null;
    this._op = null;
    this._mode = "pending";
    this._emitChanged();
    this._render();
  }

  // <META - ROLE : commit-time spec and history write | L277-315>
  // A shape captures its colour WHEN IT IS DRAWN, not when it is applied. The pending
  // shape deliberately survives pointer-up so it can still be resized, and every later
  // apply (확정 button, resize-drag release, numeric bbox edit) used to read
  // `settings.primaryColor` fresh — so changing colour after drawing recoloured the
  // object just made. `pending.color` is the snapshot that prevents that.
  _buildColor() {
    const snap = this._pending && this._pending.color;
    return snap || this._session.settings.primaryColor;
  }
  _buildSpec() {
    return buildSpec(this._kind, this._pending, this._session.settings);
  }
  commit() {
    if (!this._pending) return false;
    const layerId = this._pending.layerId;
    const label = LABELS[this._kind];
    let edit;
    try {
      edit = this._session.beginEdit({ layerId, label });
    } catch (err) {
      if (err && (err.code === "LAYER_LOCKED" || err.code === "LAYER_HIDDEN" || err.code === "LAYER_NOT_FOUND")) {
        this.discardPending();
        return false;
      }
      throw err;
    }
    // Only the FOREGROUND colour may paint. The background slot is a buffer only, so
    // it is passed as the same colour rather than being allowed to write pixels.
    const color = this._buildColor();
    applyShape(edit.writer, this._buildSpec(), packOf(color), packOf(color));
    edit.commit(label);
    this._pending = null;
    this._op = null;
    this._mode = "idle";
    this._emitChanged();
    this._render();
    return true;
  }

  // <META - ROLE : preview spec, paint layers, test hook, and overlay | L312-350>
  _previewSpec() {
    if (this._mode === "drawing" && this._draw) return drawSpec(this._kind, this._draw, this._session.settings);
    return this._pending ? this._buildSpec() : null;
  }
  _paintLayers(spec) {
    // Cache key must include the captured colour, otherwise a colour change would not
    // invalidate the cached layers and the preview would keep the old colour.
    const key = JSON.stringify(spec) + "|" + this._buildColor();
    if (this._masks.key !== key) {
      this._masks = { key, layers: computePaintLayers(spec, this._buildColor()) };
    }
    return this._masks.layers;
  }
  paintPreview(writer) {
    const spec = this._previewSpec();
    if (!spec) return 0;
    const color = this._buildColor();
    return applyShape(writer, spec, packOf(color), packOf(color));
  }
  overlay(ctx, info = {}) {
    const spec = this._previewSpec();
    if (!spec || !ctx) return;
    const view = info.view ?? this._view() ?? { zoom: 1, offsetX: 0, offsetY: 0 };
    const dpr = info.dpr ?? 1;
    const box = spec.kind === "line"
      ? { x: Math.min(spec.p0.x, spec.p1.x), y: Math.min(spec.p0.y, spec.p1.y), w: Math.abs(spec.p1.x - spec.p0.x) + 1, h: Math.abs(spec.p1.y - spec.p0.y) + 1 }
      : { ...spec.bbox };
    renderOverlay(ctx, spec, this._paintLayers(spec), box, layoutHandles(this._kind, geomFromSpec(spec)), view, dpr, this._tmp);
  }
}
