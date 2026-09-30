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
