import { radialGradientMask } from "../../core/raster/gradient.js";
import { packRGBA, parseHex } from "../../core/pixel.js";
import { Tool } from "../../tools/tool_base.js";
import { drawPixelPreview } from "../../tools/pixel_preview.js";

export class GradientTool extends Tool {
  constructor(env) {
    super(env);
    this.innerRadius = 0;
    this.edgeMode = "transparent";
    this._center = null;
    this._preview = { canvas: null };
  }

  get id() { return "radial_gradient"; }
  get cursor() { return "crosshair"; }

  pointerDown(ev) {
    if ((ev.button !== 0 && ev.button !== undefined) || !this.session.doc) return;
    this._center = {
      x: Math.round(ev.x),
      y: Math.round(ev.y),
      layerId: this.session.doc.activeLayerId,
      primary: this.session.settings.primaryColor,
      secondary: this.session.settings.secondaryColor,
      innerRadius: this.innerRadius,
      edgeMode: this.edgeMode,
      x1: Math.round(ev.x),
      y1: Math.round(ev.y),
    };
    this.requestRender();
  }

  pointerMove(ev) {
    if (!this._center) return;
    this._center.x1 = Math.round(ev.x);
    this._center.y1 = Math.round(ev.y);
    this.requestRender();
  }

  pointerUp(ev) {
    if (!this._center) return;
    const center = this._center;
    this._center = null;
    this.requestRender();
    const radius = Math.hypot(Math.round(ev.x) - center.x, Math.round(ev.y) - center.y);
    if (radius < 1) return;
    const { widthPx: width, heightPx: height } = this.session.doc.canvas;
    const inner = parseHex(center.primary);
    const outer = parseHex(center.secondary);
    const mask = radialGradientMask({
      cx: center.x,
      cy: center.y,
      r0: Math.min(center.innerRadius, radius),
      r1: radius,
      stops: [
        [0, inner.r, inner.g, inner.b, inner.a],
        [1, outer.r, outer.g, outer.b, center.edgeMode === "solid" ? outer.a : 0],
      ],
      w: width,
      h: height,
    });
    const edit = this.session.beginEdit({ layerId: center.layerId, label: "방사형 그라디언트" });
    for (let i = 0; i < mask.data.length; i++) {
      if (mask.data[i] !== 0) edit.writer.set(i % width, Math.floor(i / width), mask.data[i]);
    }
    edit.commit("방사형 그라디언트");
    this.env.requestRender?.();
  }

  overlay(ctx, info = {}) {
    const center = this._center;
    if (!center || !this.session.doc) return;
    const radius = Math.hypot(center.x1 - center.x, center.y1 - center.y);
    if (radius < 1) return;
    const { widthPx: width, heightPx: height } = this.session.doc.canvas;
    const inner = parseHex(center.primary);
    const outer = parseHex(center.secondary);
    const mask = radialGradientMask({
      cx: center.x,
      cy: center.y,
      r0: Math.min(center.innerRadius, radius),
      r1: radius,
      stops: [
        [0, inner.r, inner.g, inner.b, inner.a],
        [1, outer.r, outer.g, outer.b, center.edgeMode === "solid" ? outer.a : 0],
      ],
      w: width,
      h: height,
    });
    const view = info.view ?? this.getView?.() ?? { zoom: 1, offsetX: 0, offsetY: 0 };
    drawPixelPreview(ctx, mask, null, view, info.dpr ?? 1, this._preview, true);
    const scale = view.zoom * (info.dpr ?? 1);
    if (typeof ctx.beginPath === "function" && typeof ctx.arc === "function") {
      ctx.save();
      ctx.beginPath();
      ctx.setLineDash?.([4, 3]);
      ctx.strokeStyle = "rgba(255,255,255,0.9)";
      ctx.lineWidth = 1;
      ctx.arc(
        (view.offsetX + center.x * view.zoom) * (info.dpr ?? 1),
        (view.offsetY + center.y * view.zoom) * (info.dpr ?? 1),
        radius * scale,
        0,
        Math.PI * 2,
      );
      ctx.stroke();
      ctx.restore();
    }
  }

  cancel() {
    this._center = null;
    this.requestRender();
  }
  deactivate() { this.cancel(); }
}