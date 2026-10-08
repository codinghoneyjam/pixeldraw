// <META - FILE SUMMARY - Pure shape geometry: handles, hit tests, drag/resize/move>
import { angleSnap, dragBBox, resizeBBox, rnd, unitSnap } from "../../core/raster/raster_snap.js";
import { DrawToolError } from "../../core/errors.js";

// <META - ROLE : integer field guard | L4-8>
function asInt(v, what) {
  if (!Number.isInteger(v)) throw new DrawToolError("OUT_OF_RANGE", `invalid ${what}`);
  return v;
}

// <META - ROLE : handle ids in fixed order | L5-6>
export const HANDLE_IDS = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];

// <META - ROLE : point-to-segment distance for line grab tests | L8-17>
export function distSeg(px, py, a, b) {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len2 = dx * dx + dy * dy;
  let t = len2 === 0 ? 0 : ((px - a.x) * dx + (py - a.y) * dy) / len2;
  t = Math.min(1, Math.max(0, t));
  return Math.hypot(px - (a.x + t * dx), py - (a.y + t * dy));
}

// <META - ROLE : handle layout for bbox kinds, line endpoints, and polygon vertices | L19-36>
export function layoutHandles(kind, pending) {
  if (kind === "line") {
    return [
      { id: "p0", end: 0, x: pending.p0.x, y: pending.p0.y },
      { id: "p1", end: 1, x: pending.p1.x, y: pending.p1.y },
    ];
  }
  if (kind === "polygon") {
    return pending.points.map((p, i) => ({ id: `v${i}`, x: p.x, y: p.y }));
  }
  const { x, y, w, h } = pending.bbox;
  const x1 = x + w - 1;
  const y1 = y + h - 1;
  const xm = (x + x1) / 2;
  const ym = (y + y1) / 2;
  const pts = { nw: [x, y], n: [xm, y], ne: [x1, y], e: [x1, ym], se: [x1, y1], s: [xm, y1], sw: [x, y1], w: [x, ym] };
  return HANDLE_IDS.map((id) => ({ id, x: pts[id][0], y: pts[id][1] }));
}

// <META - ROLE : handle hit test within canvas-px tolerance | L32-38>
export function hitHandle(kind, pending, fx, fy, tol) {
  for (const h of layoutHandles(kind, pending)) {
    if (Math.abs(fx - h.x) <= tol && Math.abs(fy - h.y) <= tol) return h;
  }
  return null;
}

// <META - ROLE : even-odd point-in-polygon for integer or float test points | L40-56>
export function pointInPolygon(pts, fx, fy) {
  let inside = false;
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const xi = pts[i].x, yi = pts[i].y;
    const xj = pts[j].x, yj = pts[j].y;
    if ((yi > fy) !== (yj > fy) && fx < ((xj - xi) * (fy - yi)) / (yj - yi) + xi) {
      inside = !inside;
    }
  }
  return inside;
}

// <META - ROLE : inside test for bbox kinds, line grab width, and polygon fill/edge | L58-72>
export function insideShape(kind, pending, fx, fy, grab) {
  if (kind === "line") return distSeg(fx, fy, pending.p0, pending.p1) <= grab;
  if (kind === "polygon") {
    const pts = pending.points;
    if (pointInPolygon(pts, fx, fy)) return true;
    for (let i = 0; i < pts.length; i++) {
      if (distSeg(fx, fy, pts[i], pts[(i + 1) % pts.length]) <= grab) return true;
    }
    return false;
  }
  const { x, y, w, h } = pending.bbox;
  return fx >= x && fx <= x + w - 1 && fy >= y && fy <= y + h - 1;
}

// <META - ROLE : fresh-draw geometry from a drag vector | L50-60>
export function dragGeom(kind, p0, p1, flags, shapeRadius) {
  if (kind === "line") {
    const p = flags.shift ? angleSnap(p0.x, p0.y, p1.x, p1.y) : [p1.x, p1.y];
    return { bbox: null, p0: { ...p0 }, p1: { x: p[0], y: p[1] }, radius: 0 };
  }
  let b = dragBBox(p0, p1, flags.lock, flags.center);
  if (flags.snap) b = unitSnap(b);
  return { bbox: { x: b[0], y: b[1], w: b[2], h: b[3] }, p0: null, p1: null, radius: shapeRadius };
}

// <META - ROLE : pending resize from a handle drag | L62-84>
export function resizeGeom(kind, orig, handleId, end, p, flags, shiftSnap) {
  if (kind === "line") {
    const fixed = end === 0 ? orig.p1 : orig.p0;
    const np = shiftSnap ? angleSnap(fixed.x, fixed.y, p.x, p.y) : [p.x, p.y];
    const named = { x: np[0], y: np[1] };
    return end === 0 ? { ...orig, p0: named } : { ...orig, p1: named };
  }
  if (kind === "polygon") {
    const idx = Number(String(handleId).slice(1));
    if (!Number.isInteger(idx) || idx < 0 || idx >= orig.points.length) return orig;
    const np = flags.snap ? { x: rnd(p.x / 32) * 32, y: rnd(p.y / 32) * 32 } : { x: p.x, y: p.y };
    const points = orig.points.map((q, i) => (i === idx ? np : { ...q }));
    return { ...orig, points };
  }
  let b = resizeBBox([orig.bbox.x, orig.bbox.y, orig.bbox.w, orig.bbox.h], handleId, [p.x, p.y], flags.lock, flags.center);
  if (flags.snap) b = unitSnap(b);
  return { ...orig, bbox: { x: b[0], y: b[1], w: b[2], h: b[3] } };
}

