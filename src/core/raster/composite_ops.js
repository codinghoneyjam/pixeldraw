// <META - FILE SUMMARY - Composite operations that read back already-painted pixels>
//
// Pure module: no DOM, no Session, no filesystem. ESM, Node-importable.
//
// Every operation here acts on the tile ALREADY painted by the commands before
// it, which is why none of them can be expressed as a mask-and-fill primitive in
// shape_raster.js. They exist because the legacy enemy baker composes its slots
// that way:
//
//   clip_alpha   _compat_plain() multiplies a body slot's alpha by clip_mask
//   erase        _compat_inherited() punches `cutouts` polygons to alpha 0
//   visor        render_visor_layer() fills a shape then composites CRT
//                scanlines masked by the same shape
//   sheen        _draw_sheen_overlay() draws two diagonal white lines masked to
//                the visor shape
//   hud_brackets _draw_hud_brackets() strokes eight L-shaped corner segments
//
// All take a PixelWriter because they read back through writer.get().

import { applyShape } from "./shape_raster.js";
import { lineMask } from "./segment.js";
import { polygonMask } from "./polygon.js";
import { rrectMask } from "./raster_masks.js";

const packRGBA = (r, g, b, a) =>
  a === 0 ? 0 : ((((a & 255) << 24) | ((b & 255) << 16) | ((g & 255) << 8) | (r & 255)) >>> 0);
const alphaOf = (px) => (px >>> 24) & 255;
const redOf = (px) => px & 255;
const greenOf = (px) => (px >>> 8) & 255;
const blueOf = (px) => (px >>> 16) & 255;
const withAlpha = (px, a) => (((px & 0x00ffffff) | ((a & 255) << 24)) >>> 0);

// <META - ROLE : Keep only the alpha already painted where a shape is filled | L32-52>
/**
 * Multiply the tile's existing alpha by a rounded-rect clip mask.
 *
 * The legacy baker renders the body, then multiplies alpha by clip_mask so the
 * chamfered corners that fall outside the clip turn transparent. Alpha is scaled,
 * not replaced -- the legacy path composites the mask into the alpha channel.
 *
 * @param {{get:(x:number,y:number)=>number, set:(x:number,y:number,p:number)=>void}} writer tile writer
 * @param {{box:number[][], radius:number}} clip clip_mask shape definition
 * @param {number} w tile width
 * @param {number} h tile height
 * @returns {void}
 */
export function clipAlpha(writer, clip, w, h) {
  const [[x0, y0], [x1, y1]] = clip.box;
  const mask = rrectMask(Math.round(x1 - x0 + 1), Math.round(y1 - y0 + 1), clip.radius ?? 0);
  const ox = Math.round(x0);
  const oy = Math.round(y0);
  // Clear everything OUTSIDE the clip: the legacy path multiplies the whole
  // alpha channel by the mask, so body art that spills past the rounded rect
  // turns transparent rather than staying opaque.
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const mx = x - ox;
      const my = y - oy;
      const inside = mx >= 0 && my >= 0 && mx < mask.w && my < mask.h
        && mask.data[my * mask.w + mx] === 1;
      if (inside) continue;
      const px = writer.get(x, y);
      if (px !== 0) writer.set(x, y, 0);
    }
  }
}

// <META - ROLE : Punch a polygon to fully transparent alpha | L54-68>
/**
 * Set alpha to 0 inside the polygon. Mirrors the legacy `cutouts` path, which
 * composites a black-filled polygon into the alpha channel.
 *
 * @param {{get:(x:number,y:number)=>number, set:(x:number,y:number,p:number)=>void}} writer tile writer
 * @param {number[][]} points polygon vertices as [x, y]
 * @param {number} w tile width
 * @param {number} h tile height
 * @returns {void}
 */
export function erasePolygon(writer, points, w, h) {
  const mask = polygonMask(points, w, h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (mask[y * w + x] === 1) writer.set(x, y, 0);
    }
  }
}

// <META - ROLE : Build a fill mask for a recipe shape definition | L70-90>
/**
 * Mask for a visor/cutout shape. Only fill and polygon shapes occur in the
 * enemy specs; anything else yields an empty mask so callers fail closed.
 *
 * @param {object} shape recipe shape command
 * @param {number} w tile width
 * @param {number} h tile height
 * @returns {{w:number,h:number,data:Uint8Array}} 1 = inside
 */
