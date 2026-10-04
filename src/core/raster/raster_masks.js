// <META - FILE SUMMARY - ellipse / rounded-rect masks and the shared size guard>
//
// Pure module: no DOM, no Session, ESM, Node-importable.
// Mask = { w, h, data: Uint8Array } row-major, 1 = painted.
// Points accept [x, y] arrays or { x, y } objects.
//
// No line mask here: a PIL-exact segment stroke needs the polygon scanline
// engine, so it lives in segment.js next to polygonOutlineMask.

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
