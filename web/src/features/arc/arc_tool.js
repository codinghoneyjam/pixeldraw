import { arcMask, chordFillMask, chordOutlineMask } from "../../core/raster/arc.js";
import { packRGBA, parseHex } from "../../core/pixel.js";
import { Tool } from "../../tools/tool_base.js";
import { drawPixelPreview } from "../../tools/pixel_preview.js";

export class ArcTool extends Tool {
  constructor(env, kind = "arc") {
    super(env);
    this.kind = kind;
    this.start = 0;
    this.end = 180;
    this.width = 2;
    this.chordMode = "fill";
    this._draw = null;
    this._preview = { canvas: null };
  }

  get id() { return this.kind; }
  get cursor() { return "crosshair"; }
  get fill() { return this.chordMode === "fill"; }
  set fill(value) {
    if (value) this.chordMode = "fill";
    else if (this.chordMode === "fill") this.chordMode = "outline";
  }
  get outline() { return this.chordMode === "outline"; }
  set outline(value) {
    if (value) this.chordMode = "outline";
    else if (this.chordMode === "outline") this.chordMode = "fill";
  }

  pointerDown(ev) {
    if ((ev.button !== 0 && ev.button !== undefined) || !this.session.doc) return;
    this._draw = {
      x0: Math.round(ev.x),
      y0: Math.round(ev.y),
      layerId: this.session.doc.activeLayerId,
      fillColor: this.session.settings.primaryColor,
      outlineColor: this.session.settings.secondaryColor,
      start: this.start,
      end: this.end,
      width: this.width,
      chordMode: this.chordMode,
      x1: Math.round(ev.x),
      y1: Math.round(ev.y),
    };
    this.requestRender();
  }

  pointerMove(ev) {
    if (!this._draw) return;
    this._draw.x1 = Math.round(ev.x);
    this._draw.y1 = Math.round(ev.y);
    this.requestRender();
  }

  pointerUp(ev) {
    if (!this._draw) return;
    const draw = this._draw;
    this._draw = null;
    this.requestRender();
    const x1 = Math.round(ev.x);
    const y1 = Math.round(ev.y);
    if (x1 === draw.x0 || y1 === draw.y0) return;
    const { widthPx: width, heightPx: height } = this.session.doc.canvas;
    const cx = (draw.x0 + x1) / 2;
    const cy = (draw.y0 + y1) / 2;
    const rx = Math.abs(x1 - draw.x0) / 2;
    const ry = Math.abs(y1 - draw.y0) / 2;
    const fill = parseHex(draw.fillColor);
    const outline = parseHex(draw.outlineColor);
    const fillPacked = packRGBA(fill.r, fill.g, fill.b, fill.a);
    const outlinePacked = packRGBA(outline.r, outline.g, outline.b, outline.a);
    const masks = [];

    if (this.kind === "arc") {
      masks.push({ mask: arcMask(cx, cy, rx, ry, draw.start, draw.end, draw.width, width, height), color: fillPacked });
    } else {
      if (draw.chordMode === "fill") masks.push({ mask: chordFillMask(cx, cy, rx, ry, draw.start, draw.end, width, height), color: fillPacked });
      else masks.push({ mask: chordOutlineMask(cx, cy, rx, ry, draw.start, draw.end, draw.width, width, height), color: outlinePacked });
    }
    if (!masks.length) return;

    const edit = this.session.beginEdit({ layerId: draw.layerId, label: this.kind === "arc" ? "호" : "현" });
    for (const { mask, color } of masks) {
      for (let i = 0; i < mask.data.length; i++) {
        if (mask.data[i] === 1) edit.writer.set(i % width, Math.floor(i / width), color);
      }
    }
    edit.commit(this.kind === "arc" ? "호" : "현");
    this.env.requestRender?.();
  }

  overlay(ctx, info = {}) {
    const draw = this._draw;
    if (!draw || !this.session.doc) return;
    const { widthPx: width, heightPx: height } = this.session.doc.canvas;
    const cx = (draw.x0 + draw.x1) / 2;
    const cy = (draw.y0 + draw.y1) / 2;
    const rx = Math.abs(draw.x1 - draw.x0) / 2;
    const ry = Math.abs(draw.y1 - draw.y0) / 2;
    const masks = this.kind === "arc"
      ? [{ mask: arcMask(cx, cy, rx, ry, draw.start, draw.end, draw.width, width, height), color: draw.fillColor }]
      : draw.chordMode === "fill"
        ? [{ mask: chordFillMask(cx, cy, rx, ry, draw.start, draw.end, width, height), color: draw.fillColor }]
        : [{ mask: chordOutlineMask(cx, cy, rx, ry, draw.start, draw.end, draw.width, width, height), color: draw.outlineColor }];
    const view = info.view ?? this.getView?.() ?? { zoom: 1, offsetX: 0, offsetY: 0 };
    for (const entry of masks) {
      drawPixelPreview(ctx, entry.mask, entry.color, view, info.dpr ?? 1, this._preview);
    }
  }

  cancel() {
    this._draw = null;
    this.requestRender();
  }
  deactivate() { this.cancel(); }
}