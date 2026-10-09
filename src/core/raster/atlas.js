// <META - FILE SUMMARY - Atlas sheet assembly: place slot tiles into a strip using PIL's paste law>
//
// Pure module: no DOM, no Session, ESM, Node-importable.
//
// MEASURED FACT: the legacy enemy baker assembles the 16-slot sheet with
//     sheet.paste(part, (idx * CANVAS, 0), part)
// i.e. each tile is pasted through ITSELF as the mask, which squares alpha and
// premultiplies colour. Over all 256 alpha values and 2000 random RGBA inputs:
//     outR = (c * a + 127) / 255      outA = (a * a + 127) / 255
// Plain source-over does NOT reproduce this (alpha 128 -> 64, not 128), so
// assembleSheet() must. Destination alpha 0 reduces the general law
//     out = src * m + dst * (1 - m),   m = a / 255
// to the closed form; overlapping tiles never occur (disjoint columns) but the
// loop honours the general form anyway.

import { assertMaskSize } from "./raster_masks.js";
import { DrawToolError } from "../errors.js";

/** PIL's integer blend rounding: (value * mask + 127) / 255 */
function blend255(value, mask) {
  return Math.floor((value * mask + 127) / 255);
}

/**
 * PIL paste of an RGBA source through itself as the mask.
 * @param {number} dstC destination channel 0..255
 * @param {number} srcC source channel 0..255
 * @param {number} a source alpha 0..255
 * @returns {number} blended channel 0..255
 */
export function pasteChannel(dstC, srcC, a) {
  return blend255(srcC, a) + Math.floor((dstC * (255 - a) + 127) / 255);
}

/**
 * PIL paste of an RGBA source through itself as the mask, all four channels.
 * @param {number[]} dst destination [r, g, b, a]
 * @param {number[]} src source [r, g, b, a]
 * @returns {number[]} blended [r, g, b, a]
 */
export function pastePixel(dst, src) {
  const a = src[3];
  if (a === 0) return dst.slice();
  if (dst[3] === 0) {
    return [blend255(src[0], a), blend255(src[1], a), blend255(src[2], a), blend255(a, a)];
  }
  const inv = 255 - a;
  return [
    blend255(src[0], a) + Math.floor((dst[0] * inv + 127) / 255),
    blend255(src[1], a) + Math.floor((dst[1] * inv + 127) / 255),
    blend255(src[2], a) + Math.floor((dst[2] * inv + 127) / 255),
    blend255(src[3], a) + Math.floor((dst[3] * inv + 127) / 255),
  ];
}

/**
 * Assemble a horizontal atlas strip from slot tiles.
 * Tiles must all be the same size; the strip is tiles.length * tileW wide.
 * @param {{w:number,h:number,data:Uint32Array}[]} tiles one entry per slot, in slot order
 * @returns {{w:number,h:number,data:Uint8ClampedArray}} RGBA strip, row-major
 */
export function assembleSheet(tiles) {
  if (!Array.isArray(tiles) || tiles.length === 0) {
    throw new DrawToolError("INVALID_STATE", "assembleSheet needs at least one tile");
  }
  const { w: tw, h: th } = tiles[0];
  // assertMaskSize already rejects w/h < 1 as DrawToolError(OUT_OF_RANGE), so the
  // zero-size tile below is caught by the length check, not here.
  assertMaskSize(tw, th);
  for (const t of tiles) {
    if (t.w !== tw || t.h !== th) {
      throw new DrawToolError("INVALID_STATE", `tile size mismatch: expected ${tw}x${th}, got ${t.w}x${t.h}`);
    }
    if (t.data.length !== tw * th) {
      throw new DrawToolError("INVALID_STATE", `tile ${t.w}x${t.h} data length ${t.data.length} != ${tw * th}`);
    }
  }
  const out = new Uint8ClampedArray(tw * tiles.length * th * 4);
  for (let slot = 0; slot < tiles.length; slot++) {
    const tile = tiles[slot];
    const xOff = slot * tw;
    for (let y = 0; y < th; y++) {
      for (let x = 0; x < tw; x++) {
        const s = tile.data[y * tw + x];
        const dOff = (y * tw * tiles.length + xOff + x) * 4;
        if (s === 0) continue;
        const merged = pastePixel(
          [out[dOff], out[dOff + 1], out[dOff + 2], out[dOff + 3]],
          [s & 255, (s >>> 8) & 255, (s >>> 16) & 255, (s >>> 24) & 255],
        );
        out[dOff] = merged[0];
        out[dOff + 1] = merged[1];
        out[dOff + 2] = merged[2];
        out[dOff + 3] = merged[3];
      }
    }
  }
  return { w: tw * tiles.length, h: th, data: out };
}