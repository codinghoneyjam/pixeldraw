// NON-NORMATIVE reference port of the oracle (tools/gen_fixtures.py). Tests in fixtures/ are normative.
// Copy freely; keep behaviour identical. Masks are arrays of rows of booleans.
export const rnd = (v) => Math.floor(v + 0.5);
const grid = (w, h, f) => Array.from({ length: h }, (_, y) => Array.from({ length: w }, (_, x) => f(x, y)));

export function brushMask(n) {
  return grid(n, n, (x, y) => (2 * x + 1 - n) ** 2 + (2 * y + 1 - n) ** 2 <= n * n);
}

export function ellipseMask(w, h) {
  const m = grid(w, h, (x, y) => (2 * x + 1 - w) ** 2 * h * h + (2 * y + 1 - h) ** 2 * w * w <= w * w * h * h);
  const xs = [...new Set([Math.floor((w - 1) / 2), Math.floor(w / 2)])];
  const ys = [...new Set([Math.floor((h - 1) / 2), Math.floor(h / 2)])];
  for (let y = 0; y < h; y++) if (!m[y].some(Boolean)) for (const x of xs) m[y][x] = true;
  for (let x = 0; x < w; x++) {
    let any = false;
    for (let y = 0; y < h; y++) if (m[y][x]) { any = true; break; }
    if (!any) for (const y of ys) m[y][x] = true;
  }
  return m;
}

export function rrectMask(w, h, r) {
  r = Math.max(0, Math.min(r, Math.floor(w / 2), Math.floor(h / 2)));
  if (r === 0) return grid(w, h, () => true);
  return grid(w, h, (x, y) => {
    const X = 2 * x + 1, Y = 2 * y + 1;
    const cx = X < 2 * r ? 2 * r : X > 2 * (w - r) ? 2 * (w - r) : null;
    const cy = Y < 2 * r ? 2 * r : Y > 2 * (h - r) ? 2 * (h - r) : null;
    return cx !== null && cy !== null ? (X - cx) ** 2 + (Y - cy) ** 2 <= 4 * r * r : true;
  });
}

export function outlineRing(kind, w, h, r, n) {
  const outer = kind === "ellipse" ? ellipseMask(w, h) : rrectMask(w, h, r);
  const iw = w - 2 * n, ih = h - 2 * n;
  const inner = grid(w, h, () => false);
  if (iw >= 1 && ih >= 1) {
    const rr = Math.max(Math.min(r, Math.floor(w / 2), Math.floor(h / 2)) - n, 0);
    const im = kind === "ellipse" ? ellipseMask(iw, ih) : rrectMask(iw, ih, rr);
    for (let y = 0; y < ih; y++) for (let x = 0; x < iw; x++) inner[y + n][x + n] = im[y][x];
  }
  return grid(w, h, (x, y) => outer[y][x] && !inner[y][x]);
}

export function bresenham(x0, y0, x1, y1) {
  const pts = [];
  const dx = Math.abs(x1 - x0), sx = x0 < x1 ? 1 : -1;
  const dy = -Math.abs(y1 - y0), sy = y0 < y1 ? 1 : -1;
  let err = dx + dy;
  for (;;) {
    pts.push([x0, y0]);
    if (x0 === x1 && y0 === y1) return pts;
    const e2 = 2 * err;
    if (e2 >= dy) { err += dy; x0 += sx; }
    if (e2 <= dx) { err += dx; y0 += sy; }
  }
}

export function strokeMask(points, n, W, H) {
  const m = grid(W, H, () => false), b = brushMask(n), off = Math.floor((n - 1) / 2);
  let path = points.length === 1 ? [points[0]] : [];
  for (let i = 1; i < points.length; i++) path = path.concat(bresenham(...points[i - 1], ...points[i]));
  for (const [px, py] of path)
    for (let by = 0; by < n; by++) for (let bx = 0; bx < n; bx++) {
      if (!b[by][bx]) continue;
      const x = px - off + bx, y = py - off + by;
      if (x >= 0 && x < W && y >= 0 && y < H) m[y][x] = true;
    }
  return m;
}

