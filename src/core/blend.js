// Shared blend math (docs/contract.md §1-1, §2). Exact op order — do not reorder.
//
// This module is the SINGLE source of truth for how two RGBA values combine.
// Three compositors consume it: the display path (render/composite.js, both
// compositeChunk and samplePixel), the PNG flatten (io/export_png.js) and the
// merge-down command (features/layers/commands_pixel.js). They must all read
// the same formulas or the same pixel shows one colour on screen, another in
// the PNG, and a third under the eyedropper.

export function rnd(v) {
  return Math.floor(v + 0.5);
}

// Layer blend modes, in the order the UI lists them. Features and the schema
// validator import this list instead of restating it.
export const BLEND_MODES = Object.freeze(["normal", "multiply", "screen", "overlay", "darken", "lighten"]);

// <META - ROLE : Blend one straight (unpremultiplied) channel pair, 0..255 integer math | L24-36>
// The formulas below are literally the ones table-stated in contract §1-1, all
// working on 0..255 integers and rounded once at the end with rnd(). Mixing two
// rounding conventions would make the result depend on evaluation order.
// The overlay seam at dc=128 is the documented formula's own behaviour: 128 is
// >= 128, so it takes the screen branch and lands 1 LSB off the ideal midpoint.
function blendChannel(mode, sc, dc) {
  switch (mode) {
    case "multiply": return rnd((sc * dc) / 255);
    case "screen": return rnd(sc + dc - (sc * dc) / 255);
    case "overlay": return dc < 128
      ? rnd((2 * sc * dc) / 255)
      : rnd(255 - (2 * (255 - sc) * (255 - dc)) / 255);
    case "darken": return Math.min(sc, dc);
    case "lighten": return Math.max(sc, dc);
    default: return sc;
  }
}

// <META - ROLE : SSEOT WRITE: mode-aware source-over, the only place alpha is combined | L37-56>
/**
 * Source-over composite with the source colour replaced by blend(mode, src, dst).
 *
 * Alpha handling is deliberately identical to `over` (contract §1-1 rule 2):
 * only the RGB that gets carried forward changes. `over` is the normal-mode
 * alias, kept so callers with no concept of layers (glyph rasterisation, tests)
 * keep a stable name.
 *
 * `da === 0` skips the blend entirely (rule 3): the backdrop colour is
 * undefined, so multiplying against black would make a transparent region
 * black instead of leaving the source untouched.
 *
 * @param {number[]} dst straight [r,g,b,a] accumulated so far
 * @param {number[]} src straight [r,g,b,a] of the layer pixel
 * @param {number} opacity layer opacity 0..1
 * @param {string} mode one of BLEND_MODES
 * @returns {number[]} straight [r,g,b,a]
 */
export function overBlend(dst, src, opacity, mode) {
  const sa = (src[3] / 255) * opacity;
  if (sa === 0) return [dst[0], dst[1], dst[2], dst[3]];
  const da = dst[3] / 255;
  const oa = sa + da * (1 - sa);
  const inv = 1 - sa;
  // A mode of "normal" (or an unknown string) must produce exactly what over()
  // produces, byte for byte, so the fast paths downstream stay honest.
  const blend = mode !== "normal" && da > 0;
  const c0 = blend ? rnd((blendChannel(mode, src[0], dst[0]) * sa + dst[0] * da * inv) / oa) : rnd((src[0] * sa + dst[0] * da * inv) / oa);
  const c1 = blend ? rnd((blendChannel(mode, src[1], dst[1]) * sa + dst[1] * da * inv) / oa) : rnd((src[1] * sa + dst[1] * da * inv) / oa);
  const c2 = blend ? rnd((blendChannel(mode, src[2], dst[2]) * sa + dst[2] * da * inv) / oa) : rnd((src[2] * sa + dst[2] * da * inv) / oa);
  const a = rnd(oa * 255);
  if (a === 0) return [0, 0, 0, 0];
  return [c0, c1, c2, a];
}

// <META - ROLE : normal-mode alias for callers with no layer concept | L58-62>
export function over(dst, src, opacity) {
  return overBlend(dst, src, opacity, "normal");
}

// <META - ROLE : true when the mode needs the blend math rather than plain source-over | L64-66>
export function isBlendMode(mode) {
  return typeof mode === "string" && BLEND_MODES.includes(mode);
}
