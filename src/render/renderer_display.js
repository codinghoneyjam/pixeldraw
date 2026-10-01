import { gridLines, paintGrid } from "./grid.js";
import { paintBackground } from "./background.js";

const BORDER_STYLE = "rgba(0,0,0,0.6)";

export function drawDisplay(r, doc, view, w, h) {
  const ctx = r._dctx;
  const dpr = r._dpr || 1;
  const devW = Math.max(1, Math.round(r._cssW * dpr));
  const devH = Math.max(1, Math.round(r._cssH * dpr));
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalCompositeOperation = "source-over";
  ctx.clearRect(0, 0, devW, devH);
  const dx = Math.round(view.offsetX * dpr);
  const dy = Math.round(view.offsetY * dpr);
  const dw = Math.round(w * view.zoom * dpr);
  const dh = Math.round(h * view.zoom * dpr);
  ctx.save();
  ctx.beginPath();
  ctx.rect(dx, dy, dw, dh);
  ctx.clip();
  paintBackground(ctx, doc.canvas.background, dx, dy, dw, dh, dpr, { vw: devW, vh: devH });
  ctx.imageSmoothingEnabled = false;
  if (r._comp) blitVisible(r, ctx, view, w, h, dpr);
  ctx.restore();
  paintGrid(ctx, gridLines(view, w, h, r._cssW, r._cssH, r._gridMode()),
    view, dx, dy, dw, dh, devW, devH, dpr);
  if (dw > 0 && dh > 0) {
    ctx.fillStyle = BORDER_STYLE;
    ctx.fillRect(dx - 1, dy - 1, dw + 2, 1);
    ctx.fillRect(dx - 1, dy + dh, dw + 2, 1);
    ctx.fillRect(dx - 1, dy, 1, dh);
    ctx.fillRect(dx + dw, dy, 1, dh);
  }
  const overlay = r._getOverlay();
  if (overlay) overlay(ctx, { view, dpr, pixelToDevice: (x, y) => r.pixelToDevice(x, y) });
}

export function blitVisible(r, ctx, view, w, h, dpr) {
  const x0 = Math.max(0, Math.floor((0 - view.offsetX) / view.zoom));
  const y0 = Math.max(0, Math.floor((0 - view.offsetY) / view.zoom));
  const sw = Math.min(w, Math.ceil((r._cssW - view.offsetX) / view.zoom)) - x0;
  const sh = Math.min(h, Math.ceil((r._cssH - view.offsetY) / view.zoom)) - y0;
  if (sw <= 0 || sh <= 0) return;
  const ddw = Math.round(sw * view.zoom * dpr);
  const ddh = Math.round(sh * view.zoom * dpr);
  if (ddw <= 0 || ddh <= 0) return;
  ctx.drawImage(
    r._comp, x0, y0, sw, sh,
    Math.round((view.offsetX + x0 * view.zoom) * dpr),
    Math.round((view.offsetY + y0 * view.zoom) * dpr),
    ddw, ddh,
  );
}
