// <META - FILE SUMMARY - Shape rasterizer entry point: applyShape through PixelWriter>
//
// Pure module: no DOM, no Session, ESM, Node-importable.
// Delegates to raster_brush, raster_masks, raster_snap submodules.

import { assertMaskSize, ellipseMask, lineMask, outlineRing, rrectMask } from "./raster_masks.js";
import { assertBrushSize } from "./raster_brush.js";
import { polygonBBox, polygonInsetRing, polygonMask, polygonOutlineMask } from "./polygon.js";
// Guards are single-sourced: assertMaskSize lives in raster_masks.js,
// assertBrushSize lives in raster_brush.js (Slice D separation boundary).

const toBBox = (b) => (Array.isArray(b) ? [b[0], b[1], b[2], b[3]] : [b.x, b.y, b.w, b.h]);

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
/**
 * Rasterize a ShapeSpec through a PixelWriter. Guards: sw undefined->1,
 * trunc+assertBrushSize, zero-area bbox fails via assertMaskSize.
 * @param {{set:(x:number,y:number,p:number)=>void}} writer pixel writer
 * @param {{kind:string,strokeWidth?:number,bbox?:*,p0?:*,p1?:*,radius?:number,fillMode?:string}} spec shape spec
 * @param {number} primaryPacked primary color
 * @param {number} secondaryPacked secondary color
 * @returns {number} painted pixel count
 */
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
  if (kind === "polygon") {
    const bb = polygonBBox(spec.points);
    assertMaskSize(bb.w, bb.h);
    const local = spec.points.map((p) => (Array.isArray(p) ? [p[0] - bb.x, p[1] - bb.y] : [p.x - bb.x, p.y - bb.y]));
    const pmode = spec.fillMode ?? "outline";
    const pfill = polygonMask(local, bb.w, bb.h);
    if (pmode === "fill") return writeMask(writer, pfill, bb.x, bb.y, primaryPacked);
    // PIL polygon-outline rule (measured on PIL: square probe w1..w4):
    // width 1 strokes centered like d.line; width >= 2 paints a full-width
    // ring inset strictly inside the fill boundary.
    if (sw >= 2) {
      const pring = polygonInsetRing(pfill, sw);
      if (pmode === "both") {
        writeMask(writer, pfill, bb.x, bb.y, secondaryPacked);
        return writeMask(writer, pring, bb.x, bb.y, primaryPacked);
      }
      return writeMask(writer, pring, bb.x, bb.y, primaryPacked);
    }
    const pring = polygonOutlineMask(local, Math.max(bb.w, bb.h), sw);
    if (pmode === "both") {
      writeMask(writer, pfill, bb.x, bb.y, secondaryPacked);
      return writeMask(writer, pring, bb.x, bb.y, primaryPacked);
    }
    return writeMask(writer, pring, bb.x, bb.y, primaryPacked);
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
