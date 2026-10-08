import { PEN_MAX, PEN_MIN } from "../../core/constants.js";
import { DrawToolError } from "../../core/errors.js";

const footprintCache = new Map();

export function brushFootprint(n) {
  if (!Number.isInteger(n) || n < PEN_MIN || n > PEN_MAX) {
    throw new DrawToolError("OUT_OF_RANGE", `pen size ${String(n)} out of range`);
  }
  const cached = footprintCache.get(n);
  if (cached) return cached;
  const mask = new Uint8Array(n * n);
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n; x++) {
      if ((2 * x + 1 - n) ** 2 + (2 * y + 1 - n) ** 2 <= n * n) mask[y * n + x] = 1;
    }
  }
  const fp = Object.freeze({ n, offset: (n - 1) >> 1, mask });
  footprintCache.set(n, fp);
  return fp;
}

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
    if (e2 >= dy) {
      err += dy;
      x0 += sx;
    }
    if (e2 <= dx) {
      err += dx;
      y0 += sy;
    }
  }
}

export function stampBrush(writer, x, y, n, packed) {
  const fp = brushFootprint(n);
  const x0 = x - fp.offset;
  const y0 = y - fp.offset;
  for (let by = 0; by < n; by++) {
    for (let bx = 0; bx < n; bx++) {
      if (fp.mask[by * n + bx] === 1) writer.set(x0 + bx, y0 + by, packed);
    }
  }
}

export function strokeSegment(writer, x0, y0, x1, y1, n, packed) {
  forEachBresenham(x0, y0, x1, y1, (x, y) => stampBrush(writer, x, y, n, packed));
}

export function strokePoint(writer, x, y, n, packed) {
  stampBrush(writer, x, y, n, packed);
}
