// NON-NORMATIVE reference for render/view.js (pure math). Worked examples are asserted by tools/view_check.mjs.
export const ZOOM_LEVELS = [0.25, 0.5, 1, 2, 3, 4, 6, 8, 12, 16, 24, 32];
export const createView = (o = {}) => ({ zoom: o.zoom ?? 1, offsetX: o.offsetX ?? 0, offsetY: o.offsetY ?? 0 });
export const screenToCanvas = (v, sx, sy) => ({ x: (sx - v.offsetX) / v.zoom, y: (sy - v.offsetY) / v.zoom });
export const canvasToScreen = (v, x, y) => ({ x: v.offsetX + x * v.zoom, y: v.offsetY + y * v.zoom });
export const pixelAt = (v, sx, sy) => { const p = screenToCanvas(v, sx, sy); return { x: Math.floor(p.x), y: Math.floor(p.y) }; };
export const zoomAt = (v, ax, ay, nz) => ({ zoom: nz, offsetX: ax - (ax - v.offsetX) * (nz / v.zoom), offsetY: ay - (ay - v.offsetY) * (nz / v.zoom) });
export function stepZoom(z, dir) {
  if (dir > 0) return ZOOM_LEVELS.find((l) => l > z) ?? ZOOM_LEVELS.at(-1);
  return [...ZOOM_LEVELS].reverse().find((l) => l < z) ?? ZOOM_LEVELS[0];
}
export const panBy = (v, dx, dy) => ({ ...v, offsetX: v.offsetX + dx, offsetY: v.offsetY + dy });
export function clampView(v, cw, ch, vw, vh, margin = 32) {
  const cl = (val, lo, hi) => Math.min(Math.max(val, lo), hi);
  return { ...v, offsetX: cl(v.offsetX, margin - cw * v.zoom, vw - margin), offsetY: cl(v.offsetY, margin - ch * v.zoom, vh - margin) };
}
export function fitView(cw, ch, vw, vh, padding = 16) {
  const want = Math.min((vw - 2 * padding) / cw, (vh - 2 * padding) / ch);
  const zoom = [...ZOOM_LEVELS].reverse().find((l) => l <= want) ?? ZOOM_LEVELS[0];
  return { zoom, offsetX: Math.round((vw - cw * zoom) / 2), offsetY: Math.round((vh - ch * zoom) / 2) };
}
export function visibleChunkRange(v, cw, ch, vw, vh) {
  const a = screenToCanvas(v, 0, 0), b = screenToCanvas(v, vw, vh);
  const cx0 = Math.max(0, Math.floor(a.x / 32)), cy0 = Math.max(0, Math.floor(a.y / 32));
  const cx1 = Math.min(cw / 32 - 1, Math.floor((b.x - 1e-9) / 32)), cy1 = Math.min(ch / 32 - 1, Math.floor((b.y - 1e-9) / 32));
  return cx1 < cx0 || cy1 < cy0 ? null : { cx0, cy0, cx1, cy1 };
}
