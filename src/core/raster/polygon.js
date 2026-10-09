// <META - FILE SUMMARY - PIL-style polygon fill/outline masks (port of gen_fixtures oracle)>
//
// Pure module: no DOM, no Session, ESM, Node-importable.
// Mask = { w, h, data: Uint8Array } row-major, 1 = painted.
// Points accept [x, y] arrays or { x, y } objects.

import { assertMaskSize } from "./raster_masks.js";
import { DrawToolError } from "../errors.js";

const toXY = (p) => (Array.isArray(p) ? [p[0], p[1]] : [p.x, p.y]);

// <META - ROLE : integer bbox of polygon points | L15-24>
/**
 * Integer bbox of polygon points (inclusive min/max).
 * @param {Array<[number,number]|{x:number,y:number}>} pts polygon points
 * @returns {{x:number,y:number,w:number,h:number}} bbox
 */
export function polygonBBox(pts) {
  if (!Array.isArray(pts) || pts.length === 0) {
    throw new DrawToolError("INVALID_STATE", "polygonBBox needs a non-empty points array");
  }
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const p of pts) {
    const [x, y] = toXY(p);
    if (x < minX) minX = x;
    if (y < minY) minY = y;
    if (x > maxX) maxX = x;
    if (y > maxY) maxY = y;
  }
  return { x: minX, y: minY, w: maxX - minX + 1, h: maxY - minY + 1 };
}

// ---------------------------------------------------------------------------
// PIL raster port (Draw.c): polygon fill, wide line, and 1px line.
// Mirrors tools/gen_fixtures.py polygon_mask/polygon_outline_mask.
// ---------------------------------------------------------------------------
const roundUp = (f) => (f >= 0 ? Math.floor(f + 0.5) : -Math.floor(Math.abs(f) + 0.5));
const roundDown = (f) => (f >= 0 ? Math.ceil(f - 0.5) : -Math.ceil(Math.abs(f) - 0.5));
const roundC = (f) => (f >= 0 ? Math.floor(f + 0.5) : -Math.floor(Math.abs(f) + 0.5)); // C roundf

function polyAddEdge(e, x0, y0, x1, y1) {
  if (x0 <= x1) { e.xmin = x0; e.xmax = x1; } else { e.xmin = x1; e.xmax = x0; }
  if (y0 <= y1) { e.ymin = y0; e.ymax = y1; } else { e.ymin = y1; e.ymax = y0; }
  if (y0 === y1) { e.d = 0; e.dx = 0.0; } else { e.dx = Math.fround((x1 - x0) / (y1 - y0)); e.d = y0 === e.ymin ? 1 : -1; }
  e.x0 = x0; e.y0 = y0;
}

// Edge build with the consecutive-horizontal-merge rule from ImagingDrawPolygon.
function polyBuildEdges(pts) {
  const e = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const [x0, y0] = pts[i], [x1, y1] = pts[i + 1];
    if (y0 === y1 && i !== 0 && y0 === pts[i - 1][1]) {
      const last = e[e.length - 1];
      if (x1 > x0 && x0 > pts[i - 1][0]) { last.xmax = x1; continue; }
      if (x1 < x0 && x0 < pts[i - 1][0]) { last.xmin = x1; continue; }
    }
    const ed = {};
    polyAddEdge(ed, x0, y0, x1, y1);
    e.push(ed);
  }
  const last = pts[pts.length - 1], first = pts[0];
  if (last[0] !== first[0] || last[1] !== first[1]) {
    const ed = {};
    polyAddEdge(ed, last[0], last[1], first[0], first[1]);
    e.push(ed);
  }
  return e;
}

function polyHline(data, W, H, x0, y, x1, val) {
  if (y < 0 || y >= H) return;
  if (x0 < 0) x0 = 0; else if (x0 >= W) return;
  if (x1 < 0) return; else if (x1 >= W) x1 = W - 1;
  if (x0 <= x1) for (let x = x0; x <= x1; x++) data[y * W + x] = val;
}

