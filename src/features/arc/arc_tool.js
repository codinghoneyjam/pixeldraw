import { arcMask, chordFillMask, chordOutlineMask } from "../../core/raster/arc.js";
import { packRGBA, parseHex } from "../../core/pixel.js";
import { Tool } from "../../tools/tool_base.js";

export class ArcTool extends Tool {
  constructor(env, kind = "arc") {
    super(env);
    this.kind = kind;
    this.start = 0;
    this.end = 180;
    this.width = 2;
    this.fill = true;
    this.outline = true;
    this._draw = null;
  }

  get id() { return this.kind; }
  get cursor() { return "crosshair"; }

  pointerDown(ev) {
    if ((ev.button !== 0 && ev.button !== undefined) || !this.session.doc) return;
    this._draw = {
      x0: Math.round(ev.x),
      y0: Math.round(ev.y),
      layerId: this.session.doc.activeLayerId,
      fillColor: this.session.settings.primaryColor,
      outlineColor: this.session.settings.secondaryColor,
    };
  }

  pointerUp(ev) {
    if (!this._draw) return;
    const draw = this._draw;
    this._draw = null;
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
      masks.push({ mask: arcMask(cx, cy, rx, ry, this.start, this.end, this.width, width, height), color: fillPacked });
    } else {
      if (this.fill) masks.push({ mask: chordFillMask(cx, cy, rx, ry, this.start, this.end, width, height), color: fillPacked });
      if (this.outline) masks.push({ mask: chordOutlineMask(cx, cy, rx, ry, this.start, this.end, this.width, width, height), color: outlinePacked });
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

  cancel() { this._draw = null; }
  deactivate() { this.cancel(); }
}