export function angleSnap(x0, y0, x1, y1, step = 15) {
  const dx = x1 - x0, dy = y1 - y0;
  if (dx === 0 && dy === 0) return [x1, y1];
  const st = (step * Math.PI) / 180, s = rnd(Math.atan2(dy, dx) / st) * st, L = Math.hypot(dx, dy);
  return [x0 + rnd(L * Math.cos(s)), y0 + rnd(L * Math.sin(s))];
}

export function resizeBBox(b, handle, p, lock = false, center = false, minsz = 1) {
  const [x, y, w, h] = b;
  const hx = handle.includes("e") ? 1 : handle.includes("w") ? -1 : 0;
  const hy = handle.includes("s") ? 1 : handle.includes("n") ? -1 : 0;
  const axis = (pos, size, hd, pv) => {
    if (hd === 0) return [pos, size];
    if (center) {
      const delta = hd === 1 ? pv - (pos + size - 1) : pos - pv;
      let ns = size + 2 * delta;
      if (ns < minsz) ns = minsz;
      return [ns === minsz && delta < 0 ? pos + Math.floor((size - ns) / 2) : pos - delta, ns];
    }
    const anchor = hd === 1 ? pos : pos + size - 1;
    const lo = Math.min(anchor, pv), hi = Math.max(anchor, pv);
    return [lo, hi - lo + 1];
  };
  let [nx, nw] = axis(x, w, hx, p[0]);
  let [ny, nh] = axis(y, h, hy, p[1]);
  if (lock && (hx || hy)) {
    if (hx && hy) { if (nw * h > nh * w) nh = Math.max(minsz, rnd((nw * h) / w)); else nw = Math.max(minsz, rnd((nh * w) / h)); }
    else if (hx) nh = Math.max(minsz, rnd((nw * h) / w));
    else nw = Math.max(minsz, rnd((nh * w) / h));
    const place = (pos, size, hd, pv, ns) => {
      if (center || hd === 0) return pos + Math.floor((size - ns) / 2);
      const anchor = hd === 1 ? pos : pos + size - 1;
      return pv >= anchor ? anchor : anchor - ns + 1;
    };
    nx = place(x, w, hx, p[0], nw);
    ny = place(y, h, hy, p[1], nh);
  }
  return [nx, ny, nw, nh];
}

export function over(dst, src, opacity) {
  const sa = (src[3] / 255) * opacity;
  if (sa === 0) return [...dst];
  const da = dst[3] / 255, oa = sa + da * (1 - sa);
  const ch = [0, 1, 2].map((i) => rnd((src[i] * sa + dst[i] * da * (1 - sa)) / oa));
  const a = rnd(oa * 255);
  return a === 0 ? [0, 0, 0, 0] : [...ch, a];
}

export function flood(g, x, y, nv) {
  const H = g.length, W = g[0].length, out = g.map((r) => r.slice()), old = out[y][x];
  if (old === nv) return out;
  const st = [[x, y]];
  while (st.length) {
    const [cx, cy] = st.pop();
    if (cx < 0 || cy < 0 || cx >= W || cy >= H || out[cy][cx] !== old) continue;
    out[cy][cx] = nv;
    st.push([cx + 1, cy], [cx - 1, cy], [cx, cy + 1], [cx, cy - 1]);
  }
  return out;
}

export function dragBBox(p0, p1, lock = false, center = false) {
  const dx = p1[0] - p0[0], dy = p1[1] - p0[1], adx = Math.abs(dx), ady = Math.abs(dy);
  if (center) {
    let w = 2 * adx + 1, h = 2 * ady + 1;
    if (lock) w = h = Math.max(w, h);
    return [p0[0] - Math.floor((w - 1) / 2), p0[1] - Math.floor((h - 1) / 2), w, h];
  }
  let w = adx + 1, h = ady + 1;
  if (lock) w = h = Math.max(w, h);
  return [dx >= 0 ? p0[0] : p0[0] - w + 1, dy >= 0 ? p0[1] : p0[1] - h + 1, w, h];
}

export function unitSnap([x, y, w, h]) {
  return [rnd(x / 32) * 32, rnd(y / 32) * 32, Math.max(32, rnd(w / 32) * 32), Math.max(32, rnd(h / 32) * 32)];
}

