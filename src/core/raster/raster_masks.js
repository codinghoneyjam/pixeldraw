// <META - FILE SUMMARY - Shape mask generators: ellipse, rrect, outline ring, line>
//
// Pure module: no DOM, no Session, ESM, Node-importable.
// Mask = { w, h, data: Uint8Array } row-major, 1 = painted.
// Points accept [x, y] arrays or { x, y } objects.

import { assertBrushSize, brushFootprint, forEachBresenham } from "./raster_brush.js";

/**
 * Canonical mask-size guard. Every mask entry asserts at entry (Slice D boundary).
 * @param {number} w mask width
 * @param {number} h mask height
 * @returns {void}
 */
export function assertMaskSize(w, h) {
  if (!Number.isInteger(w) || !Number.isInteger(h) || w < 1 || h < 1) {
    throw new RangeError(`mask size must be integers >= 1, got ${w}x${h}`);
  }
}

const toXY = (p) => (Array.isArray(p) ? [p[0], p[1]] : [p.x, p.y]);

// <META - ROLE : ellipse mask with empty row/col correction | L16-44>
export function ellipseMask(w, h) {
  assertMaskSize(w, h);
  const data = new Uint8Array(w * h);
  const rhs = w * w * h * h;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const lhs = (2 * x + 1 - w) ** 2 * h * h + (2 * y + 1 - h) ** 2 * w * w;
      if (lhs <= rhs) data[y * w + x] = 1;
    }
  }
  const xs = [...new Set([Math.floor((w - 1) / 2), Math.floor(w / 2)])];
  const ys = [...new Set([Math.floor((h - 1) / 2), Math.floor(h / 2)])];
  for (let y = 0; y < h; y++) {
    let any = false;
    for (let x = 0; x < w; x++) {
      if (data[y * w + x] === 1) { any = true; break; }
    }
    if (!any) for (const x of xs) data[y * w + x] = 1;
  }
  for (let x = 0; x < w; x++) {
    let any = false;
    for (let y = 0; y < h; y++) {
      if (data[y * w + x] === 1) { any = true; break; }
    }
    if (!any) for (const y of ys) data[y * w + x] = 1;
  }
  return { w, h, data };
}

// <META - ROLE : rounded-rect mask with X=2x+1 corner rule | L46-69>
export function rrectMask(w, h, r) {
  assertMaskSize(w, h);
  r = Math.max(0, Math.min(Math.trunc(r), Math.floor(w / 2), Math.floor(h / 2)));
  const data = new Uint8Array(w * h);
  if (r === 0) {
    data.fill(1);
    return { w, h, data };
  }
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const X = 2 * x + 1;
      const Y = 2 * y + 1;
      const cx = X < 2 * r ? 2 * r : X > 2 * (w - r) ? 2 * (w - r) : null;
      const cy = Y < 2 * r ? 2 * r : Y > 2 * (h - r) ? 2 * (h - r) : null;
      data[y * w + x] = cx !== null && cy !== null
        ? ((X - cx) ** 2 + (Y - cy) ** 2 <= 4 * r * r ? 1 : 0)
        : 1;
    }
  }
  return { w, h, data };
}

// <META - ROLE : inner-aligned outline ring (2n>=min => solid) | L71-99>
export function outlineRing(kind, w, h, r = 0, n = 1) {
  assertMaskSize(w, h);
  const outer = kind === "ellipse" ? ellipseMask(w, h) : rrectMask(w, h, r);
  const iw = w - 2 * n;
  const ih = h - 2 * n;
  const data = new Uint8Array(w * h);
  if (iw < 1 || ih < 1) {
    data.set(outer.data);
    return { w, h, data };
  }
  const clampedR = Math.max(Math.min(Math.trunc(r), Math.floor(w / 2), Math.floor(h / 2)), 0);
  const rr = Math.max(clampedR - n, 0);
  const im = kind === "ellipse" ? ellipseMask(iw, ih) : rrectMask(iw, ih, rr);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      let inner = 0;
      if (x >= n && x < n + iw && y >= n && y < n + ih) inner = im.data[(y - n) * iw + (x - n)];
      data[y * w + x] = outer.data[y * w + x] === 1 && inner === 0 ? 1 : 0;
    }
  }
  return { w, h, data };
}

// <META - ROLE : line mask as brush-stamp union with minimal bbox | L101-139>
/**
 * Line mask as brush-stamp union. Brush guard lives in raster_brush.js.
 * @param {[number, number]|{x:number,y:number}} p0 start point
 * @param {[number, number]|{x:number,y:number}} p1 end point
 * @param {number} n brush size
 * @returns {{x:number,y:number,w:number,h:number,data:Uint8Array}} line mask
 */
export function lineMask(p0, p1, n) {
  assertBrushSize(n);
  const [x0, y0] = toXY(p0);
  const [x1, y1] = toXY(p1);
  const { offset, mask: bm } = brushFootprint(n);
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  const visit = (px, py) => {
    for (let by = 0; by < n; by++) {
      for (let bx = 0; bx < n; bx++) {
        if (bm[by * n + bx] === 0) continue;
        const x = px - offset + bx;
        const y = py - offset + by;
        if (x < minX) minX = x;
        if (y < minY) minY = y;
        if (x > maxX) maxX = x;
        if (y > maxY) maxY = y;
      }
    }
  };
  forEachBresenham(x0, y0, x1, y1, visit);
  const w = maxX - minX + 1;
  const h = maxY - minY + 1;
  const data = new Uint8Array(w * h);
  const paint = (px, py) => {
    for (let by = 0; by < n; by++) {
      for (let bx = 0; bx < n; bx++) {
        if (bm[by * n + bx] === 0) continue;
        data[(py - offset + by - minY) * w + (px - offset + bx - minX)] = 1;
      }
    }
  };
  forEachBresenham(x0, y0, x1, y1, paint);
  return { x: minX, y: minY, w, h, data };
}
