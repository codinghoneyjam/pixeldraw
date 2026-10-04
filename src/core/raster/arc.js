// <META - FILE SUMMARY - PIL-style arc/chord masks (port of ImageDraw arc/chord)>
//
// Pure module: no DOM, ESM, Node-importable.
// Mask = { w, h, data: Uint8Array } row-major, 1 = painted. Masks span the
// full canvas (arcs are positioned by float center/radii, not by bbox).
// Angles follow the PIL convention: 0 deg = 3 o'clock, increasing clockwise.

import { assertMaskSize } from "./raster_masks.js";
import { line8, polygonGeneric, polygonMask, wideLineQuadEdges } from "./polygon.js";

const DEG = Math.PI / 180;

// <META - ROLE : Sample ellipse perimeter points between two PIL angles | L15-33>
export function arcEllipsePoints(cx, cy, rx, ry, startDeg, endDeg, stepDeg = 0.5) {
  let end = endDeg;
  while (end < startDeg) end += 360;
  const span = end - startDeg;
  const steps = Math.max(1, Math.ceil(span / stepDeg));
  const pts = [];
  for (let i = 0; i <= steps; i++) {
    const a = (startDeg + (span * i) / steps) * DEG;
    pts.push([cx + rx * Math.cos(a), cy + ry * Math.sin(a)]);
  }
  return pts;
}

// <META - ROLE : Stroke an open point path segment-by-segment | L35-60>
function strokeOpenPath(data, W, H, pts, width) {
  for (let i = 0; i < pts.length - 1; i++) {
    // Integer plotting: line8/wide quads address grid pixels (as in PIL,
    // which resolves float coords to ints). Shared endpoints round
    // identically, so the chained path stays continuous.
    const x0 = Math.round(pts[i][0]);
    const y0 = Math.round(pts[i][1]);
    const x1 = Math.round(pts[i + 1][0]);
    const y1 = Math.round(pts[i + 1][1]);
    if (x0 === x1 && y0 === y1) {
      // Degenerate after rounding (dense sampling on flat curve regions):
      // paint a width-sized dot, matching PIL's zero-length wide line.
      dotRect(data, W, H, x0, y0, width);
      continue;
    }
    if (width <= 1) line8(data, W, H, x0, y0, x1, y1, 1);
    else polygonGeneric(data, W, H, wideLineQuadEdges(x0, y0, x1, y1, width));
  }
}

// <META - ROLE : Fill a width-sized dot rect at integer grid point | L62-70>
function dotRect(data, W, H, x, y, width) {
  // Centered on the point (matching PIL's even-width stroke placement
  // around the curve vertex, e.g. arc bottom rows y-1..y for width 2).
  const o = Math.floor(width / 2);
  for (let dy = -o; dy < width - o; dy++) {
    for (let dx = -o; dx < width - o; dx++) {
      const px = x + dx;
      const py = y + dy;
      if (px >= 0 && py >= 0 && px < W && py < H) data[py * W + px] = 1;
    }
  }
}

// <META - ROLE : Arc stroke mask matching PIL draw.arc | L48-68>
export function arcMask(cx, cy, rx, ry, startDeg, endDeg, width, w, h) {
  assertMaskSize(w, h);
  const data = new Uint8Array(w * h);
  strokeOpenPath(data, w, h, arcEllipsePoints(cx, cy, rx, ry, startDeg, endDeg), width);
  clipAngleRange(data, w, h, cx, cy, startDeg, endDeg);
  return { w, h, data };
}

// <META - ROLE : Clear stroke pixels outside the PIL angle range | L70-88>
//
// PIL clips the arc stroke to its angle sector (pixels past the start/end
// angles are not painted: butt-cap behavior). Angles use the PIL convention
// on integer pixel coords. The chord closing line is NOT part of the arc
// sector and must be stroked separately (see chordOutlineMask).
export function clipAngleRange(data, W, H, cx, cy, startDeg, endDeg) {
  let end = endDeg;
  while (end < startDeg) end += 360;
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      if (data[y * W + x] !== 1) continue;
      let a = Math.atan2(y - cy, x - cx) / DEG;
      if (a < 0) a += 360;
      if (a < startDeg) a += 360;
      if (a < startDeg || a > end) data[y * W + x] = 0;
    }
  }
}

// <META - ROLE : Chord fill mask matching PIL draw.chord fill | L59-72>
export function chordFillMask(cx, cy, rx, ry, startDeg, endDeg, w, h) {
  assertMaskSize(w, h);
  // Integer-snapped samples: the scanline engine assumes integer grid
  // coords (near-horizontal float edges iterate non-integer rows and paint
  // nothing). Consecutive duplicates are dropped so the vertex-adjustment
  // path sees a clean integer polygon. Reuses the proven polygon fill path.
  const pts = [];
  for (const [x, y] of arcEllipsePoints(cx, cy, rx, ry, startDeg, endDeg)) {
    const q = [Math.round(x), Math.round(y)];
    const l = pts[pts.length - 1];
    if (!l || l[0] !== q[0] || l[1] !== q[1]) pts.push(q);
  }
  return polygonMask(pts, w, h);
}

// <META - ROLE : Chord outline mask (closed path stroke) | L70-78>
export function chordOutlineMask(cx, cy, rx, ry, startDeg, endDeg, width, w, h) {
  assertMaskSize(w, h);
  const data = new Uint8Array(w * h);
  const pts = arcEllipsePoints(cx, cy, rx, ry, startDeg, endDeg);
  strokeOpenPath(data, w, h, pts, width);
  clipAngleRange(data, w, h, cx, cy, startDeg, endDeg);
  // The chord closing line is outside the arc sector: stroke it unclipped.
  strokeOpenPath(data, w, h, [pts[0], pts[pts.length - 1]], width);
  return { w, h, data };
}