// ---------------------------------------------------------------------------
// PIL raster port (Draw.c) + polygon/fillet/gradient/arc. Bool-grid flavor.
// Mirrors tools/gen_fixtures.py polygon_mask / polygon_outline_mask /
// fillet_polygon / radial_gradient_mask / arc_mask / chord_*_mask.
// ---------------------------------------------------------------------------
const roundUpR = (f) => (f >= 0 ? Math.floor(f + 0.5) : -Math.floor(Math.abs(f) + 0.5));
const roundDownR = (f) => (f >= 0 ? Math.ceil(f - 0.5) : -Math.ceil(Math.abs(f) - 0.5));
const roundCR = (f) => (f >= 0 ? Math.floor(f + 0.5) : -Math.floor(Math.abs(f) + 0.5));

function addEdge(e, x0, y0, x1, y1) {
  if (x0 <= x1) { e.xmin = x0; e.xmax = x1; } else { e.xmin = x1; e.xmax = x0; }
  if (y0 <= y1) { e.ymin = y0; e.ymax = y1; } else { e.ymin = y1; e.ymax = y0; }
  if (y0 === y1) { e.d = 0; e.dx = 0.0; } else { e.dx = Math.fround((x1 - x0) / (y1 - y0)); e.d = y0 === e.ymin ? 1 : -1; }
  e.x0 = x0; e.y0 = y0;
}

function buildEdges(pts) {
  const e = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const [x0, y0] = pts[i], [x1, y1] = pts[i + 1];
    if (y0 === y1 && i !== 0 && y0 === pts[i - 1][1]) {
      const last = e[e.length - 1];
      if (x1 > x0 && x0 > pts[i - 1][0]) { last.xmax = x1; continue; }
      if (x1 < x0 && x0 < pts[i - 1][0]) { last.xmin = x1; continue; }
    }
    const ed = {};
    addEdge(ed, x0, y0, x1, y1);
    e.push(ed);
  }
  const last = pts[pts.length - 1], first = pts[0];
  if (last[0] !== first[0] || last[1] !== first[1]) {
    const ed = {};
    addEdge(ed, last[0], last[1], first[0], first[1]);
    e.push(ed);
  }
  return e;
}

function hline(data, W, H, x0, y, x1, val) {
  if (y < 0 || y >= H) return;
  if (x0 < 0) x0 = 0; else if (x0 >= W) return;
  if (x1 < 0) return; else if (x1 >= W) x1 = W - 1;
  if (x0 <= x1) for (let x = x0; x <= x1; x++) data[y * W + x] = val;
}

function polyGeneric(data, W, H, edges) {
  const table = [];
  let ymin = H - 1, ymax = 0;
  for (const e of edges) {
    if (e.ymin === e.ymax) hline(data, W, H, e.xmin, e.ymin, e.xmax, 1);
    else table.push(e);
  }
  for (const e of edges) { if (ymin > e.ymin) ymin = e.ymin; if (ymax < e.ymax) ymax = e.ymax; }
  if (ymin < 0) ymin = 0;
  if (ymax > H) ymax = H;
  const xx = new Float32Array(table.length * 2);
  for (let y = ymin; y <= ymax; y++) {
    let j = 0;
    for (let i = 0; i < table.length; i++) {
      const cur = table[i];
      if (y >= cur.ymin && y <= cur.ymax) {
        xx[j++] = Math.fround(Math.fround(Math.fround(y - cur.y0) * cur.dx) + cur.x0);
        if (y === cur.ymax && y < ymax) { xx[j] = xx[j - 1]; j++; }
        else if ((y === cur.ymin || y === cur.ymax) && cur.dx !== 0) {
          for (let k = 0; k < i; k++) {
            const other = table[k];
            if ((y !== other.ymin && y !== other.ymax) || other.dx === 0) continue;
            if (roundCR(xx[j - 1]) === roundCR(Math.fround(Math.fround(Math.fround(y - other.y0) * other.dx) + other.x0))) {
              const offset = y === cur.ymax ? -1 : 1;
              const adj = Math.fround(Math.fround(Math.fround(y + offset - cur.y0) * cur.dx) + cur.x0);
              if (y + offset >= other.ymin && y + offset <= other.ymax) {
                const adjO = Math.fround(Math.fround(Math.fround(y + offset - other.y0) * other.dx) + other.x0);
                if (xx[j - 1] > adj + 1 && xx[j - 1] > adjO + 1) xx[j - 1] = roundCR(Math.max(adj, adjO)) + 1;
                else if (xx[j - 1] < adj - 1 && xx[j - 1] < adjO - 1) xx[j - 1] = roundCR(Math.min(adj, adjO)) - 1;
                break;
              }
            }
          }
        }
      }
    }
    const vals = Array.from(xx.slice(0, j)).sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
    for (let i = 1; i < j; i += 2) hline(data, W, H, roundUpR(vals[i - 1]), y, roundDownR(vals[i]), 1);
  }
}