export function shapeFillMask(shape, w, h) {
  const data = new Uint8Array(w * h);
  if (!shape) return { w, h, data };
  if (Array.isArray(shape.points) && shape.points.length >= 3) {
    return polygonMask(shape.points.map((p) => [Math.round(p[0]), Math.round(p[1])]), w, h);
  }
  if (shape.box) {
    const [[x0, y0], [x1, y1]] = shape.box;
    const bw = Math.round(x1 - x0 + 1);
    const bh = Math.round(y1 - y0 + 1);
    const m = shape.cmd === "rounded_rect" ? rrectMask(bw, bh, shape.radius ?? 0) : null;
    if (!m) return { w, h, data };
    const ox = Math.round(x0);
    const oy = Math.round(y0);
    for (let y = 0; y < m.h; y++) {
      for (let x = 0; x < m.w; x++) {
        if (m.data[y * m.w + x] === 1 && ox + x < w && oy + y < h) data[(oy + y) * w + ox + x] = 1;
      }
    }
  }
  return { w, h, data };
}

// <META - ROLE : Fill the visor shape then composite masked CRT scanlines | L92-124>
/**
 * Render a visor slot: fill and outline its shape, then darken every scanline
 * row inside that shape.
 *
 * The legacy path draws the shape into an overlay, draws scanlines at alpha 45
 * into a second image, multiplies the overlay alpha by a mask built from the same
 * shape, and alpha_composites it. The base under the visor is already opaque, so
 * the composite only darkens -- which is what this reproduces.
 *
 * @param {{get:(x:number,y:number)=>number, set:(x:number,y:number,p:number)=>void}} writer tile writer
 * @param {object} shape visor shape command
 * @param {object} [crtLines] {y_range:[y0,y1,step], x_range:[x0,x1]}
 * @param {number} w tile width
 * @param {number} h tile height
 * @param {number} fillPacked packed fill colour
 * @param {number} outlinePacked packed outline colour
 * @returns {void}
 */
export function visorLayer(writer, shape, crtLines, w, h, fillPacked, outlinePacked) {
  applyShape(writer, { ...shapeArgs(shape), strokeWidth: shape.width ?? 1, fillMode: fillModeOf(shape) },
    outlinePacked, fillPacked);
  if (!crtLines) return;
  const mask = shapeFillMask(shape, w, h);
  const [y0, y1, step = 3] = crtLines.y_range ?? [45, 80, 3];
  const [x0, x1] = crtLines.x_range ?? [20, 108];
  const alpha = crtLines.alpha ?? 45;
  const keep = 1 - alpha / 255;
  for (let y = y0 + 2; y < y1 - 1; y += step) {
    for (let x = x0 + 2; x <= x1 - 2; x++) {
      if (x < 0 || y < 0 || x >= w || y >= h) continue;
      if (mask.data[y * w + x] !== 1) continue;
      const px = writer.get(x, y);
      if (px === 0) continue;
      // Composite black at `alpha` over the existing pixel: colour collapses to
      // zero while alpha stays (base was already opaque).
      writer.set(x, y, packRGBA(
        Math.round(redOf(px) * keep),
        Math.round(greenOf(px) * keep),
        Math.round(blueOf(px) * keep),
        alphaOf(px),
      ));
    }
  }
}

// <META - ROLE : Overlay two diagonal sheen lines, masked to the visor shape | L126-152>
/**
 * A bright main line plus a dimmer offset line, both masked to the visor shape.
 * Mirrors _draw_sheen_overlay(), which draws them at alpha 110 (width 2) and 60
 * (width 1) and alpha_composites them.
 *
 * @param {{get:(x:number,y:number)=>number, set:(x:number,y:number,p:number)=>void}} writer tile writer
 * @param {object} visorShape visor shape, used as the mask
 * @param {object} sheen {p1:[x,y], p2:[x,y], dx:number}
 * @param {number} w tile width
 * @param {number} h tile height
 * @returns {void}
 */
