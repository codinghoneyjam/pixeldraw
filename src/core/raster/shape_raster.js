// <META - FILE SUMMARY - Shape rasterizer entry point: applyShape through PixelWriter>
//
// Pure module: no DOM, no Session, ESM, Node-importable.
// Delegates to raster_brush, raster_masks, raster_snap submodules.

import { ellipseMask, lineMask, outlineRing, rrectMask } from "./raster_masks.js";

const toBBox = (b) => (Array.isArray(b) ? [b[0], b[1], b[2], b[3]] : [b.x, b.y, b.w, b.h]);

function assertMaskSize(w, h) {
  if (!Number.isInteger(w) || !Number.isInteger(h) || w < 1 || h < 1) {
    throw new RangeError(`mask size must be integers >= 1, got ${w}x${h}`);
  }
}

function assertBrushSize(n) {
  if (!Number.isInteger(n) || n < 1 || n > 64) {
    throw new RangeError(`brush size must be an integer in [1,64], got ${n}`);
  }
}

function writeMask(writer, mask, dx, dy, packed) {
  let count = 0;
  for (let y = 0; y < mask.h; y++) {
    for (let x = 0; x < mask.w; x++) {
      if (mask.data[y * mask.w + x] === 1) {
        writer.set(dx + x, dy + y, packed);
        count++;
      }
    }
  }
  return count;
}

// <META - ROLE : rasterize a ShapeSpec through a PixelWriter | L28-68>
export function applyShape(writer, spec, primaryPacked, secondaryPacked) {
  const kind = spec.kind;
  const sw = spec.strokeWidth === undefined ? 1 : Math.trunc(spec.strokeWidth);
  assertBrushSize(sw);
  if (kind === "line") {
    const m = lineMask(spec.p0, spec.p1, sw);
    let count = 0;
    for (let y = 0; y < m.h; y++) {
      for (let x = 0; x < m.w; x++) {
        if (m.data[y * m.w + x] === 1) {
          writer.set(m.x + x, m.y + y, primaryPacked);
          count++;
        }
      }
    }
    return count;
  }
  const [bx, by, bw, bh] = toBBox(spec.bbox);
  assertMaskSize(bw, bh);
  const radius = kind === "rrect" ? Math.trunc(spec.radius ?? 0) : 0;
  const mode = spec.fillMode ?? "outline";
  const outer = kind === "ellipse" ? ellipseMask(bw, bh) : rrectMask(bw, bh, radius);
  if (mode === "fill") return writeMask(writer, outer, bx, by, primaryPacked);
  const ring = outlineRing(kind === "ellipse" ? "ellipse" : "rrect", bw, bh, radius, sw);
  if (mode === "both") {
    writeMask(writer, outer, bx, by, secondaryPacked);
    return writeMask(writer, ring, bx, by, primaryPacked);
  }
  return writeMask(writer, ring, bx, by, primaryPacked);
}
