// <META - FILE SUMMARY - Snapping utilities: angle, drag bbox, resize bbox, unit grid>
//
// Pure module: no DOM, no Session, ESM, Node-importable.
// Bboxes accept [x, y, w, h] arrays or { x, y, w, h } objects.
// All functions return [x, y(, w, h)] arrays, matching reference/raster_ref.mjs.

// rnd lives in core/blend.js and is re-exported here so existing importers of
// this module's rnd keep working. It used to be a second, identical definition.
import { rnd } from "../blend.js";
export { rnd };

const toXY = (p) => (Array.isArray(p) ? [p[0], p[1]] : [p.x, p.y]);

const toBBox = (b) => (Array.isArray(b) ? [b[0], b[1], b[2], b[3]] : [b.x, b.y, b.w, b.h]);

// <META - ROLE : snap drag angle to step grid | L14-22>
export function angleSnap(x0, y0, x1, y1, step = 15) {
  const dx = x1 - x0;
  const dy = y1 - y0;
  if (dx === 0 && dy === 0) return [x1, y1];
  const st = (step * Math.PI) / 180;
  const s = rnd(Math.atan2(dy, dx) / st) * st;
  const L = Math.hypot(dx, dy);
  return [x0 + rnd(L * Math.cos(s)), y0 + rnd(L * Math.sin(s))];
}

// <META - ROLE : fresh-draw bbox from drag vector | L24-42>
export function dragBBox(p0, p1, lock = false, center = false) {
  const [ax, ay] = toXY(p0);
  const [bx, by] = toXY(p1);
  const dx = bx - ax;
  const dy = by - ay;
  const adx = Math.abs(dx);
  const ady = Math.abs(dy);
  if (center) {
    let w = 2 * adx + 1;
    let h = 2 * ady + 1;
    if (lock) w = h = Math.max(w, h);
    return [ax - Math.floor((w - 1) / 2), ay - Math.floor((h - 1) / 2), w, h];
  }
  let w = adx + 1;
  let h = ady + 1;
  if (lock) w = h = Math.max(w, h);
  return [dx >= 0 ? ax : ax - w + 1, dy >= 0 ? ay : ay - h + 1, w, h];
}

// <META - ROLE : pending-shape resize (reference port) | L44-84>
export function resizeBBox(b, handle, p, lock = false, center = false, minSize = 1) {
  const [x, y, w, h] = toBBox(b);
  const [px, py] = toXY(p);
  const hx = handle.includes("e") ? 1 : handle.includes("w") ? -1 : 0;
  const hy = handle.includes("s") ? 1 : handle.includes("n") ? -1 : 0;
  const axis = (pos, size, hd, pv) => {
    if (hd === 0) return [pos, size];
    if (center) {
      const delta = hd === 1 ? pv - (pos + size - 1) : pos - pv;
      let ns = size + 2 * delta;
      if (ns < minSize) ns = minSize;
      return [ns === minSize && delta < 0 ? pos + Math.floor((size - ns) / 2) : pos - delta, ns];
    }
    const anchor = hd === 1 ? pos : pos + size - 1;
    const lo = Math.min(anchor, pv);
    const hi = Math.max(anchor, pv);
    return [lo, hi - lo + 1];
  };
  let [nx, nw] = axis(x, w, hx, px);
  let [ny, nh] = axis(y, h, hy, py);
  if (lock && (hx !== 0 || hy !== 0)) {
    if (hx !== 0 && hy !== 0) {
      if (nw * h > nh * w) nh = Math.max(minSize, rnd((nw * h) / w));
      else nw = Math.max(minSize, rnd((nh * w) / h));
    } else if (hx !== 0) {
      nh = Math.max(minSize, rnd((nw * h) / w));
    } else {
      nw = Math.max(minSize, rnd((nh * w) / h));
    }
    const place = (pos, size, hd, pv, ns) => {
      if (center || hd === 0) return pos + Math.floor((size - ns) / 2);
      const anchor = hd === 1 ? pos : pos + size - 1;
      return pv >= anchor ? anchor : anchor - ns + 1;
    };
    nx = place(x, w, hx, px, nw);
    ny = place(y, h, hy, py, nh);
  }
  return [nx, ny, nw, nh];
}

// <META - ROLE : snap bbox to 32px unit grid | L86-94>
export function unitSnap(b) {
  const [x, y, w, h] = toBBox(b);
  return [
    rnd(x / 32) * 32,
    rnd(y / 32) * 32,
    Math.max(32, rnd(w / 32) * 32),
    Math.max(32, rnd(h / 32) * 32),
  ];
}
