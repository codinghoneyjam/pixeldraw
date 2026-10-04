// <META - FILE SUMMARY - Flatten document to RGBA + PNG export via shared over()>
// Base = background (or transparent), then visible layers bottom-to-top with opacity.
import { DrawToolError } from "../core/errors.js";
import { over } from "../core/blend.js";
import { parseHex } from "../core/pixel.js";
import { encodePng } from "./png.js";

// <META - ROLE : Resolve export base pixel | L10-22>
function basePixel(doc, includeBackground) {
  if (includeBackground !== true) return [0, 0, 0, 0];
  const bg = doc.canvas.background;
  if (bg === "transparent" || bg === undefined) return [0, 0, 0, 0];
  const c = parseHex(bg);
  if (c === null) throw new DrawToolError("INVALID_STATE", `bad background ${String(bg)}`);
  return [c.r, c.g, c.b, c.a];
}

// <META - ROLE : Composite all visible layers to a full RGBA buffer | L24-68>
export function flattenToRgba(doc, { includeBackground = true } = {}) {
  const w = doc.canvas.widthPx;
  const h = doc.canvas.heightPx;
  const base = basePixel(doc, includeBackground);
  const out = new Uint8ClampedArray(w * h * 4);
  const srcs = [];
  for (const layer of doc.layers) {
    if (!layer || layer.visible !== true) continue;
    const opacity = layer.opacity ?? 1;
    if (!(opacity > 0)) continue;
    if (!layer.store) continue;
    srcs.push({ store: layer.store, opacity });
  }
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      let dr = base[0];
      let dg = base[1];
      let db = base[2];
      let da = base[3];
      for (let l = 0; l < srcs.length; l++) {
        const px = srcs[l].store.getPixel(x, y);
        const sa8 = px >>> 24;
        if (sa8 === 0) continue;
        const sr = px & 255;
        const sg = (px >>> 8) & 255;
        const sb = (px >>> 16) & 255;
        const op = srcs[l].opacity;
        if (sa8 === 255 && op === 1) {
          dr = sr;
          dg = sg;
          db = sb;
          da = 255;
          continue;
        }
        const mixed = over([dr, dg, db, da], [sr, sg, sb, sa8], op);
        dr = mixed[0];
        dg = mixed[1];
        db = mixed[2];
        da = mixed[3];
      }
      const o = (y * w + x) * 4;
      out[o] = dr;
      out[o + 1] = dg;
      out[o + 2] = db;
      out[o + 3] = da;
      if (da === 0) {
        out[o] = 0;
        out[o + 1] = 0;
        out[o + 2] = 0;
      }
    }
  }
  return out;
}

// <META - ROLE : Export flattened document as PNG bytes | L70-74>
/**
 * Export flattened document as PNG bytes.
 *
 * By default this encodes the full canvas. When the document carries a logical
 * viewport (`canvas.viewport`), pass `{ cropToViewport: true }` to encode just
 * that region at the viewport's size instead. The viewport never changes the
 * stored canvas, so ChunkStore alignment is untouched either way.
 * @param {object} doc document to export
 * @param {{includeBackground?:boolean, cropToViewport?:boolean}} opts export options
 * @returns {Promise<Uint8Array>} PNG bytes
 */
export async function exportPngBytes(doc, opts = {}) {
  const rgba = flattenToRgba(doc, opts);
  const vp = opts.cropToViewport === true ? doc.canvas.viewport : null;
  if (!vp) return encodePng(rgba, doc.canvas.widthPx, doc.canvas.heightPx);
  // Composite stays full-canvas (viewport-aware flattening is the caller's
  // business); crop here so a viewport pixel maps 1:1 onto output pixel.
  const cw = vp.w;
  const ch = vp.h;
  const cropped = new Uint8ClampedArray(cw * ch * 4);
  for (let y = 0; y < ch; y++) {
    const srcRow = (vp.y + y) * doc.canvas.widthPx;
    const dstRow = y * cw;
    for (let x = 0; x < cw; x++) {
      const s = (srcRow + vp.x + x) * 4;
      const d = (dstRow + x) * 4;
      cropped[d] = rgba[s];
      cropped[d + 1] = rgba[s + 1];
      cropped[d + 2] = rgba[s + 2];
      cropped[d + 3] = rgba[s + 3];
    }
  }
  return encodePng(cropped, cw, ch);
}
