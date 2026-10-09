// <META - FILE SUMMARY - Brush-size guard, kept next to brush code through the Slice D split>
//
// Pure module: no DOM, no Session, ESM, Node-importable.
//
// Only assertBrushSize remains here. The duplicated brushFootprint and
// forEachBresenham that used to live beside it were deleted: features/pen/brush.js
// is the single implementation (imported by pen.js, panel_brush.js and the
// overlays), and the two copies had already drifted in error type - this one
// threw a raw RangeError while the canonical one throws DrawToolError
// (contract §2 rule 7 allows only the latter).

import { PEN_MAX, PEN_MIN } from "../constants.js";
import { DrawToolError } from "../errors.js";

/**
 * Canonical brush-size guard.
 *
 * Throws DrawToolError so the guard sits inside contract §2-7 like every other
 * raster error. Reachable from user data (a recipe's stroke_width reaches this
 * through applyShape), so it cannot stay a bare RangeError.
 *
 * @param {number} n brush size
 * @returns {void}
 */
export function assertBrushSize(n) {
  if (!Number.isInteger(n) || n < PEN_MIN || n > PEN_MAX) {
    throw new DrawToolError("OUT_OF_RANGE", `brush size must be an integer in [${PEN_MIN},${PEN_MAX}], got ${n}`);
  }
}
