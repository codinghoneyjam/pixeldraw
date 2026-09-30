// D4 grid: pure line computation (Node-importable) + canvas painting.
// Split out of renderer.js to respect the 300-line file limit.
//
// Modes are mutually exclusive ladders, not additive overlays:
//   off   -> no lines | unit -> 32px only | tile -> 64px only
//   pixel -> 1px + 32px + 64px (the inspect-everything mode)
//
// A stroke entry stays `{ g, a }` only — `a` is the palette marker that tells
// paintGrid which weight/contrast profile to use, so gridLines stays a pure,
// DOM-free function and the entry shape stays stable for tests.

import { CHUNK_PX, TILE_PX } from "../core/constants.js";

export const GRID_ALPHA_THIN = 0.3;
export const GRID_ALPHA_THICK = 0.55;
export const GRID_ALPHA_PIXEL = 0.15;

// Stroke widths are declared in CSS pixels and converted to whole device pixels.
const THIN_CSS_PX = 1;
const THICK_CSS_PX = 2;
const BEVEL_DEV_PX = 1;

// META - CONTRAST STRATEGY
// `difference` with a white stroke moves the backdrop toward its own inverse,
// so a mid-tone pixel (the checkerboard seam, average mid-value artwork) nets
// out to almost no change and the line vanishes exactly there. Instead every
// visible line is stamped as a two-tone bevel: a light run plus a one-device-
// pixel dark run on its trailing edge, both `source-over`. Whatever colour sits
// underneath, at least one of the two runs contrasts with it, so the grid reads
// over light AND dark content while total ink stays low enough not to bury the
// artwork. The 64px tile grid doubles the run to 2 CSS px and inks both passes
// harder, which makes "unit" and "tile" unmistakable at a glance.
// The 1px inspect grid keeps the old faint `difference` hairline: at z >= 8 its
// spacing is >= 8 CSS px and it is a precision aid that must not compete with
// the artwork, so it needs no bevel.

// <META - ROLE : whole-device-pixel run width for a stroke alpha | L41-L45>
// CEIL, not round: a 1 CSS px line at dpr 1.25 must not round down to 1 device
// px (0.8 CSS px) or it disappears again. The 1px inspect rung is a fixed
// hairline by design and stays 1 device px at every dpr.
function strokeDevPx(a, dpr) {
  if (a === GRID_ALPHA_PIXEL) return 1;
  const css = a === GRID_ALPHA_THICK ? THICK_CSS_PX : THIN_CSS_PX;
  return Math.max(1, Math.ceil(css * dpr));
}

// <META - ROLE : per-pass ink; thick strokes out-ink thin ones | L48-L51>
function passAlpha(a, dark) {
  if (a === GRID_ALPHA_THICK) return dark ? 0.5 : 0.44;
  return dark ? 0.32 : 0.3;
}

export function gridStrokeDevPx(a, dpr) {
  return strokeDevPx(a, dpr);
}

// <META - ROLE : stamp k*step coords in [lo,hi] clipped to [0,max] | L58-L62>
function pushLines(arr, lo, hi, max, step, alpha) {
  const k0 = Math.max(0, Math.ceil(Math.max(0, lo) / step));
  const k1 = Math.min(max / step, Math.floor(Math.min(max, hi) / step));
  for (let k = k0; k <= k1; k++) arr.push({ g: k * step, a: alpha });
}

// <META - ROLE : pure line list for a view/mode, null when zoom hides them all | L65-L95>
export function gridLines(view, w, h, vw, vh, mode) {
  const z = view.zoom;
  // Thresholds stay at 6 CSS px of on-screen spacing for both rungs: 6 CSS px
  // is 9 device px at dpr 1.5, far wider than the 3-device-px tile bevel, so
  // the bevel can never close the gap between two neighbouring lines.
  const showUnit = (mode === "unit" || mode === "pixel") && CHUNK_PX * z >= 6;
  const showTile = (mode === "tile" || mode === "pixel") && TILE_PX * z >= 6;
  const showPixel = mode === "pixel" && z >= 8;
  if (!showUnit && !showTile && !showPixel) return null;
  const xA = (0 - view.offsetX) / z;
  const xB = (vw - view.offsetX) / z;
  const yA = (0 - view.offsetY) / z;
  const yB = (vh - view.offsetY) / z;
  const xs = [];
  const ys = [];
  // Order matters: the pixel rung must stay at index 0 (pixel mode is pushed
  // thinnest-first, which also leaves tile lines drawn over unit lines).
  if (showPixel) {
    pushLines(xs, xA, xB, w, 1, GRID_ALPHA_PIXEL);
    pushLines(ys, yA, yB, h, 1, GRID_ALPHA_PIXEL);
  }
  if (showUnit) {
    pushLines(xs, xA, xB, w, CHUNK_PX, GRID_ALPHA_THIN);
    pushLines(ys, yA, yB, h, CHUNK_PX, GRID_ALPHA_THIN);
  }
  if (showTile) {
    pushLines(xs, xA, xB, w, TILE_PX, GRID_ALPHA_THICK);
    pushLines(ys, yA, yB, h, TILE_PX, GRID_ALPHA_THICK);
  }
  return { xs, ys };
}

// <META - ROLE : paint one grid run (hairline or two-tone bevel) on either axis | L98-L112>
function strokeRun(ctx, a, x, y, len, horizontal, dpr) {
  const px = strokeDevPx(a, dpr);
  const hair = a === GRID_ALPHA_PIXEL;
  const main = hair ? 1 : px;
  const cw = horizontal ? (hair ? len : main) : main;
  const ch = horizontal ? main : (hair ? len : main);
  ctx.globalCompositeOperation = hair ? "difference" : "source-over";
  ctx.fillStyle = `rgba(255,255,255,${hair ? a : passAlpha(a, false)})`;
  ctx.fillRect(x, y, cw, ch);
  if (hair) return;
  ctx.fillStyle = `rgba(0,0,0,${passAlpha(a, true)})`;
  const bx = horizontal ? x : x + px - BEVEL_DEV_PX;
  const by = horizontal ? y + px - BEVEL_DEV_PX : y;
  ctx.fillRect(bx, by, horizontal ? len : BEVEL_DEV_PX, horizontal ? BEVEL_DEV_PX : len);
}

// <META - ROLE : device-space painter for a gridLines() result | L115-L134>
export function paintGrid(ctx, lines, view, dx, dy, dw, dh, devW, devH, dpr) {
  if (!lines) return;
  const y0 = Math.max(dy, 0);
  const y1 = Math.min(dy + dh, devH);
  const gx0 = Math.max(dx, 0);
  const gx1 = Math.min(dx + dw, devW);
  if (y1 <= y0 || gx1 <= gx0) return;
  const sx = Math.max(1, dpr || 1);
  ctx.save();
  ctx.beginPath();
  ctx.rect(dx, dy, dw, dh);
  ctx.clip();
  for (const { g, a } of lines.xs) {
    strokeRun(ctx, a, Math.round((view.offsetX + g * view.zoom) * sx), y0, y1 - y0, false, sx);
  }
  for (const { g, a } of lines.ys) {
    strokeRun(ctx, a, gx0, Math.round((view.offsetY + g * view.zoom) * sx), gx1 - gx0, true, sx);
  }
  ctx.restore();
}
