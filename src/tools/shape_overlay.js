// <META - FILE SUMMARY - Shape overlay paint layers and canvas rendering>
import { ellipseMask, outlineRing, rrectMask } from "../core/raster/raster_masks.js";
import { lineMask } from "../core/raster/segment.js";
import { DrawToolError } from "../core/errors.js";
import { packRGBA, parseHex } from "../core/pixel.js";

// <META - ROLE : pack a normalized css hex color | L6-12>
export function packOf(css) {
  const c = parseHex(css);
  if (!c) throw new DrawToolError("OUT_OF_RANGE", `invalid color ${String(css)}`);
  return packRGBA(c.r, c.g, c.b, 255);
}

// <META - ROLE : expand a ShapeSpec into colored paint layers | L14-34>
// `color` is the FOREGROUND hex captured when the shape was drawn. The background slot
// is a buffer only and must never paint, so there is exactly one colour here and no
// "both" mode that used to flood the outer area with the secondary colour. See
// shape_render.js buildColor for the capture rule.
export function computePaintLayers(spec, color) {
  const packed = packOf(color);
  if (spec.kind === "line") {
    const m = lineMask(spec.p0, spec.p1, spec.strokeWidth);
    return [{ x: m.x, y: m.y, w: m.w, h: m.h, data: m.data, packed, css: color }];
  }
  const { x, y, w, h } = spec.bbox;
  const rk = spec.kind === "ellipse" ? "ellipse" : "rrect";
  const outer = spec.kind === "ellipse" ? ellipseMask(w, h) : rrectMask(w, h, spec.radius);
  if (spec.fillMode === "fill") return [{ x, y, w, h, data: outer.data, packed, css: color }];
  const ring = outlineRing(rk, w, h, spec.radius, spec.strokeWidth);
  return [{ x, y, w, h, data: ring.data, packed, css: color }];
}

// <META - ROLE : device mapping from canvas px via view and dpr | L38-44>
export function deviceOf(view, dpr, x, y) {
  return { x: (view.offsetX + x * view.zoom) * dpr, y: (view.offsetY + y * view.zoom) * dpr };
}

// <META - ROLE : paint one layer through a reused temp canvas | L46-63>
function blitLayer(ctx, tmpRef, layer, view, dpr, k) {
  if (!tmpRef.el) tmpRef.el = document.createElement("canvas");
  const tctx = tmpRef.el.getContext("2d");
  tmpRef.el.width = layer.w;
  tmpRef.el.height = layer.h;
  const img = tctx.createImageData(layer.w, layer.h);
  const c = parseHex(layer.css) ?? { r: 0, g: 0, b: 0 };
  for (let i = 0; i < layer.w * layer.h; i++) {
    img.data[i * 4] = c.r;
    img.data[i * 4 + 1] = c.g;
    img.data[i * 4 + 2] = c.b;
    img.data[i * 4 + 3] = layer.data[i] === 1 ? 255 : 0;
  }
  tctx.putImageData(img, 0, 0);
  const o = deviceOf(view, dpr, layer.x, layer.y);
  ctx.save();
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(tmpRef.el, o.x, o.y, layer.w * k, layer.h * k);
  ctx.restore();
}

// <META - ROLE : per-pixel fallback when no DOM canvas exists | L65-75>
function fillLayer(ctx, layer, view, dpr, k) {
  ctx.fillStyle = layer.css;
  for (let yy = 0; yy < layer.h; yy++) {
    for (let xx = 0; xx < layer.w; xx++) {
      if (layer.data[yy * layer.w + xx] !== 1) continue;
      const o = deviceOf(view, dpr, layer.x + xx, layer.y + yy);
      ctx.fillRect(o.x, o.y, k, k);
    }
  }
}

// <META - ROLE : dashed bbox outline plus fixed-size handles | L77-95>
function drawFrame(ctx, box, handles, view, dpr) {
  const b0 = deviceOf(view, dpr, box.x, box.y);
  const k = view.zoom * dpr;
  ctx.save();
  if (ctx.setLineDash) ctx.setLineDash([4 * dpr, 3 * dpr]);
  ctx.strokeStyle = "#000000";
  ctx.lineWidth = 1;
  ctx.strokeRect(b0.x, b0.y, Math.max(box.w * k, 1), Math.max(box.h * k, 1));
  ctx.restore();
  const hs = 8 * dpr;
  for (const h of handles) {
    const o = deviceOf(view, dpr, h.x, h.y);
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(o.x - hs / 2, o.y - hs / 2, hs, hs);
    ctx.strokeStyle = "#000000";
    ctx.lineWidth = 1;
    ctx.strokeRect(o.x - hs / 2, o.y - hs / 2, hs, hs);
  }
}

// <META - ROLE : full WYSIWYG overlay entry point | L97-104>
export function renderOverlay(ctx, spec, layers, box, handles, view, dpr, tmpRef) {
  const k = view.zoom * dpr;
  const hasDoc = typeof document !== "undefined";
  for (const layer of layers) {
    if (hasDoc) blitLayer(ctx, tmpRef, layer, view, dpr, k);
    else fillLayer(ctx, layer, view, dpr, k);
  }
  drawFrame(ctx, box, handles, view, dpr);
}