function line8R(data, W, H, x0, y0, x1, y1, val) {
  let dx = x1 - x0, xs, dy = y1 - y0, ys;
  if (dx < 0) { dx = -dx; xs = -1; } else xs = 1;
  if (dy < 0) { dy = -dy; ys = -1; } else ys = 1;
  let n = dx > dy ? dx : dy;
  const plot = (x, y) => { if (x >= 0 && x < W && y >= 0 && y < H) data[y * W + x] = val; };
  if (dx === 0) { for (let i = 0; i < dy; i++) { plot(x0, y0); y0 += ys; } }
  else if (dy === 0) { for (let i = 0; i < dx; i++) { plot(x0, y0); x0 += xs; } }
  else if (dx > dy) {
    n = dx; dy += dy; let e = dy - dx; dx += dx;
    for (let i = 0; i < n; i++) { plot(x0, y0); if (e >= 0) { y0 += ys; e -= dx; } e += dy; x0 += xs; }
  } else {
    n = dy; dx += dx; let e = dx - dy; dy += dy;
    for (let i = 0; i < n; i++) { plot(x0, y0); if (e >= 0) { x0 += xs; e -= dy; } e += dx; y0 += ys; }
  }
}

function wideQuadEdges(x0, y0, x1, y1, width) {
  const dx = x1 - x0, dy = y1 - y0;
  const big = Math.hypot(dx, dy);
  const small = (width - 1) / 2.0;
  const ratioMax = roundUpR(small) / big, ratioMin = roundDownR(small) / big;
  const dxmin = roundDownR(ratioMin * dy), dxmax = roundDownR(ratioMax * dy);
  const dymin = roundDownR(ratioMin * dx), dymax = roundDownR(ratioMax * dx);
  const v = [
    [x0 - dxmin, y0 + dymax], [x1 - dxmin, y1 + dymax],
    [x1 + dxmax, y1 - dymin], [x0 + dxmax, y0 - dymin],
  ];
  const e = [];
  for (let i = 0; i < 4; i++) {
    const a = v[i], b = v[(i + 1) % 4];
    const ed = {}; addEdge(ed, a[0], a[1], b[0], b[1]); e.push(ed);
  }
  return e;
}

const toGrid = (data, w, h) => Array.from({ length: h }, (_, y) => Array.from({ length: w }, (_, x) => data[y * w + x] === 1));

export function polygonMask(pts, w, h) {
  const data = new Uint8Array(w * h);
  polyGeneric(data, w, h, buildEdges(pts.map((p) => [p[0], p[1]])));
  return toGrid(data, w, h);
}

export function polygonOutlineMask(pts, w, width) {
  const data = new Uint8Array(w * w);
  const local = pts.map((p) => [p[0], p[1]]);
  const path = [...local, local[0]];
  for (let i = 0; i < path.length - 1; i++) {
    const [x0, y0] = path[i], [x1, y1] = path[i + 1];
    if (width <= 1) line8R(data, w, w, x0, y0, x1, y1, 1);
    else polyGeneric(data, w, w, wideQuadEdges(x0, y0, x1, y1, width));
  }
  return toGrid(data, w, w);
}

