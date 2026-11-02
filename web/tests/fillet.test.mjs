// Golden tests for filletPolygon: src port must reproduce the legacy oracle masks.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { filletPolygon, polygonMask } from "../src/core/raster/polygon.js";

const G = JSON.parse(
  readFileSync(new URL("./fixtures/raster_golden.json", import.meta.url), "utf-8"),
);

const rowsOfMask = (mask) => {
  const rows = [];
  for (let y = 0; y < mask.h; y++) {
    let s = "";
    for (let x = 0; x < mask.w; x++) s += mask.data[y * mask.w + x] === 1 ? "#" : ".";
    rows.push(s);
  }
  return rows;
};

describe("fillet golden bit-match", () => {
  it("polygonMask(filletPolygon(...)) matches all golden fillet rows", () => {
    for (const c of G.fillet) {
      assert.deepEqual(rowsOfMask(polygonMask(filletPolygon(c.pts, c.radius), c.w, c.h)), c.rows, `fillet ${c.name}`);
    }
  });

  it("fillet vertex output matches legacy pts2 in the golden", () => {
    for (const c of G.fillet) {
      assert.deepEqual(filletPolygon(c.pts, c.radius), c.pts2, `fillet verts ${c.name}`);
    }
  });
});

describe("fillet degenerate inputs", () => {
  it("sharp ~15deg corner does not throw", () => {
    const c = G.fillet.find((g) => g.name === "sharp_15deg_corner");
    assert.doesNotThrow(() => filletPolygon(c.pts, c.radius));
    assert.ok(filletPolygon(c.pts, c.radius).length >= 3);
  });

  it("radius 0 is identity", () => {
    const pts = [[2, 2], [13, 2], [12, 12], [3, 12]];
    assert.deepEqual(filletPolygon(pts, 0), pts);
  });

  it("degenerate zero-length edge does not throw", () => {
    const pts = [[3, 3], [3, 3], [13, 3], [8, 13]];
    assert.doesNotThrow(() => filletPolygon(pts, 4));
  });

  it("fewer than 3 points returns input", () => {
    const pts = [[1, 1], [5, 5]];
    assert.deepEqual(filletPolygon(pts, 3), pts);
  });
});
