import { radialGradientMask } from "../../core/raster/gradient.js";
import { packRGBA, parseHex } from "../../core/pixel.js";
import { Tool } from "../../tools/tool_base.js";

export class GradientTool extends Tool {
  constructor(env) {
    super(env);
    this.innerRadius = 0;
    this.edgeMode = "transparent";
    this._center = null;
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
    };
  }

  pointerUp(ev) {
    if (!this._center) return;
    const center = this._center;
    this._center = null;
    const radius = Math.hypot(Math.round(ev.x) - center.x, Math.round(ev.y) - center.y);
    if (radius < 1) return;
    const { widthPx: width, heightPx: height } = this.session.doc.canvas;
    const inner = parseHex(center.primary);
    const outer = parseHex(center.secondary);
    const mask = radialGradientMask({
      cx: center.x,
      cy: center.y,
      r0: Math.min(this.innerRadius, radius),
      r1: radius,
      stops: [
        [0, inner.r, inner.g, inner.b, inner.a],
        [1, outer.r, outer.g, outer.b, this.edgeMode === "solid" ? outer.a : 0],
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

  cancel() { this._center = null; }
  deactivate() { this.cancel(); }
}