// Python round(): half to even, unlike Math.round.
function pyRoundR(v) {
  const f = Math.floor(v);
  const r = v - f;
  if (r > 0.5) return f + 1;
  if (r < 0.5) return f;
  return f % 2 === 0 ? f : f + 1;
}

export function filletPolygon(pts, radius, steps = 8) {
  const local = pts.map((p) => [p[0], p[1]]);
  if (radius <= 0.0 || local.length < 3) return local;
  const n = local.length;
  const newPts = [];
  for (let i = 0; i < n; i++) {
    const pPrev = local[(i - 1 + n) % n], pCurr = local[i], pNext = local[(i + 1) % n];
    const v1x = pPrev[0] - pCurr[0], v1y = pPrev[1] - pCurr[1];
    const v2x = pNext[0] - pCurr[0], v2y = pNext[1] - pCurr[1];
    const l1 = Math.hypot(v1x, v1y), l2 = Math.hypot(v2x, v2y);
    if (l1 === 0 || l2 === 0) { newPts.push([...pCurr]); continue; }
    const u1x = v1x / l1, u1y = v1y / l1, u2x = v2x / l2, u2y = v2y / l2;
    const dot = Math.max(-0.999, Math.min(0.999, u1x * u2x + u1y * u2y));
    const halfAngle = Math.acos(dot) / 2.0;
    const tanHalf = Math.tan(halfAngle);
    if (tanHalf < 0.001) { newPts.push([...pCurr]); continue; }
    const maxR = Math.min(radius, l1 * 0.45 * tanHalf, l2 * 0.45 * tanHalf);
    const tangentLen = maxR / tanHalf;
    const pStartX = pCurr[0] + u1x * tangentLen, pStartY = pCurr[1] + u1y * tangentLen;
    const pEndX = pCurr[0] + u2x * tangentLen, pEndY = pCurr[1] + u2y * tangentLen;
    const bisX = u1x + u2x, bisY = u1y + u2y;
    const bLen = Math.hypot(bisX, bisY);
    if (bLen < 0.001) { newPts.push([...pCurr]); continue; }
    const wX = bisX / bLen, wY = bisY / bLen;
    const centerDist = maxR / Math.sin(halfAngle);
    const cx = pCurr[0] + wX * centerDist, cy = pCurr[1] + wY * centerDist;
    const aStart = Math.atan2(pStartY - cy, pStartX - cx);
    const aEnd = Math.atan2(pEndY - cy, pEndX - cx);
    const diff = ((aEnd - aStart + Math.PI + 2.0 * Math.PI) % (2.0 * Math.PI)) - Math.PI;
    for (let s = 0; s <= steps; s++) {
      const ang = aStart + diff * (s / steps);
      newPts.push([pyRoundR(cx + maxR * Math.cos(ang)), pyRoundR(cy + maxR * Math.sin(ang))]);
    }
  }
  return newPts;
}

const gradTruncR = (v) => Math.floor(v);
const clamp01R = (t) => (t < 0 ? 0 : t > 1 ? 1 : t);

function sampleStopsR(sorted, t) {
  const first = sorted[0], last = sorted[sorted.length - 1];
  if (t <= first[0]) return [first[1], first[2], first[3], first[4]];
  if (t >= last[0]) return [last[1], last[2], last[3], last[4]];
  for (let i = 0; i < sorted.length - 1; i++) {
    const a = sorted[i], b = sorted[i + 1];
    if (t < a[0] || t > b[0]) continue;
    const f = b[0] === a[0] ? 0 : (t - a[0]) / (b[0] - a[0]);
    return [gradTruncR(a[1] + (b[1] - a[1]) * f), gradTruncR(a[2] + (b[2] - a[2]) * f),
      gradTruncR(a[3] + (b[3] - a[3]) * f), gradTruncR(a[4] + (b[4] - a[4]) * f)];
  }
  return [last[1], last[2], last[3], last[4]];
}

