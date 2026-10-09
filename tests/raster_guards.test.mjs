// Raster guards must throw DrawToolError (contract §2-7), not a bare RangeError.
//
// This was the last open contract violation in src/core/raster/*: assertBrushSize
// and assertMaskSize threw RangeError, and assembleSheet threw three of its own.
// They are all reachable from user data (a recipe's stroke_width reaches
// assertBrushSize through applyShape), so a caller must be able to branch on a
// code rather than sniff an error class.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { DrawToolError } from "../src/core/errors.js";
import { assertBrushSize } from "../src/core/raster/raster_brush.js";
import { assertMaskSize } from "../src/core/raster/raster_masks.js";
import { assembleSheet } from "../src/core/raster/atlas.js";
import { lineMask } from "../src/core/raster/segment.js";
import { applyShape } from "../src/core/raster/shape_raster.js";
import { PEN_MIN, PEN_MAX } from "../src/core/constants.js";

function codeOf(fn) {
  try {
    fn();
    return null;
  } catch (e) {
    return e instanceof DrawToolError ? e.code : e.constructor.name;
  }
}

const noop = { set() {} };

describe("raster guards throw DrawToolError", () => {
  it("assertBrushSize rejects the whole illegal range with OUT_OF_RANGE", () => {
    const bad = [PEN_MIN - 1, PEN_MAX + 1, 0, -1, 1.5, NaN, Infinity, "3", null, undefined];
    for (const n of bad) {
      assert.equal(codeOf(() => assertBrushSize(n)), "OUT_OF_RANGE", `brush size ${String(n)}`);
    }
    // The legal endpoints must stay legal.
    assert.equal(codeOf(() => assertBrushSize(PEN_MIN)), null);
    assert.equal(codeOf(() => assertBrushSize(PEN_MAX)), null);
  });

  it("assertMaskSize rejects non-integers and zero/negative with OUT_OF_RANGE", () => {
    for (const [w, h] of [[0, 4], [4, 0], [-1, 4], [4, -1], [1.5, 4], [4, 2.5], [NaN, 4], [4, NaN]]) {
      assert.equal(codeOf(() => assertMaskSize(w, h)), "OUT_OF_RANGE", `mask ${w}x${h}`);
    }
    assert.equal(codeOf(() => assertMaskSize(1, 1)), null);
    assert.equal(codeOf(() => assertMaskSize(2048, 1088)), null);
  });

  it("assembleSheet codes its three invariants", () => {
    assert.equal(codeOf(() => assembleSheet([])), "INVALID_STATE");
    assert.equal(codeOf(() => assembleSheet(null)), "INVALID_STATE");
    assert.equal(codeOf(() => assembleSheet([{ w: 4, h: 4, data: new Uint32Array(16) }, { w: 8, h: 4, data: new Uint32Array(32) }])), "INVALID_STATE");
    // A 1x1 tile is a legal size, so it fails the data-length check instead.
    assert.equal(codeOf(() => assembleSheet([{ w: 1, h: 1, data: new Uint32Array(4) }])), "INVALID_STATE");
    // A 0x4 tile is an illegal size, caught by assertMaskSize before the length check.
    assert.equal(codeOf(() => assembleSheet([{ w: 0, h: 4, data: new Uint32Array(0) }])), "OUT_OF_RANGE");
  });

  it("no RangeError escapes src/core/raster/** any more", () => {
    // Scans the sources so a future regression cannot reintroduce the class.
    const offenders = [];
    const walk = (dir) => {
      for (const e of readdirSync(dir, { withFileTypes: true })) {
        const p = join(dir, e.name);
        if (e.isDirectory()) walk(p);
        else if (e.name.endsWith(".js")) {
          if (/new RangeError/.test(readFileSync(p, "utf8"))) offenders.push(p);
        }
      }
    };
    walk(join(process.cwd(), "src", "core", "raster"));
    assert.deepEqual(offenders, [], "RangeError still thrown in src/core/raster/**");
  });

  it("a bad stroke width reaching applyShape is a coded error, not a RangeError", () => {
    // The user-facing path: a recipe (or a numeric field) with strokeWidth 999
    // lands in applyShape, which calls assertBrushSize.
    assert.equal(
      codeOf(() => applyShape(noop, { kind: "rect", bbox: { x: 0, y: 0, w: 8, h: 8 }, strokeWidth: 999 }, 0xff0000ff, 0xff0000ff)),
      "OUT_OF_RANGE",
    );
    assert.equal(
      codeOf(() => applyShape(noop, { kind: "rect", bbox: { x: 0, y: 0, w: 8, h: 8 }, strokeWidth: -2 }, 0xff0000ff, 0xff0000ff)),
      "OUT_OF_RANGE",
    );
  });

  it("lineMask rejects an illegal brush size the same way", () => {
    assert.equal(codeOf(() => lineMask({ x: 0, y: 0 }, { x: 8, y: 8 }, 0)), "OUT_OF_RANGE");
    assert.equal(codeOf(() => lineMask({ x: 0, y: 0 }, { x: 8, y: 8 }, 999)), "OUT_OF_RANGE");
  });
});