// PIL polygon_generic (non-alpha path): scanline intersection pairing.
export function polygonGeneric(data, W, H, edges) {
  const table = [];
  let ymin = H - 1, ymax = 0;
  for (const e of edges) {
    if (e.ymin === e.ymax) polyHline(data, W, H, e.xmin, e.ymin, e.xmax, 1);
    else table.push(e);
  }
  for (const e of edges) { if (ymin > e.ymin) ymin = e.ymin; if (ymax < e.ymax) ymax = e.ymax; }
  if (ymin < 0) ymin = 0;
  if (ymax > H) ymax = H;
  const xx = new Float32Array(table.length * 2);
  for (let y = ymin; y <= ymax; y++) {
    let j = 0;
    for (let i = 0; i < table.length; i++) {
      const cur = table[i];
      if (y >= cur.ymin && y <= cur.ymax) {
        xx[j++] = Math.fround(Math.fround(Math.fround(y - cur.y0) * cur.dx) + cur.x0);
        if (y === cur.ymax && y < ymax) {
          xx[j] = xx[j - 1];
          j++;
        } else if ((y === cur.ymin || y === cur.ymax) && cur.dx !== 0) {
          for (let k = 0; k < i; k++) {
            const other = table[k];
            if ((y !== other.ymin && y !== other.ymax) || other.dx === 0) continue;
            if (roundC(xx[j - 1]) === roundC(Math.fround(Math.fround(Math.fround(y - other.y0) * other.dx) + other.x0))) {
              const offset = y === cur.ymax ? -1 : 1;
              const adj = Math.fround(Math.fround(Math.fround(y + offset - cur.y0) * cur.dx) + cur.x0);
              if (y + offset >= other.ymin && y + offset <= other.ymax) {
                const adjO = Math.fround(Math.fround(Math.fround(y + offset - other.y0) * other.dx) + other.x0);
                if (xx[j - 1] > adj + 1 && xx[j - 1] > adjO + 1) xx[j - 1] = roundC(Math.max(adj, adjO)) + 1;
                else if (xx[j - 1] < adj - 1 && xx[j - 1] < adjO - 1) xx[j - 1] = roundC(Math.min(adj, adjO)) - 1;
                break;
              }
            }
          }
        }
      }
    }
    const vals = Array.from(xx.slice(0, j)).sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
    for (let i = 1; i < j; i += 2) polyHline(data, W, H, roundUp(vals[i - 1]), y, roundDown(vals[i]), 1);
  }
}

// PIL line8: Bresenham-ish path of line pixels (endpoint of each segment excluded).
export function line8(data, W, H, x0, y0, x1, y1, val) {
  let dx = x1 - x0, xs, dy = y1 - y0, ys;
  if (dx < 0) { dx = -dx; xs = -1; } else xs = 1;
  if (dy < 0) { dy = -dy; ys = -1; } else ys = 1;
  let n = dx > dy ? dx : dy;
  const plot = (x, y) => { if (x >= 0 && x < W && y >= 0 && y < H) data[y * W + x] = val; };
  if (dx === 0) { for (let i = 0; i < dy; i++) { plot(x0, y0); y0 += ys; } }
  else if (dy === 0) { for (let i = 0; i < dx; i++) { plot(x0, y0); x0 += xs; } }
  else if (dx > dy) {
    n = dx; dy += dy; let e = dy - dx; dx += dx;
    for (let i = 0; i < n; i++) { plot(x0, y0); if (e >= 0) { y0 += ys; e -= dx; } e += dy; x0 += xs; }
  } else {
    n = dy; dx += dx; let e = dx - dy; dy += dy;
    for (let i = 0; i < n; i++) { plot(x0, y0); if (e >= 0) { x0 += xs; e -= dy; } e += dx; y0 += ys; }
  }
}

