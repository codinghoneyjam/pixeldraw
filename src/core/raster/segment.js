// <META - FILE SUMMARY - PIL-exact single-segment stroke mask (d.line)>
//
// Pure module: no DOM, ESM, Node-importable.
// Mask = { x, y, w, h, data: Uint8Array }, minimal bbox, 1 = painted.
//
// Two regimes, because PIL has two:
//
//	width == 1  Bresenham path (line8).
//	width >= 2  ImagingDrawWideLine widens the segment into a ROTATED quad and
//	            fills it with the polygon scanline engine. Reuses the same
//	            wideLineQuadEdges + polygonGeneric pair that polygonOutlineMask
//	            uses for closed paths, so both stroke paths share one primitive.
//
// The obvious alternative -- stamp a n x n square along a Bresenham walk and
// trim the end caps by projection -- is what PIL is often assumed to do. It is
// wrong: measured against Pillow 12.1.0 over 9 geometries x widths 2..7 it is
// exact on 1 case. The rotated quad is exact on 54/54. See task memo T-7b.

import { assertBrushSize } from "./raster_brush.js";
import { line8, polygonGeneric, wideLineQuadEdges } from "./polygon.js";

/**
 * Stroke a single segment, matching PIL d.line at any integer width.
 * @param {[number, number]|{x:number,y:number}} p0 start point
 * @param {[number, number]|{x:number,y:number}} p1 end point
 * @param {number} n stroke width
 * @returns {{x:number,y:number,w:number,h:number,data:Uint8Array}} minimal-bbox mask
 */
export function lineMask(p0, p1, n) {
  assertBrushSize(n);
  const x0 = Array.isArray(p0) ? p0[0] : p0.x;
  const y0 = Array.isArray(p0) ? p0[1] : p0.y;
  const x1 = Array.isArray(p1) ? p1[0] : p1.x;
  const y1 = Array.isArray(p1) ? p1[1] : p1.y;

  // Pad by the stroke half-width so the quad's corners stay inside the buffer
  // and get the same clipping a full-canvas raster would apply.
  const pad = n;
  const bx = Math.min(x0, x1) - pad;
  const by = Math.min(y0, y1) - pad;
  const w = Math.max(x0, x1) + pad - bx + 1;
  const h = Math.max(y0, y1) + pad - by + 1;
  const data = new Uint8Array(w * h);
  const lx0 = x0 - bx;
  const ly0 = y0 - by;
  const lx1 = x1 - bx;
  const ly1 = y1 - by;

  if (n <= 1) {
    // line8 excludes the far endpoint of each segment; include it so a
    // single-segment stroke covers both endpoints the way PIL does.
    line8(data, w, h, lx0, ly0, lx1, ly1, 1);
    data[(ly1 * w) + lx1] = 1;
  } else {
    polygonGeneric(data, w, h, wideLineQuadEdges(lx0, ly0, lx1, ly1, n));
  }
  return { x: bx, y: by, w, h, data };
}
