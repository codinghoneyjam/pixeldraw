// <META - FILE SUMMARY - Radial multi-stop gradient raster (shockwave ring reproduction)>
//
// Pure module: no DOM, ESM, Node-importable.
// Output is per-pixel packed RGBA ({w, h, data: Uint32Array}), not a 1-bit
// mask: a gradient carries color, so callers write nonzero pixels via PixelWriter.

import { assertMaskSize } from "./raster_masks.js";
import { packRGBA } from "../pixel.js";
import { DrawToolError } from "../errors.js";

// Truncation (floor). The legacy shockwave baker uses int() truncation for
// alpha/luminance, and the T-5 gate is alpha parity vs that baker: round-half-up
// disagrees with truncation on ~50% of fractional pixels, so the spec rule here
// is truncation. Must match the oracle floor() in tools/gen_fixtures.py.
const gradTrunc = (v) => Math.floor(v);
const clamp01 = (t) => (t < 0 ? 0 : t > 1 ? 1 : t);

// <META - ROLE : Interpolate one RGBA tuple across sorted stops at t | L14-35>
function sampleStops(sorted, t) {
  const first = sorted[0];
  const last = sorted[sorted.length - 1];
  if (t <= first[0]) return [first[1], first[2], first[3], first[4]];
  if (t >= last[0]) return [last[1], last[2], last[3], last[4]];
  for (let i = 0; i < sorted.length - 1; i++) {
    const a = sorted[i];
    const b = sorted[i + 1];
    if (t < a[0] || t > b[0]) continue;
    const f = b[0] === a[0] ? 0 : (t - a[0]) / (b[0] - a[0]);
    return [
      gradTrunc(a[1] + (b[1] - a[1]) * f),
      gradTrunc(a[2] + (b[2] - a[2]) * f),
      gradTrunc(a[3] + (b[3] - a[3]) * f),
      gradTrunc(a[4] + (b[4] - a[4]) * f),
    ];
  }
  return [last[1], last[2], last[3], last[4]];
}

// <META - ROLE : Radial gradient with non-monotonic multi-stop profile | L37-62>
/**
 * Radial gradient over a w x h canvas. Stops are [t, r, g, b, a] tuples
 * (t float, channels numbers truncated to int at sampling); multi-stop
 * profiles may be non-monotonic (e.g. transparent -> peak -> transparent
 * ring). Float channels reproduce smooth legacy profiles bit-closely;
 * int channels work too. Pixels with alpha 0 are left as 0 so callers can
 * skip them.
 */
export function radialGradientMask({ cx, cy, r0, r1, stops, w, h }) {
  assertMaskSize(w, h);
  if (!Array.isArray(stops) || stops.length === 0) {
    throw new DrawToolError("INVALID_STATE", "radialGradientMask needs at least one stop");
  }
  const sorted = stops.map((s) => [s[0], s[1], s[2], s[3], s[4]]);
  sorted.sort((a, b) => a[0] - b[0]);
  const data = new Uint32Array(w * h);
  const span = r1 - r0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const d = Math.hypot(x - cx, y - cy);
      const t = span <= 0 ? (d <= r0 ? 0 : 1) : clamp01((d - r0) / span);
      const c = sampleStops(sorted, t);
      if (c[3] > 0) data[y * w + x] = packRGBA(c[0], c[1], c[2], c[3]);
    }
  }
  return { w, h, data };
}
