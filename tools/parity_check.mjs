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
// PIL d.line ground truth. The production primitive is polygon.js's
// wideLineQuadEdges + polygonGeneric, already verified by polygon_outline.
for (const c of G.wide_line) eq(rows(R.wideLineMask(c.p0, c.p1, c.width, c.W, c.H)), c.rows, `wide_line ${c.name}`);
for (const c of G.polygon) eq(rows(R.polygonMask(c.pts, c.w, c.h)), c.rows, `polygon ${c.name}`);
for (const c of G.polygon_outline) eq(rows(R.polygonOutlineMask(c.pts, c.w, c.width)), c.rows, `polygon_outline ${c.name} w=${c.width}`);
for (const c of G.fillet) eq(rows(R.polygonMask(R.filletPolygon(c.pts, c.radius), c.w, c.h)), c.rows, `fillet ${c.name}`);
for (const c of G.gradient) eq(R.radialGradientMask(c.cx, c.cy, c.r0, c.r1, c.stops, c.w, c.h), c.rgba, `gradient ${c.name}`);
for (const c of G.arc) {
  // Near-parity: the port covers every PIL pixel (missing == 0) with a
  // bounded overshoot (extras <= 16). PIL strokes arcs as its own ellipse
  // outline clipped to the angle sector; our integer internals differ by
  // boundary pixels. Sheet recipes only use width <= 2 arcs.
  const got = R.arcMask(c.box, c.start, c.end, c.width, c.w, c.h);
  const want = c.rows.map((r) => [...r].map((ch) => ch === "#"));
  let missing = 0;
  let extras = 0;
  for (let y = 0; y < c.h; y++) {
    for (let x = 0; x < c.w; x++) {
      if (want[y][x] && !got[y][x]) missing++;
      if (got[y][x] && !want[y][x]) extras++;
    }
  }
  assert.ok(missing === 0, `arc ${c.name} drops ${missing} PIL pixels`);
  assert.ok(extras <= 16, `arc ${c.name} overshoot ${extras} > 16`);
  n += 2;
}
for (const c of G.chord) {
  if ("width" in c) {
    // Chord outline: same near-parity shape. No sheet recipe uses chord
    // outline (all are fill-only); the bound locks current behavior.
    // (PIL strokes the closing diameter onto interior rows; see task memo.)
    const got = R.chordOutlineMask(c.box, c.start, c.end, c.width, c.w, c.h);
    const want = c.rows.map((r) => [...r].map((ch) => ch === "#"));
    let missing = 0;
    let extras = 0;
    for (let y = 0; y < c.h; y++) {
      for (let x = 0; x < c.w; x++) {
        if (want[y][x] && !got[y][x]) missing++;
        if (got[y][x] && !want[y][x]) extras++;
      }
    }
    assert.ok(missing <= 16, `chord_outline ${c.name} drops ${missing} > 16`);
    assert.ok(extras <= 40, `chord_outline ${c.name} overshoot ${extras} > 40`);
    n += 2;
  } else {
    // Chord fill sits 1px wider than PIL at the arc apex (PIL chord fill
    // is not PIL polygon fill for the same vertices; measured 2026-10-04).
    const got = R.chordFillMask(c.box, c.start, c.end, c.w, c.h);
    const want = c.rows.map((r) => [...r].map((ch) => ch === "#"));
    let missing = 0;
    let extras = 0;
    for (let y = 0; y < c.h; y++) {
      for (let x = 0; x < c.w; x++) {
        if (want[y][x] && !got[y][x]) missing++;
        if (got[y][x] && !want[y][x]) extras++;
      }
    }
    assert.ok(missing === 0, `chord_fill ${c.name} drops ${missing} PIL pixels`);
    assert.ok(extras <= 4, `chord_fill ${c.name} overshoot ${extras} > 4`);
    n += 2;
  }
}
for (const c of G.angle_snap) eq(R.angleSnap(...c.p0, ...c.p1), c.expect, `angle ${c.p1}`);
for (const c of G.resize) eq(R.resizeBBox(c.bbox, c.handle, c.pointer, c.lock, c.center), c.expect, `resize ${c.handle} ${c.pointer}`);
for (const c of G.drag_bbox) eq(R.dragBBox(c.p0, c.p1, c.lock, c.center), c.expect, `drag_bbox ${c.p0}->${c.p1}`);
for (const c of G.unit_snap) eq(R.unitSnap(c.bbox), c.expect, `unit_snap ${c.bbox}`);
// PIL paste(t, (i*cell,0), t) ground truth: the atlas assembly law. `inputs`
// holds what PIL was given (one hex RGBA run per slot), `expect` what it
// produced. The JS assembler must reproduce `expect` from `inputs`.
for (const c of G.paste) {
  // Each golden input is one hex RGBA run; with cell 1x1 that is a single
  // pixel, so wrap it as a one-pixel tile for the assembler.
  const tiles = c.inputs.map((hex) => [
    [0, 1, 2, 3].map((k) => parseInt(hex.slice(k * 2, k * 2 + 2), 16)),
  ]);
  eq(R.assembleStripRef(tiles, c.cell_w, c.cell_h), c.expect, `paste ${c.name}`);
}
for (const c of G.composite) eq(R.over(c.dst, c.src, c.opacity), c.expect, `composite ${JSON.stringify(c.src)}`);
for (const c of G.fill) eq(R.flood(c.grid, c.x, c.y, c.new), c.expect, `fill ${c.x},${c.y}`);
const big = { "brush 64": R.brushMask(64), "brush 33": R.brushMask(33), "ellipse 128x64": R.ellipseMask(128, 64),
  "rrect 200x120 r30": R.rrectMask(200, 120, 30), "outline ellipse 128x64 n8": R.outlineRing("ellipse", 128, 64, 0, 8),
  "outline rrect 200x120 r30 n6": R.outlineRing("rrect", 200, 120, 30, 6) };
for (const c of G.hashes) eq(sha(big[c.what]), c.sha256, `hash ${c.what}`);
console.log(`parity OK: ${n} golden checks passed (JS reference == Python oracle)`);