// PIL ImagingDrawWideLine: widened segment quad filled via polygon_generic.
export function wideLineQuadEdges(x0, y0, x1, y1, width) {
  const dx = x1 - x0, dy = y1 - y0;
  const big = Math.hypot(dx, dy);
  const small = (width - 1) / 2.0;
  const ratioMax = roundUp(small) / big, ratioMin = roundDown(small) / big;
  const dxmin = roundDown(ratioMin * dy), dxmax = roundDown(ratioMax * dy);
  const dymin = roundDown(ratioMin * dx), dymax = roundDown(ratioMax * dx);
  const v = [
    [x0 - dxmin, y0 + dymax],
    [x1 - dxmin, y1 + dymax],
    [x1 + dxmax, y1 - dymin],
    [x0 + dxmax, y0 - dymin],
  ];
  const e = [];
  for (let i = 0; i < 4; i++) {
    const a = v[i], b = v[(i + 1) % 4];
    const ed = {}; polyAddEdge(ed, a[0], a[1], b[0], b[1]); e.push(ed);
  }
  return e;
}

// <META - ROLE : polygon fill mask (goldens) | L167-177>
/**
 * Polygon fill mask matching the PIL oracle.
 * @param {Array<[number,number]>} pts polygon points
 * @param {number} w mask width
 * @param {number} h mask height
 * @returns {{w:number,h:number,data:Uint8Array}} fill mask
 */
export function polygonMask(pts, w, h) {
  assertMaskSize(w, h);
  // Fill-only ground truth: legacy bakers call d.polygon with outline=None,
  // so the scanline fill keeps its boundary ring (no edge erase).
  const data = new Uint8Array(w * h);
  const local = pts.map((p) => toXY(p));
  polygonGeneric(data, w, h, polyBuildEdges(local));
  return { w, h, data };
}

// Python round(): round half to even (banker's rounding), unlike Math.round.
function pyRound(v) {
  const f = Math.floor(v);
  const r = v - f;
  if (r > 0.5) return f + 1;
  if (r < 0.5) return f;
  return f % 2 === 0 ? f : f + 1;
}

// <META - ROLE : fillet polygon corners (port of geometry.py fillet_polygon) | L204-255>
/**
 * Fillet polygon corners: exact port of legacy dev/tools/assets/core/geometry.py fillet_polygon.
 * @param {Array<[number,number]>} pts polygon points
 * @param {number} radius fillet radius
 * @param {number} steps arc samples per corner
 * @returns {Array<[number,number]>} filleted vertex list
 */
export function filletPolygon(pts, radius, steps = 8) {
  const local = pts.map((p) => toXY(p));
  if (radius <= 0.0 || local.length < 3) return local;
  const n = local.length;
  const newPts = [];
  for (let i = 0; i < n; i++) {
    const pPrev = local[(i - 1 + n) % n];
    const pCurr = local[i];
    const pNext = local[(i + 1) % n];
    const v1x = pPrev[0] - pCurr[0], v1y = pPrev[1] - pCurr[1];
    const v2x = pNext[0] - pCurr[0], v2y = pNext[1] - pCurr[1];
    const l1 = Math.hypot(v1x, v1y);
    const l2 = Math.hypot(v2x, v2y);
    if (l1 === 0 || l2 === 0) { newPts.push([...pCurr]); continue; }
    const u1x = v1x / l1, u1y = v1y / l1;
    const u2x = v2x / l2, u2y = v2y / l2;
    const dot = Math.max(-0.999, Math.min(0.999, u1x * u2x + u1y * u2y));
    const halfAngle = Math.acos(dot) / 2.0;
    const tanHalf = Math.tan(halfAngle);
    if (tanHalf < 0.001) { newPts.push([...pCurr]); continue; }

    const maxR = Math.min(radius, l1 * 0.45 * tanHalf, l2 * 0.45 * tanHalf);
    const tangentLen = maxR / tanHalf;
    const pStartX = pCurr[0] + u1x * tangentLen, pStartY = pCurr[1] + u1y * tangentLen;
    const pEndX = pCurr[0] + u2x * tangentLen, pEndY = pCurr[1] + u2y * tangentLen;
    const bisX = u1x + u2x, bisY = u1y + u2y;
    const bLen = Math.hypot(bisX, bisY);
    if (bLen < 0.001) { newPts.push([...pCurr]); continue; }
    const wX = bisX / bLen, wY = bisY / bLen;
    const centerDist = maxR / Math.sin(halfAngle);
    const cx = pCurr[0] + wX * centerDist, cy = pCurr[1] + wY * centerDist;

    const aStart = Math.atan2(pStartY - cy, pStartX - cx);
    const aEnd = Math.atan2(pEndY - cy, pEndX - cx);

    const diff = ((aEnd - aStart + Math.PI + 2.0 * Math.PI) % (2.0 * Math.PI)) - Math.PI;
    for (let s = 0; s <= steps; s++) {
      const ang = aStart + diff * (s / steps);
      newPts.push([pyRound(cx + maxR * Math.cos(ang)), pyRound(cy + maxR * Math.sin(ang))]);
    }
  }
  return newPts;
}

