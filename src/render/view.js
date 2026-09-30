// Pure view math (DOM-free, Node-importable). Contract: docs/render.md (view.js).
// View: { zoom, offsetX, offsetY }; zoom = CSS px per canvas px,
// offset = screen (CSS px, viewport-relative) position of canvas (0,0) corner.

import { ZOOM_LEVELS, CHUNK_PX } from "../core/constants.js";
import { DrawToolError } from "../core/errors.js";

export { ZOOM_LEVELS };

function assertNum(v, name) {
  if (typeof v !== "number" || !Number.isFinite(v)) {
    throw new DrawToolError("OUT_OF_RANGE", `view.${name} must be a finite number`);
  }
}

export function createView(o = {}) {
  const zoom = o.zoom ?? 1;
  const offsetX = o.offsetX ?? 0;
  const offsetY = o.offsetY ?? 0;
  assertNum(zoom, "zoom");
  assertNum(offsetX, "offsetX");
  assertNum(offsetY, "offsetY");
  if (zoom <= 0) throw new DrawToolError("OUT_OF_RANGE", "view.zoom must be > 0");
  return { zoom, offsetX, offsetY };
}

export function screenToCanvas(v, sx, sy) {
  return { x: (sx - v.offsetX) / v.zoom, y: (sy - v.offsetY) / v.zoom };
}

export function canvasToScreen(v, x, y) {
  return { x: v.offsetX + x * v.zoom, y: v.offsetY + y * v.zoom };
}

export function pixelAt(v, sx, sy) {
  const p = screenToCanvas(v, sx, sy);
  return { x: Math.floor(p.x), y: Math.floor(p.y) };
}

export function zoomAt(v, ax, ay, newZoom) {
  assertNum(newZoom, "zoom");
  if (newZoom <= 0) throw new DrawToolError("OUT_OF_RANGE", "newZoom must be > 0");
  const k = newZoom / v.zoom;
  return { zoom: newZoom, offsetX: ax - (ax - v.offsetX) * k, offsetY: ay - (ay - v.offsetY) * k };
}

export function stepZoom(zoom, dir) {
  if (dir > 0) {
    const up = ZOOM_LEVELS.find((l) => l > zoom);
    return up ?? ZOOM_LEVELS[ZOOM_LEVELS.length - 1];
  }
  if (dir < 0) {
    const down = [...ZOOM_LEVELS].reverse().find((l) => l < zoom);
    return down ?? ZOOM_LEVELS[0];
  }
  return zoom;
}

export function panBy(v, dx, dy) {
  return { ...v, offsetX: v.offsetX + dx, offsetY: v.offsetY + dy };
}

export function clampView(v, canvasW, canvasH, vw, vh, margin = 32) {
  const cl = (val, lo, hi) => Math.min(Math.max(val, lo), hi);
  return {
    ...v,
    offsetX: cl(v.offsetX, margin - canvasW * v.zoom, vw - margin),
    offsetY: cl(v.offsetY, margin - canvasH * v.zoom, vh - margin),
  };
}

export function fitView(canvasW, canvasH, vw, vh, padding = 16) {
  const want = Math.min((vw - 2 * padding) / canvasW, (vh - 2 * padding) / canvasH);
  const found = [...ZOOM_LEVELS].reverse().find((l) => l <= want);
  const zoom = found ?? ZOOM_LEVELS[0];
  return {
    zoom,
    offsetX: Math.round((vw - canvasW * zoom) / 2),
    offsetY: Math.round((vh - canvasH * zoom) / 2),
  };
}

export function actualSizeView(canvasW, canvasH, vw, vh) {
  return {
    zoom: 1,
    offsetX: Math.round((vw - canvasW) / 2),
    offsetY: Math.round((vh - canvasH) / 2),
  };
}

export function visibleChunkRange(v, canvasW, canvasH, vw, vh) {
  const a = screenToCanvas(v, 0, 0);
  const b = screenToCanvas(v, vw, vh);
  const cx0 = Math.max(0, Math.floor(a.x / CHUNK_PX));
  const cy0 = Math.max(0, Math.floor(a.y / CHUNK_PX));
  const cx1 = Math.min(canvasW / CHUNK_PX - 1, Math.floor((b.x - 1e-9) / CHUNK_PX));
  const cy1 = Math.min(canvasH / CHUNK_PX - 1, Math.floor((b.y - 1e-9) / CHUNK_PX));
  if (cx1 < cx0 || cy1 < cy0) return null;
  return { cx0, cy0, cx1, cy1 };
}
