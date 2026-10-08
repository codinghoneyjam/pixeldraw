// <META - FILE SUMMARY - ShapeTool: pending state machine and pointer/key interaction>
// <META - SUMMARY CONT - Raster output lives in shape_render.js; placing +
// <META - SUMMARY CONT - pending accessors in shape_pending.js; keys in shape_keys.js>
import { DrawToolError } from "../../core/errors.js";
import { EVENTS } from "../../core/events.js";
import { dragGeom, hitHandle, insideShape, moveGeom, resizeGeom, snapPlacePoint } from "./shape_geom.js";
import { commitPending, paintPreview, renderToolOverlay } from "./shape_render.js";
import { placePoint, finishPlacing, readPending, writePending } from "./shape_pending.js";
import { handleKeyDown } from "./shape_keys.js";

// <META - ROLE : kind table, labels, live settings keys | L9-14>
const KINDS = ["line", "rect", "rrect", "ellipse", "polygon"];
const LABELS = { line: "직선", rect: "사각형", rrect: "둥근 사각형", ellipse: "타원", polygon: "다각형" };
// Colours are deliberately NOT live keys: a pending shape owns the colour it was drawn
// with (see shape_render.js buildColor for why).
const LIVE_KEYS = ["penSize", "shapeFill", "shapeRadius", "snapUnit", "shapeLockAspect"];

// <META - ROLE : integer field guard | L17-20>
function asInt(v, what) {
  if (!Number.isInteger(v)) throw new DrawToolError("OUT_OF_RANGE", `invalid ${what}`);
  return v;
}

export class ShapeTool {
  // <META - ROLE : construct per-kind tool and subscribe session events | L24-63>
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
    this._place = null;
    this._hoverPt = null;
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

  // <META - ROLE : Tool identity, lifecycle, pending primitives | L66-106>
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
    if (this._mode === "placing" && this._place) {
      // Same rule as pending: leaving the tool keeps finished work.
      if (this._place.points.length >= 3) this._finishPlacing();
      else { this._place = null; this._hoverPt = null; this._mode = this._pending ? "pending" : "idle"; }
    }
    if (this.hasPending()) this.commit();
    this._draw = null;
    this._op = null;
    this._place = null;
    this._hoverPt = null;
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
    this._place = null;
    this._hoverPt = null;
    this._mode = "idle";
    this._emitChanged();
    this._render();
  }
  cancel() {
    if (this._mode === "placing") {
      // Placing never opened an edit, so there is nothing to roll back.
      this._place = null;
      this._hoverPt = null;
      this._mode = this._pending ? "pending" : "idle";
      this._emitChanged();
      this._render();
      return;
    }
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

  // <META - ROLE : view, render, emit, and pointer flag plumbing | L109-134>
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

  // <META - ROLE : pointer down: handles, move, commit-plus-new, or polygon placing | L137-178>
  pointerDown(ev) {
    if (ev.button !== 0 && ev.button !== undefined) return;
    if (!this._session.doc || this._mode !== "idle" && this._mode !== "pending" && this._mode !== "placing") return;
    const p = { x: asInt(ev.x, "x"), y: asInt(ev.y, "y") };
    if (this._mode === "placing" && this._place) {
      this._placePoint(ev, p);
      return;
    }
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
    if (this._kind === "polygon") {
      this._place = { points: [snapPlacePoint(p, this._session.settings.snapUnit)], layerId: this._session.doc.activeLayerId };
      this._hoverPt = null;
      this._mode = "placing";
      this._emitChanged();
      this._render();
      return;
    }
    this._draw = { p0: p, layerId: this._session.doc.activeLayerId, shift: !!ev.shift, alt: !!ev.alt };
    this._mode = "drawing";
    this._render();
  }

  // <META - ROLE : polygon placing delegates (flow in shape_pending.js) | L180-198>
  _placePoint(ev, p) {
    placePoint(this, ev, p);
  }

  // <META - ROLE : placing with >= 3 vertices becomes pending | L200-212>
  _finishPlacing() {
    return finishPlacing(this);
  }

  // <META - ROLE : pointer move: drawing, placing hover, resizing, or moving preview | L214-236>
  pointerMove(ev) {
    if (this._mode === "drawing" && this._draw) {
      this._draw.cur = { x: ev.x, y: ev.y };
      this._draw.shift = !!ev.shift;
      this._draw.alt = !!ev.alt;
      this._render();
    } else if (this._mode === "placing" && this._place) {
      this._hoverPt = { x: ev.x, y: ev.y };
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

  // <META - ROLE : pointer up: drawing becomes pending, ops settle | L185-214>
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
        // `color` is the draw-time foreground snapshot; see shape_render.js buildColor.
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

  // <META - ROLE : keyboard dispatch lives in shape_keys.js | L238-278>
  keyDown(ev) {
    return handleKeyDown(this, ev);
  }

  // <META - ROLE : numeric pending accessors for the option bar | L249-273>
  getPending() {
    return readPending(this);
  }
  setPending(v) {
    writePending(this, v);
  }

  // <META - ROLE : commit + preview + overlay entry points (raster in shape_render.js) | L272-284>
  _label() {
    return LABELS[this._kind];
  }
  commit() {
    return commitPending(this);
  }
  paintPreview(writer) {
    return paintPreview(this, writer);
  }
  overlay(ctx, info = {}) {
    return renderToolOverlay(this, ctx, info);
  }
}