// <META - ROLE : polygon outline mask (closed path stroke) | L179-193>
/**
 * Polygon outline mask matching the PIL oracle (square w x w canvas).
 * Models d.line over the closed path (centered stroke); the oracle renders
 * it padded, which coincides on interior shapes (all golden cases).
 * @param {Array<[number,number]>} pts polygon points
 * @param {number} w canvas size
 * @param {number} width stroke width
 * @returns {{w:number,h:number,data:Uint8Array}} outline mask
 */
export function polygonOutlineMask(pts, w, width) {
  assertMaskSize(w, w);
  const data = new Uint8Array(w * w);
  const local = pts.map((p) => toXY(p));
  const path = [...local, local[0]];
  for (let i = 0; i < path.length - 1; i++) {
    const [x0, y0] = path[i], [x1, y1] = path[i + 1];
    if (width <= 1) line8(data, w, w, x0, y0, x1, y1, 1);
    else polygonGeneric(data, w, w, wideLineQuadEdges(x0, y0, x1, y1, width));
  }
  return { w, h: w, data };
}

/**
 * Inset outline ring for the polygon kind: PIL paints d.polygon outlines
 * strictly INSIDE the fill boundary, unlike d.line strokes which straddle the
 * path (memo T-6). Modelled as the fill mask minus its EUCLIDEAN erosion by
 * the closed disk of radius `width`. Not Manhattan: PIL offsets wide lines
 * perpendicular to the path, so Manhattan under-thins 45-degree slants but
 * matches axis-aligned edges (Pillow 12.1.0, diamond_gem hexagon w3 measured:
 * Euclidean 0 missing / 0 extra, Manhattan 24 missing).
 * @param {{w:number,h:number,data:Uint8Array}} fill fill mask
 * @param {number} width stroke width
 * @returns {{w:number,h:number,data:Uint8Array}} inset ring mask
 */
export function polygonInsetRing(fill, width) {
  const { w: W, h: H } = fill;
  const r = Math.max(0, Math.trunc(width));
  const offsets = [];
  for (let dy = -r; dy <= r; dy++) {
    for (let dx = -r; dx <= r; dx++) {
      if (dx * dx + dy * dy <= r * r) offsets.push(dx, dy);
    }
  }
  const data = new Uint8Array(W * H);
  const isSet = (x, y) => x >= 0 && y >= 0 && x < W && y < H && fill.data[y * W + x] === 1;
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      if (fill.data[y * W + x] !== 1) continue;
      let eroded = 1;
      for (let i = 0; i < offsets.length && eroded === 1; i += 2) {
        if (!isSet(x + offsets[i], y + offsets[i + 1])) eroded = 0;
      }
      data[y * W + x] = eroded === 1 ? 0 : 1;
    }
  }
  return { w: W, h: H, data };
}
