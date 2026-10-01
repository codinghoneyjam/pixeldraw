// <META - FILE SUMMARY - Brush footprint and Bresenham line algorithm>
//
// Pure module: no DOM, no Session, ESM, Node-importable.
// All decisions are integer math; rounding is always rnd(v)=floor(v+0.5).

/**
 * Canonical brush-size guard. Stays with brush code through Slice D split.
 * @param {number} n brush size
 * @returns {void}
 */
export function assertBrushSize(n) {
  if (!Number.isInteger(n) || n < 1 || n > 64) {
    throw new RangeError(`brush size must be an integer in [1,64], got ${n}`);
  }
}

// <META - ROLE : brush footprint per integer rounding rule | L14-30>
const brushCache = new Map();
export function brushFootprint(n) {
  assertBrushSize(n);
  let hit = brushCache.get(n);
  if (hit !== undefined) return hit;
  const mask = new Uint8Array(n * n);
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n; x++) {
      if ((2 * x + 1 - n) ** 2 + (2 * y + 1 - n) ** 2 <= n * n) mask[y * n + x] = 1;
    }
  }
  hit = Object.freeze({ n, offset: (n - 1) >> 1, mask });
  brushCache.set(n, hit);
  return hit;
}

// <META - ROLE : integer Bresenham point enumeration | L32-46>
export function forEachBresenham(x0, y0, x1, y1, cb) {
  const dx = Math.abs(x1 - x0);
  const sx = x0 < x1 ? 1 : -1;
  const dy = -Math.abs(y1 - y0);
  const sy = y0 < y1 ? 1 : -1;
  let err = dx + dy;
  for (;;) {
    cb(x0, y0);
    if (x0 === x1 && y0 === y1) return;
    const e2 = 2 * err;
    if (e2 >= dy) { err += dy; x0 += sx; }
    if (e2 <= dx) { err += dx; y0 += sy; }
  }
}
