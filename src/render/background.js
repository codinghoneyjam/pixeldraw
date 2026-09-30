// D3 step 2c: canvas background (screen-fixed checkerboard + background fill).
// Split out of renderer.js to respect the 300-line file limit.

import { parseHex } from "../core/pixel.js";

export const CHECKER_A = "#ffffff";
export const CHECKER_B = "#d9d9d9";

export function paintChecker(ctx, dx, dy, dw, dh, dpr, checkerCssPx = 8, viewport = null) {
  const cell = Math.max(1, Math.round(checkerCssPx * dpr));
  // Intersect with the visible viewport (device px) so cost is viewport-bound,
  // not zoom-bound. Cell phase stays anchored at the device origin, so the
  // painted pixels are identical to the full-rect loop.
  let vx0 = dx;
  let vy0 = dy;
  let vx1 = dx + dw;
  let vy1 = dy + dh;
  if (viewport && Number.isFinite(viewport.vw) && Number.isFinite(viewport.vh)) {
    vx0 = Math.max(vx0, 0);
    vy0 = Math.max(vy0, 0);
    vx1 = Math.min(vx1, viewport.vw);
    vy1 = Math.min(vy1, viewport.vh);
    if (vx1 <= vx0 || vy1 <= vy0) return;
  }
  ctx.fillStyle = CHECKER_A;
  ctx.fillRect(vx0, vy0, vx1 - vx0, vy1 - vy0);
  ctx.fillStyle = CHECKER_B;
  const x0 = Math.floor(vx0 / cell) * cell;
  const y0 = Math.floor(vy0 / cell) * cell;
  let iy = Math.round(y0 / cell);
  for (let y = y0; y < vy1; y += cell, iy++) {
    let ix = Math.round(x0 / cell);
    for (let x = x0; x < vx1; x += cell, ix++) {
      if ((((ix + iy) % 2) + 2) % 2 !== 0) ctx.fillRect(x, y, cell, cell);
    }
  }
}

export function paintBackground(ctx, background, dx, dy, dw, dh, dpr, viewport = null) {
  const bg = background === "transparent" || typeof background !== "string" ? null : parseHex(background);
  if (!bg || bg.a === 0) {
    paintChecker(ctx, dx, dy, dw, dh, dpr, 8, viewport);
    return;
  }
  if (bg.a !== 255) paintChecker(ctx, dx, dy, dw, dh, dpr, 8, viewport);
  ctx.fillStyle = bg.a === 255
    ? `rgb(${bg.r},${bg.g},${bg.b})`
    : `rgba(${bg.r},${bg.g},${bg.b},${(bg.a / 255).toFixed(3)})`;
  ctx.fillRect(dx, dy, dw, dh);
}
