// <META - FILE SUMMARY - WheelAccumulator: pure input-domain wheel delta math (60px = 1 step).
//
// Extracted from input_controller.js so the pure numeric part carries no DOM
// knowledge (TL-DRAW-04 Slice D: no core/raster import, no cursor fns here).

/**
 * Input-domain wheel accumulator (TL-DRAW-04 Slice D split).
 * Pure input side of the input/shape boundary: no `core/raster`
 * import, no cursor fns. Shape rasterization lives in
 * `core/raster/shape_raster.js` (TL-DRAW-03 owns it).
 */
export class WheelAccumulator {
  constructor() {
    this._residue = 0;
    this._dir = 0;
  }

  reset() {
    this._residue = 0;
    this._dir = 0;
  }

  /**
   * Accumulate a vertical wheel delta into -step units (pure input math).
   * @param {number} deltaY - Vertical wheel delta in event units.
   * @param {number} [deltaMode=0] - DOM deltaMode (0 px, 1 line x16, 2 page x pageHeight).
   * @param {number} [pageHeight=0] - Viewport height px for deltaMode 2.
   * @returns {number} Negative step count, or 0 when below threshold.
   */
  feed(deltaY, deltaMode = 0, pageHeight = 0) {
    let px = Number(deltaY);
    if (!Number.isFinite(px) || px === 0) return 0;
    if (deltaMode === 1) px *= 16;
    else if (deltaMode === 2) px *= Number(pageHeight) || 0;
    if (px === 0) return 0;
    const dir = Math.sign(px);
    // Direction reversal discards the accumulated residue instead of summing
    // against it, otherwise one flip-back overshoots by up to a full step.
    if (this._dir !== 0 && dir !== this._dir) {
      this._residue = px;
    } else {
      this._residue += px;
    }
    this._dir = dir;
    const steps = Math.trunc(this._residue / 60);
    if (steps !== 0) {
      this._residue -= steps * 60;
      return -steps;
    }
    return 0;
  }
}