// <META - ROLE : pending move by integer delta with optional unit snap | L86-96>
export function moveGeom(kind, orig, dx, dy, snap) {
  if (kind === "line") {
    const sh = (q) => (snap ? { x: rnd((q.x + dx) / 32) * 32, y: rnd((q.y + dy) / 32) * 32 } : { x: q.x + dx, y: q.y + dy });
    return { ...orig, p0: sh(orig.p0), p1: sh(orig.p1) };
  }
  if (kind === "polygon") {
    const sh = (q) => (snap ? { x: rnd((q.x + dx) / 32) * 32, y: rnd((q.y + dy) / 32) * 32 } : { x: q.x + dx, y: q.y + dy });
    return { ...orig, points: orig.points.map(sh) };
  }
  const nx = snap ? rnd((orig.bbox.x + dx) / 32) * 32 : orig.bbox.x + dx;
  const ny = snap ? rnd((orig.bbox.y + dy) / 32) * 32 : orig.bbox.y + dy;
  return { ...orig, bbox: { ...orig.bbox, x: nx, y: ny } };
}

// <META - ROLE : validated option-bar geometry for line pending | L88-104>
export function parseLineGeom(v) {
  if (v !== null && typeof v === "object" && "x0" in v) {
    return {
      bbox: null,
      p0: { x: asInt(v.x0, "x0"), y: asInt(v.y0, "y0") },
      p1: { x: asInt(v.x1, "x1"), y: asInt(v.y1, "y1") },
      radius: 0,
    };
  }
  const x = asInt(v.x, "x");
  const y = asInt(v.y, "y");
  const w = asInt(v.w, "w");
  const h = asInt(v.h, "h");
  if (w < 1 || h < 1) throw new DrawToolError("OUT_OF_RANGE", "line extents must be >= 1");
  return { bbox: null, p0: { x, y }, p1: { x: x + w - 1, y: y + h - 1 }, radius: 0 };
}

// <META - ROLE : validated option-bar geometry for polygon pending | L106-120>
export function parsePolygonGeom(v) {
  if (v === null || typeof v !== "object" || !Array.isArray(v.points)) {
    throw new DrawToolError("OUT_OF_RANGE", "polygon needs points");
  }
  if (v.points.length < 3) throw new DrawToolError("OUT_OF_RANGE", "polygon needs >= 3 points");
  const points = v.points.map((p, i) => {
    const q = Array.isArray(p) ? { x: p[0], y: p[1] } : p;
    if (!q || typeof q !== "object") throw new DrawToolError("OUT_OF_RANGE", `invalid polygon point ${i}`);
    return { x: asInt(q.x, `points[${i}].x`), y: asInt(q.y, `points[${i}].y`) };
  });
  return { bbox: null, p0: null, p1: null, points };
}

// <META - ROLE : snap one placement point to the 32px grid when snapping | L122-126>
export function snapPlacePoint(p, snap) {
  if (!snap) return { ...p };
  return { x: rnd(p.x / 32) * 32, y: rnd(p.y / 32) * 32 };
}
// <META - ROLE : validated option-bar geometry for bbox pending | L106-116>
export function parseBoxGeom(v, shapeRadius) {
  const x = asInt(v.x, "x");
  const y = asInt(v.y, "y");
  const w = asInt(v.w, "w");
  const h = asInt(v.h, "h");
  if (w < 1 || h < 1) throw new DrawToolError("OUT_OF_RANGE", "shape w,h must be >= 1");
  const r = v.radius === undefined ? shapeRadius : v.radius;
  if (!Number.isInteger(r) || r < 0) throw new DrawToolError("OUT_OF_RANGE", "radius must be >= 0");
  return { bbox: { x, y, w, h }, p0: null, p1: null, radius: r };
}

// <META - ROLE : commit-time ShapeSpec from pending geometry | L118-136>
export function buildSpec(kind, pending, settings) {
  if (kind === "line") return { kind: "line", p0: { ...pending.p0 }, p1: { ...pending.p1 }, strokeWidth: settings.penSize };
  if (kind === "polygon") {
    return {
      kind: "polygon",
      points: pending.points.map((p) => ({ x: p.x, y: p.y })),
      strokeWidth: settings.penSize,
      fillMode: settings.shapeFill,
    };
  }
  return {
    kind,
    bbox: { ...pending.bbox },
    radius: kind === "rrect" ? pending.radius : 0,
    strokeWidth: settings.penSize,
    fillMode: settings.shapeFill,
  };
}

// <META - ROLE : preview spec for an in-progress draw | L132-142>
export function drawSpec(kind, draw, settings) {
  const cur = draw.cur ?? draw.p0;
  const flags = { shift: draw.shift, lock: draw.shift || settings.shapeLockAspect, center: draw.alt, snap: settings.snapUnit };
  const g = dragGeom(kind, draw.p0, cur, flags, settings.shapeRadius);
  if (kind === "line") return { kind: "line", p0: g.p0, p1: g.p1, strokeWidth: settings.penSize };
  return { kind, bbox: g.bbox, radius: kind === "rrect" ? g.radius : 0, strokeWidth: settings.penSize, fillMode: settings.shapeFill };
}

// <META - ROLE : pending-like geometry back from a spec for handles | L144-152>
export function geomFromSpec(spec) {
  if (spec.kind === "line") return { bbox: null, p0: spec.p0, p1: spec.p1, radius: 0 };
  if (spec.kind === "polygon") return { bbox: null, p0: null, p1: null, points: spec.points.map((p) => ({ ...p })) };
  return { bbox: { ...spec.bbox }, p0: null, p1: null, radius: spec.radius ?? 0 };
}
