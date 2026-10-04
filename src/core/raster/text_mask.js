// <META - FILE SUMMARY - Bitmap-glyph text raster with brute-force disk stroke>
//
// Pure module: no DOM, ESM, Node-importable. Fonts are never parsed here;
// glyphs come from the committed text_glyphs.json bitmaps (determinism).
// Mirrors the legacy O(w^2) disk stroke: the glyph is stamped at every
// integer (ox, oy) with ox*ox+oy*oy <= sw*sw in stroke color, then once at
// the pen in fill color. Blending is src-over per stamp, same order.

import { assertMaskSize } from "./raster_masks.js";
import { over } from "../blend.js";
import { packRGBA, unpackRGBA } from "../pixel.js";

// <META - ROLE : Blend one glyph stamp over the canvas with src-over | L14-34>
function stampGlyph(data, W, H, glyph, atX, atY, color) {
  const [cr, cg, cb] = color;
  for (let gy = 0; gy < glyph.h; gy++) {
    const y = atY + gy;
    if (y < 0 || y >= H) continue;
    for (let gx = 0; gx < glyph.w; gx++) {
      const cov = glyph.alpha[gy * glyph.w + gx];
      if (cov === 0) continue;
      const x = atX + gx;
      if (x < 0 || x >= W) continue;
      const i = y * W + x;
      const dst = unpackRGBA(data[i]);
      const blended = over(dst, [cr, cg, cb, cov], 1);
      data[i] = packRGBA(blended[0], blended[1], blended[2], blended[3]);
    }
  }
}

// <META - ROLE : Rasterize text from baked glyphs with disk stroke | L36-84>
export function textMask({ text, glyphs, x, y, fill, stroke, strokeWidth, strokeMode, w, h }) {
  assertMaskSize(w, h);
  const data = new Uint32Array(w * h);
  const placed = placeText(glyphs, text, x, y);
  // Packed u32 colors (same contract as applyShape); unpacked once here.
  const fillRgb = unpackRGBA(fill >>> 0).slice(0, 3);
  const strokeRgb = unpackRGBA((stroke ?? fill) >>> 0).slice(0, 3);
  const sw = Math.trunc(strokeWidth ?? 0);
  if (stroke !== 0 && sw > 0) {
    // PIL-shaped stroke (d.text stroke_width engine path): baked stroked
    // alpha stamped once. Disk stroke (brute-force d.text stamps) otherwise.
    // Disk stamping does NOT reproduce PIL's vector stroke (measured).
    const mode = strokeMode ?? "shape";
    for (const p of placed) {
      const shaped = mode === "shape" ? p.glyph.shaped?.[String(sw)] : undefined;
      if (shaped) {
        stampGlyph(data, w, h, shaped, p.x - p.glyph.dx + shaped.dx, p.y - p.glyph.dy + shaped.dy, strokeRgb);
      } else {
        for (let ox = -sw; ox <= sw; ox++) {
          for (let oy = -sw; oy <= sw; oy++) {
            if (ox * ox + oy * oy <= sw * sw) stampGlyph(data, w, h, p.glyph, p.x + ox, p.y + oy, strokeRgb);
          }
        }
      }
    }
  }
  for (const p of placed) stampGlyph(data, w, h, p.glyph, p.x, p.y, fillRgb);
  return { w, h, data };
}

// <META - ROLE : Place chars at integer pen from glyph bearings/advances | L70-84>
export function placeText(glyphs, text, penX, penY) {
  // Integer pen: baked advances are integers (monogram 90, default-42 27),
  // matching PIL's integer pen layout for these fonts (verified at bake).
  const out = [];
  let pen = penX;
  for (const ch of text) {
    const g = glyphs[ch];
    if (!g) throw new RangeError(`glyph missing for ${JSON.stringify(ch)}`);
    out.push({ ch, x: Math.round(pen + g.dx), y: Math.round(penY + g.dy), glyph: g });
    pen += g.advance;
  }
  return out;
}

// <META - ROLE : Measure string ink bbox from glyph metrics | L86-102>
export function measureText(glyphs, text) {
  const placed = placeText(glyphs, text, 0, 0);
  if (placed.length === 0) return { x0: 0, y0: 0, x1: 0, y1: 0, w: 0, h: 0 };
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const p of placed) {
    if (p.x < minX) minX = p.x;
    if (p.y < minY) minY = p.y;
    if (p.x + p.glyph.w > maxX) maxX = p.x + p.glyph.w;
    if (p.y + p.glyph.h > maxY) maxY = p.y + p.glyph.h;
  }
  // Legacy textbbox((0,0)) left edge sits at the pen (default-42 "A" ink
  // starts at dx=1 but bbox x0=0); width runs to the advance edge.
  // Height is ink-based.
  const x0 = Math.min(0, minX);
  const last = placed[placed.length - 1];
  const advanceRight = last ? last.x - last.glyph.dx + last.glyph.advance : maxX;
  const x1 = Math.max(maxX, advanceRight);
  return { x0, y0: minY, x1, y1: maxY, w: x1 - x0, h: maxY - minY };
}