export function sheenOverlay(writer, visorShape, sheen, w, h) {
  if (!visorShape || !sheen) return;
  const mask = shapeFillMask(visorShape, w, h);
  const [p1x, p1y] = sheen.p1 ?? [42, 44];
  const [p2x, p2y] = sheen.p2 ?? [30, 76];
  const dx = sheen.dx ?? 5;
  const lines = [
    { a: [p1x, p1y], b: [p2x, p2y], alpha: 110, width: 2 },
    { a: [p1x + dx, p1y], b: [p2x + dx, p2y], alpha: 60, width: 1 },
  ];

  // Stage 1: the sheen lives in its own overlay. The legacy path (_apply_mask)
  // draws into a transparent overlay, multiplies that overlay's ALPHA by the
  // visor mask, and only then composites. Compositing first and masking after is
  // a different result.
  const overlay = new Uint8Array(w * h * 4);
  for (const ln of lines) {
    // PIL d.line(width>=2) widens the segment into a ROTATED quad filled by the
    // polygon scanline engine, not an axis-aligned stamp. segment.js lineMask is
    // the PIL-exact implementation (verified 54/54 against Pillow).
    const m = lineMask(ln.a, ln.b, ln.width);
    for (let y = 0; y < m.h; y++) {
      for (let x = 0; x < m.w; x++) {
        if (m.data[y * m.w + x] !== 1) continue;
        const tx = m.x + x;
        const ty = m.y + y;
        if (tx < 0 || ty < 0 || tx >= w || ty >= h) continue;
        const o = (ty * w + tx) * 4;
        overlay[o] = 255;
        overlay[o + 1] = 255;
        overlay[o + 2] = 255;
        overlay[o + 3] = Math.max(overlay[o + 3], ln.alpha);
      }
    }
  }

  // Stage 2: multiply the overlay alpha by the visor mask (ImageChops.multiply
  // is a per-channel multiply, NOT a lerp).
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const o = (y * w + x) * 4;
      if (overlay[o + 3] === 0) continue;
      overlay[o + 3] = (overlay[o + 3] * mask.data[y * w + x]) / 255;
    }
  }

  // Stage 3: alpha_composite onto the tile.
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const o = (y * w + x) * 4;
      const sN = overlay[o + 3] / 255;
      if (sN === 0) continue;
      const px = writer.get(x, y);
      const dN = alphaOf(px) / 255;
      const oN = sN + dN * (1 - sN);
      const mix = (sc) => Math.round((overlay[o] * sN + sc * dN * (1 - sN)) / oN);
      writer.set(x, y, packRGBA(mix(redOf(px)), mix(greenOf(px)), mix(blueOf(px)),
        Math.round(oN * 255)));
    }
  }
}

// <META - ROLE : Stroke the eight L-shaped HUD corner brackets | L154-180>
/**
 * Four corner brackets, each an L of two segments of length `arm`, drawn with
 * PIL's width-1 Bresenham path (not the rotated-quad wide-line path).
 *
 * @param {{set:(x:number,y:number,p:number)=>void}} writer tile writer
 * @param {object} hud {box:[[x0,y0],[x1,y1]], arm:number, color:[r,g,b,a], width:number}
 * @param {number} w tile width
 * @param {number} h tile height
 * @returns {void}
 */
export function hudBrackets(writer, hud, w, h) {
  if (!hud) return;
  const [[x0, y0], [x1, y1]] = hud.box ?? [[36, 60], [92, 82]];
  const arm = hud.arm ?? 5;
  const [r, g, b, a] = hud.color ?? [255, 255, 255, 235];
  const packed = packRGBA(r, g, b, a);
  const segs = [
    [[x0, y0], [x0 + arm, y0]], [[x0, y0], [x0, y0 + arm]],
    [[x1, y0], [x1 - arm, y0]], [[x1, y0], [x1, y0 + arm]],
    [[x0, y1], [x0 + arm, y1]], [[x0, y1], [x0, y1 - arm]],
    [[x1, y1], [x1 - arm, y1]], [[x1, y1], [x1, y1 - arm]],
  ];
  for (const [p, q] of segs) {
    applyShape(writer, {
      kind: "line",
      p0: { x: Math.round(p[0]), y: Math.round(p[1]) },
      p1: { x: Math.round(q[0]), y: Math.round(q[1]) },
      strokeWidth: hud.width ?? 1,
      fillMode: "fill",
    }, packed, packed);
  }
}

function fillModeOf(shape) {
  const hasFill = Boolean(shape.fill);
  const hasOutline = Boolean(shape.outline);
  if (hasFill && hasOutline) return "both";
  if (hasFill) return "fill";
  return "outline";
}

function shapeArgs(shape) {
  if (Array.isArray(shape.points)) {
    return { kind: "polygon", points: shape.points };
  }
  const [[x0, y0], [x1, y1]] = shape.box;
  return {
    kind: shape.cmd === "rounded_rect" ? "rrect" : "rrect",
    bbox: [Math.round(x0), Math.round(y0), Math.round(x1 - x0 + 1), Math.round(y1 - y0 + 1)],
    radius: shape.radius ?? 8,
  };
}