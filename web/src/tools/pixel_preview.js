import { parseHex, unpackRGBA } from "../core/pixel.js";

export function drawPixelPreview(ctx, mask, color, view, dpr, cache, packed = false) {
  if (!ctx || typeof document === "undefined" || typeof ctx.drawImage !== "function") return;
  if (!cache.canvas) cache.canvas = document.createElement("canvas");
  const canvas = cache.canvas;
  if (canvas.width !== mask.w || canvas.height !== mask.h) {
    canvas.width = mask.w;
    canvas.height = mask.h;
  }
  const previewCtx = canvas.getContext("2d");
  const image = previewCtx.createImageData(mask.w, mask.h);
  const rgba = packed ? null : parseHex(color);
  for (let i = 0; i < mask.data.length; i++) {
    const pixel = mask.data[i];
    if (pixel === 0) continue;
    const offset = i * 4;
    if (packed) {
      const [r, g, b, a] = unpackRGBA(pixel);
      image.data[offset] = r;
      image.data[offset + 1] = g;
      image.data[offset + 2] = b;
      image.data[offset + 3] = Math.round(a * 0.82);
    } else {
      image.data[offset] = rgba.r;
      image.data[offset + 1] = rgba.g;
      image.data[offset + 2] = rgba.b;
      image.data[offset + 3] = Math.round(rgba.a * 0.82);
    }
  }
  previewCtx.putImageData(image, 0, 0);
  const zoom = view.zoom * dpr;
  ctx.save();
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(
    canvas,
    view.offsetX * dpr,
    view.offsetY * dpr,
    mask.w * zoom,
    mask.h * zoom,
  );
  ctx.restore();
}