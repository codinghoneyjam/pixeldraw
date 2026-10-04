// Golden + fillMode behaviour tests for the polygon raster branch.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { applyShape } from "../src/core/raster/shape_raster.js";
import { polygonBBox, polygonInsetRing, polygonMask, polygonOutlineMask } from "../src/core/raster/polygon.js";
import { ShapeTool } from "../src/tools/shape.js";

const G = JSON.parse(
  readFileSync(new URL("./fixtures/raster_golden.json", import.meta.url), "utf-8"),
);

// Clipped-canvas fake writer (same contract as shape_raster_basic tests).
function makeCanvasWriter(W, H) {
  const grid = Array.from({ length: H }, () => new Array(W).fill(0));
  return {
    grid,
    set(x, y, packed) {
      if (x < 0 || y < 0 || x >= W || y >= H) return;
      grid[y][x] = packed;
    },
  };
}

const rowsOfMask = (mask) => {
  const rows = [];
  for (let y = 0; y < mask.h; y++) {
    let s = "";
    for (let x = 0; x < mask.w; x++) s += mask.data[y * mask.w + x] === 1 ? "#" : ".";
    rows.push(s);
  }
  return rows;
};

const toLocal = (pts, bb) =>
  pts.map((p) => (Array.isArray(p) ? [p[0] - bb.x, p[1] - bb.y] : [p.x - bb.x, p.y - bb.y]));

// paint a {w,h,data} mask into a W x H grid at (dx,dy)
const paintGrid = (W, H, mask, dx, dy, packed) => {
  const grid = Array.from({ length: H }, () => new Array(W).fill(0));
  for (let y = 0; y < mask.h; y++) {
    for (let x = 0; x < mask.w; x++) {
      if (mask.data[y * mask.w + x] !== 1) continue;
      const cx = dx + x;
      const cy = dy + y;
      if (cx >= 0 && cy >= 0 && cx < W && cy < H) grid[cy][cx] = packed;
    }
  }
  return grid;
};

describe("polygon golden bit-match", () => {
  it("polygonMask matches all 6 golden polygon rows", () => {
    for (const c of G.polygon) {
      assert.deepEqual(rowsOfMask(polygonMask(c.pts, c.w, c.h)), c.rows, `polygon ${c.name}`);
    }
  });

  it("polygonOutlineMask matches all 6 golden polygon_outline rows", () => {
    for (const c of G.polygon_outline) {
      assert.deepEqual(rowsOfMask(polygonOutlineMask(c.pts, c.w, c.width)), c.rows, `outline ${c.name}`);
    }
  });

  it("applyShape fill produces pixels equal to polygonMask / golden rows", () => {
    const P = 7;
    for (const c of G.polygon) {
      const w = makeCanvasWriter(c.w, c.h);
      applyShape(w, { kind: "polygon", points: c.pts, fillMode: "fill", strokeWidth: 1 }, P, 0);
      const want = Array.from({ length: c.h }, (_, y) =>
        Array.from({ length: c.w }, (_, x) => {
          const m = polygonMask(c.pts, c.w, c.h);
          return m.data[y * c.w + x] === 1 ? P : 0;
        }),
      );
      assert.deepEqual(w.grid, want, `fill ${c.name} == polygonMask`);
      // golden rows frame the same 16x16 canvas
      const golden = c.rows.map((r) => [...r].map((ch) => (ch === "#" ? P : 0)));
      assert.deepEqual(w.grid, golden, `fill ${c.name} == golden rows`);
    }
  });
});

