// Pixel packing + hex parsing (docs/core.md, pixel.js).

const scratchBytes = new Uint8ClampedArray(4);
const scratchU32 = new Uint32Array(scratchBytes.buffer);

export const TRANSPARENT = 0;

function clampByte(v) {
  v = Math.round(Number(v));
  if (!Number.isFinite(v)) return 0;
  return v < 0 ? 0 : v > 255 ? 255 : v;
}

export function packRGBA(r, g, b, a) {
  a = clampByte(a);
  if (a === 0) return 0;
  scratchBytes[0] = clampByte(r);
  scratchBytes[1] = clampByte(g);
  scratchBytes[2] = clampByte(b);
  scratchBytes[3] = a;
  return scratchU32[0];
}

export function unpackRGBA(p) {
  scratchU32[0] = p >>> 0;
  return [scratchBytes[0], scratchBytes[1], scratchBytes[2], scratchBytes[3]];
}

export function parseHex(s) {
  if (typeof s !== "string") return null;
  const t = s.trim().toLowerCase();
  if (!t.startsWith("#")) return null;
  const h = t.slice(1);
  const hex = (c) => parseInt(c, 16);
  if (/^[0-9a-f]{3}$/.test(h)) {
    return { r: hex(h[0] + h[0]), g: hex(h[1] + h[1]), b: hex(h[2] + h[2]), a: 255 };
  }
  if (/^[0-9a-f]{6}$/.test(h)) {
    return { r: hex(h.slice(0, 2)), g: hex(h.slice(2, 4)), b: hex(h.slice(4, 6)), a: 255 };
  }
  if (/^[0-9a-f]{8}$/.test(h)) {
    return { r: hex(h.slice(0, 2)), g: hex(h.slice(2, 4)), b: hex(h.slice(4, 6)), a: hex(h.slice(6, 8)) };
  }
  return null;
}

export function toHex(r, g, b) {
  const hx = (v) => clampByte(v).toString(16).padStart(2, "0");
  return `#${hx(r)}${hx(g)}${hx(b)}`;
}

export function toHex8(r, g, b, a) {
  const hx = (v) => clampByte(v).toString(16).padStart(2, "0");
  return `#${hx(r)}${hx(g)}${hx(b)}${hx(a)}`;
}