export function radialGradientMask(cx, cy, r0, r1, stops, w, h) {
  const sorted = stops.map((s) => [s[0], s[1], s[2], s[3], s[4]]);
  sorted.sort((a, b) => a[0] - b[0]);
  const span = r1 - r0;
  const out = [];
  for (let y = 0; y < h; y++) {
    const row = [];
    for (let x = 0; x < w; x++) {
      const d = Math.hypot(x - cx, y - cy);
      const t = span <= 0 ? (d <= r0 ? 0 : 1) : clamp01R((d - r0) / span);
      const c = sampleStopsR(sorted, t);
      row.push(c[3] > 0 ? c : [0, 0, 0, 0]);
    }
    out.push(row);
  }
  return out;
}

const DEGR = Math.PI / 180;

export function arcEllipsePoints(cx, cy, rx, ry, startDeg, endDeg, stepDeg = 0.5) {
  let end = endDeg;
  while (end < startDeg) end += 360;
  const span = end - startDeg;
  const steps = Math.max(1, Math.ceil(span / stepDeg));
  const pts = [];
  for (let i = 0; i <= steps; i++) {
    const a = (startDeg + (span * i) / steps) * DEGR;
    pts.push([cx + rx * Math.cos(a), cy + ry * Math.sin(a)]);
  }
  return pts;
}

function dotRectRef(data, W, H, x, y, width) {
  const o = Math.floor(width / 2);
  for (let dy = -o; dy < width - o; dy++) {
    for (let dx = -o; dx < width - o; dx++) {
      const px = x + dx, py = y + dy;
      if (px >= 0 && py >= 0 && px < W && py < H) data[py * W + px] = 1;
    }
  }
}

function strokeOpenPathRef(data, W, H, pts, width) {
  for (let i = 0; i < pts.length - 1; i++) {
    const x0 = Math.round(pts[i][0]), y0 = Math.round(pts[i][1]);
    const x1 = Math.round(pts[i + 1][0]), y1 = Math.round(pts[i + 1][1]);
    if (x0 === x1 && y0 === y1) { dotRectRef(data, W, H, x0, y0, width); continue; }
    if (width <= 1) line8R(data, W, H, x0, y0, x1, y1, 1);
    else polyGeneric(data, W, H, wideQuadEdges(x0, y0, x1, y1, width));
  }
}

function clipAngleRef(data, W, H, cx, cy, startDeg, endDeg) {
  let end = endDeg;
  while (end < startDeg) end += 360;
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      if (data[y * W + x] !== 1) continue;
      let a = Math.atan2(y - cy, x - cx) / DEGR;
      if (a < 0) a += 360;
      if (a < startDeg) a += 360;
      if (a < startDeg || a > end) data[y * W + x] = 0;
    }
  }
}

const boxArgs = (box) => [
  (box[0][0] + box[1][0]) / 2, (box[0][1] + box[1][1]) / 2,
  (box[1][0] - box[0][0]) / 2, (box[1][1] - box[0][1]) / 2,
];

export function arcMask(box, start, end, width, w, h) {
  const [cx, cy, rx, ry] = boxArgs(box);
  const data = new Uint8Array(w * h);
  strokeOpenPathRef(data, w, h, arcEllipsePoints(cx, cy, rx, ry, start, end), width);
  clipAngleRef(data, w, h, cx, cy, start, end);
  return toGrid(data, w, h);
}

export function chordFillMask(box, start, end, w, h) {
  const [cx, cy, rx, ry] = boxArgs(box);
  const pts = [];
  for (const [x, y] of arcEllipsePoints(cx, cy, rx, ry, start, end)) {
    const q = [Math.round(x), Math.round(y)];
    const l = pts[pts.length - 1];
    if (!l || l[0] !== q[0] || l[1] !== q[1]) pts.push(q);
  }
  return polygonMask(pts, w, h);
}

export function chordOutlineMask(box, start, end, width, w, h) {
  const [cx, cy, rx, ry] = boxArgs(box);
  const data = new Uint8Array(w * h);
  const pts = arcEllipsePoints(cx, cy, rx, ry, start, end);
  strokeOpenPathRef(data, w, h, pts, width);
  clipAngleRef(data, w, h, cx, cy, start, end);
  strokeOpenPathRef(data, w, h, [pts[0], pts[pts.length - 1]], width);
  return toGrid(data, w, h);
}