describe("polygon fillMode", () => {
  const P = 7;
  const S = 9;

  it("fillMode outline dispatches centered w1 vs inset w2+ like PIL", () => {
    const widths = new Map(G.polygon_outline.map((c) => [c.name, c.width]));
    for (const c of G.polygon) {
      const sw = widths.get(c.name);
      const w = makeCanvasWriter(c.w, c.h);
      applyShape(w, { kind: "polygon", points: c.pts, fillMode: "outline", strokeWidth: sw }, P, S);
      const bb = polygonBBox(c.pts);
      const want = sw >= 2
        ? paintGrid(c.w, c.h, polygonInsetRing(polygonMask(toLocal(c.pts, bb), bb.w, bb.h), sw), bb.x, bb.y, P)
        : paintGrid(c.w, c.h, polygonOutlineMask(toLocal(c.pts, bb), Math.max(bb.w, bb.h), sw), bb.x, bb.y, P);
      assert.deepEqual(w.grid, want, `outline ${c.name}`);
      assert.ok(w.grid.flat().every((v) => v === 0 || v === P), `outline ${c.name} primary only`);
    }
  });

  it("PIL polygon-outline width rule on the measured square", () => {
    // PIL-measured (poly_inset.py probe): w1 centered like d.line, w>=2
    // full-width ring inset inside the fill boundary.
    const sq = [[3, 3], [12, 3], [12, 12], [3, 12]];
    const bb = polygonBBox(sq);
    const fill = polygonMask(sq.map(([x, y]) => [x - bb.x, y - bb.y]), bb.w, bb.h);
    const col7 = (grid) => grid.map((row) => row[7]);
    const run = (sw) => {
      const w = makeCanvasWriter(16, 16);
      applyShape(w, { kind: "polygon", points: sq, fillMode: "outline", strokeWidth: sw }, P, S);
      return w.grid;
    };
    assert.deepEqual(col7(run(1)).map((v) => (v === P ? 1 : 0)).join("").slice(0, 16),
      "0001000000001000", "w1 boundary rows");
    assert.deepEqual(col7(run(2)).map((v) => (v === P ? 1 : 0)).join(""),
      "0001100000011000", "w2 inset rows");
    assert.deepEqual(col7(run(3)).map((v) => (v === P ? 1 : 0)).join(""),
      "0001110000111000", "w3 inset rows");
  });

  it("fillMode both paints secondary fill + primary outline", () => {
    const c = G.polygon.find((g) => g.name === "square");
    const sw = 3;
    const fillW = makeCanvasWriter(c.w, c.h);
    const outW = makeCanvasWriter(c.w, c.h);
    const bothW = makeCanvasWriter(c.w, c.h);
    applyShape(fillW, { kind: "polygon", points: c.pts, fillMode: "fill", strokeWidth: sw }, P, S);
    applyShape(outW, { kind: "polygon", points: c.pts, fillMode: "outline", strokeWidth: sw }, P, S);
    applyShape(bothW, { kind: "polygon", points: c.pts, fillMode: "both", strokeWidth: sw }, P, S);
    let fillOnly = 0;
    for (let y = 0; y < c.h; y++) {
      for (let x = 0; x < c.w; x++) {
        if (fillW.grid[y][x] === P && outW.grid[y][x] === 0) {
          assert.equal(bothW.grid[y][x], S, `fill-only interior pixel ${x},${y} is secondary`);
          fillOnly++;
        }
        // Outline is painted after the fill, so it wins wherever it paints.
        // (With pure-fill semantics the ring sits inside the fill region, so a
        // "primary-only" sample is geometry-dependent and not required.)
        if (outW.grid[y][x] === P) {
          assert.equal(bothW.grid[y][x], P, `outline pixel ${x},${y} is primary`);
        }
        assert.ok(bothW.grid[y][x] === 0 || bothW.grid[y][x] === P || bothW.grid[y][x] === S, `both pixel ${x},${y} valid`);
      }
    }
    assert.ok(fillOnly > 0, "found a secondary-only fill sample");
  });
});

describe("polygon kind isolation", () => {
  it("unknown kind still throws in ShapeTool; polygon not in tool KINDS", () => {
    assert.throws(() => new ShapeTool({}, "triangle"), /unknown shape kind/);
    assert.throws(() => new ShapeTool({}, "polygon"), /unknown shape kind/);
  });

  it("rect path unchanged (fill equals rrect-region of bbox from spec.bbox)", () => {
    const P = 4;
    const w = makeCanvasWriter(12, 12);
    applyShape(w, { kind: "rect", bbox: { x: 2, y: 2, w: 8, h: 8 }, strokeWidth: 1, fillMode: "fill" }, P, 0);
    for (let y = 2; y < 10; y++) {
      for (let x = 2; x < 10; x++) assert.equal(w.grid[y][x], P, `rect fill ${x},${y}`);
    }
    assert.ok(w.grid.flat().every((v) => v === 0 || v === P));
  });
});
