// Verifies reference/raster_ref.mjs against fixtures/raster_golden.json. Run: node tools/parity_check.mjs
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import * as R from "../reference/raster_ref.mjs";

const G = JSON.parse(readFileSync(new URL("../tests/fixtures/raster_golden.json", import.meta.url)));
const rows = (m) => m.map((r) => r.map((v) => (v ? "#" : ".")).join(""));
const sha = (m) => createHash("sha256").update(rows(m).join("\n")).digest("hex");
let n = 0;
const eq = (a, b, msg) => { assert.deepEqual(a, b, msg); n++; };

for (const c of G.brush) eq(rows(R.brushMask(c.n)), c.rows, `brush ${c.n}`);
for (const c of G.ellipse) eq(rows(R.ellipseMask(c.w, c.h)), c.rows, `ellipse ${c.w}x${c.h}`);
for (const c of G.rrect) eq(rows(R.rrectMask(c.w, c.h, c.r)), c.rows, `rrect ${c.w}x${c.h} r${c.r}`);
for (const c of G.outline) eq(rows(R.outlineRing(c.kind, c.w, c.h, c.r, c.n)), c.rows, `outline ${c.kind} ${c.w}x${c.h}`);
for (const c of G.bresenham) eq(R.bresenham(...c.p0, ...c.p1), c.points, `bresenham ${c.p0}->${c.p1}`);
for (const c of G.stroke) eq(rows(R.strokeMask(c.points, c.n, c.W, c.H)), c.rows, `stroke n=${c.n}`);
for (const c of G.angle_snap) eq(R.angleSnap(...c.p0, ...c.p1), c.expect, `angle ${c.p1}`);
for (const c of G.resize) eq(R.resizeBBox(c.bbox, c.handle, c.pointer, c.lock, c.center), c.expect, `resize ${c.handle} ${c.pointer}`);
for (const c of G.drag_bbox) eq(R.dragBBox(c.p0, c.p1, c.lock, c.center), c.expect, `drag_bbox ${c.p0}->${c.p1}`);
for (const c of G.unit_snap) eq(R.unitSnap(c.bbox), c.expect, `unit_snap ${c.bbox}`);
for (const c of G.composite) eq(R.over(c.dst, c.src, c.opacity), c.expect, `composite ${JSON.stringify(c.src)}`);
for (const c of G.fill) eq(R.flood(c.grid, c.x, c.y, c.new), c.expect, `fill ${c.x},${c.y}`);
const big = { "brush 64": R.brushMask(64), "brush 33": R.brushMask(33), "ellipse 128x64": R.ellipseMask(128, 64),
  "rrect 200x120 r30": R.rrectMask(200, 120, 30), "outline ellipse 128x64 n8": R.outlineRing("ellipse", 128, 64, 0, 8),
  "outline rrect 200x120 r30 n6": R.outlineRing("rrect", 200, 120, 30, 6) };
for (const c of G.hashes) eq(sha(big[c.what]), c.sha256, `hash ${c.what}`);
console.log(`parity OK: ${n} golden checks passed (JS reference == Python oracle)`);
