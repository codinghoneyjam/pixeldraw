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

/**
 * Canonical brush-size guard.
 * @param {number} n brush size
 * @returns {void}
 */
export function assertBrushSize(n) {
  if (!Number.isInteger(n) || n < 1 || n > 64) {
    throw new RangeError(`brush size must be an integer in [1,64], got ${n}`);
  }
}
