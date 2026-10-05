// <META - FILE SUMMARY - Recipe command dispatch + single-tile rasterisation.
//
// Pure module: no DOM, no Session, no filesystem. ESM, Node-importable.
// applyCommand is the ONLY command dispatch in the recipe path: transpile.mjs
// drives the stored document through it, renderTile drives an atlas cell.
// Do not fork a second copy and do not reorder the branches: the branch order
// is the recipe semantics and the 20-asset parity gate depends on it.

import path from "node:path";
import { fileURLToPath } from "node:url";

import { arcMask, chordFillMask, chordOutlineMask } from "../../src/core/raster/arc.js";
import { radialGradientMask } from "../../src/core/raster/gradient.js";
import { measureText, textMask } from "../../src/core/raster/text_mask.js";
import { applyShape } from "../../src/core/raster/shape_raster.js";
import { loadGlyphSet } from "./text_glyphs.mjs";
import { resolveColor } from "./color_tokens.mjs";
import { ChunkStore, PixelWriter } from "../../src/core/chunkstore.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Committed bitmap glyphs (no runtime font parsing). Loaded once.
const GLYPH_SETS = loadGlyphSet(path.join(__dirname, "text_glyphs.json"));

// <META - ROLE : Apply one recipe command to the context writer | L25-215>
/**
 * Apply one recipe command to the context's writer.
 *
 * `ctx.width`/`ctx.height` are the STORED extent the paint loops iterate, which
 * differs from the recipe canvas whenever export.canvas pads the document.
 *
 * @param {{writer:object, palette:object, width:number, height:number}} ctx render context
 * @param {object} cmd recipe command
 * @returns {void}
 */
export function applyCommand(ctx, cmd) {
  const { writer, palette, width, height } = ctx;
  const type = cmd.type ?? cmd.cmd;
  const strokeWidth = cmd.width ?? 1;
  const fillCol = resolveColor(cmd.fill ?? cmd.color, palette);
  const outlineCol = resolveColor(cmd.outline, palette);

  let bbox = null;
  if (cmd.box) {
    const [[x0, y0], [x1, y1]] = cmd.box;
    bbox = [Math.round(x0), Math.round(y0), Math.round(x1 - x0 + 1), Math.round(y1 - y0 + 1)];
  } else if (cmd.bbox) {
    const [[x0, y0], [x1, y1]] = cmd.bbox;
    bbox = [Math.round(x0), Math.round(y0), Math.round(x1 - x0 + 1), Math.round(y1 - y0 + 1)];
  } else if (cmd.center && cmd.radius) {
    const [cx, cy] = cmd.center;
    const r = cmd.radius;
    bbox = [Math.round(cx - r), Math.round(cy - r), Math.round(r * 2 + 1), Math.round(r * 2 + 1)];
  }

  if (type === "rounded_rect") {
    const radius = cmd.radius ?? 8;
    if (fillCol && outlineCol) {
      applyShape(writer, { kind: "rrect", bbox, radius, strokeWidth, fillMode: "both" }, outlineCol, fillCol);
    } else if (fillCol) {
      applyShape(writer, { kind: "rrect", bbox, radius, strokeWidth, fillMode: "fill" }, fillCol, 0);
    } else if (outlineCol) {
      applyShape(writer, { kind: "rrect", bbox, radius, strokeWidth, fillMode: "outline" }, outlineCol, 0);
    }
  } else if (type === "rect" || type === "rectangle") {
    if (bbox) {
      if (fillCol && outlineCol) {
        applyShape(writer, { kind: "rrect", bbox, radius: 0, strokeWidth, fillMode: "both" }, outlineCol, fillCol);
      } else if (fillCol) {
        applyShape(writer, { kind: "rrect", bbox, radius: 0, strokeWidth, fillMode: "fill" }, fillCol, 0);
      } else if (outlineCol) {
        applyShape(writer, { kind: "rrect", bbox, radius: 0, strokeWidth, fillMode: "outline" }, outlineCol, 0);
      }
    }
  } else if (type === "circle" || type === "ellipse") {
    if (fillCol && outlineCol) {
      applyShape(writer, { kind: "ellipse", bbox, strokeWidth, fillMode: "both" }, outlineCol, fillCol);
    } else if (fillCol) {
      applyShape(writer, { kind: "ellipse", bbox, strokeWidth, fillMode: "fill" }, fillCol, 0);
    } else if (outlineCol) {
      applyShape(writer, { kind: "ellipse", bbox, strokeWidth, fillMode: "outline" }, outlineCol, 0);
    }
  } else if (type === "line") {
    // Legacy d.line(pts) strokes a polyline through ALL points; drawing
    // only the first segment drops the rest (e.g. bow string 2nd span).
    const pts = cmd.points ?? cmd.pts;
    const sCol = fillCol || outlineCol;
    if (pts && pts.length >= 2) {
      for (let i = 0; i < pts.length - 1; i++) {
        const p0 = { x: Math.round(pts[i][0]), y: Math.round(pts[i][1]) };
        const p1 = { x: Math.round(pts[i + 1][0]), y: Math.round(pts[i + 1][1]) };
        applyShape(writer, { kind: "line", p0, p1, strokeWidth }, sCol, 0);
      }
    }
  } else if (type === "lines") {
    const segs = cmd.segments ?? cmd.lines ?? [];
    for (const seg of segs) {
      const pts = seg.points ?? seg.pts ?? seg;
      const sCol = resolveColor(seg.color ?? seg.fill ?? cmd.color, palette) || fillCol || outlineCol;
      const sw = seg.width ?? strokeWidth;
      if (pts && pts.length >= 2) {
        const p0 = { x: Math.round(pts[0][0]), y: Math.round(pts[0][1]) };
        const p1 = { x: Math.round(pts[1][0]), y: Math.round(pts[1][1]) };
        applyShape(writer, { kind: "line", p0, p1, strokeWidth: sw }, sCol, 0);
      }
    }
  } else if (type === "spokes") {
    const [cx, cy] = cmd.center ?? [64, 64];
    const count = cmd.count ?? 8;
    const length = cmd.length ?? 20;
    const innerR = cmd.inner_radius ?? 0;
    const offset = cmd.angle_offset ?? 0;
    const sw = cmd.width ?? 1;
    const sCol = fillCol || outlineCol;
    for (let i = 0; i < count; i++) {
      const rad = ((offset + (360 * i) / count) * Math.PI) / 180;
      const p0 = { x: Math.round(cx + innerR * Math.cos(rad)), y: Math.round(cy + innerR * Math.sin(rad)) };
      const p1 = { x: Math.round(cx + (innerR + length) * Math.cos(rad)), y: Math.round(cy + (innerR + length) * Math.sin(rad)) };
      applyShape(writer, { kind: "line", p0, p1, strokeWidth: sw }, sCol, 0);
    }
  } else if (type === "polygon") {
    const rawPts = cmd.pts ?? cmd.points ?? [];
    const roundedPts = rawPts.map((p) =>
      Array.isArray(p) ? [Math.round(p[0]), Math.round(p[1])] : { x: Math.round(p.x), y: Math.round(p.y) },
    );
    if (roundedPts.length >= 3) {
      const hasFill = fillCol !== 0;
      const hasOutline = outlineCol !== 0;
      if (hasFill && hasOutline) {
        applyShape(writer, { kind: "polygon", points: roundedPts, strokeWidth, fillMode: "both" }, outlineCol, fillCol);
      } else if (hasFill) {
        applyShape(writer, { kind: "polygon", points: roundedPts, strokeWidth, fillMode: "fill" }, fillCol, 0);
      } else if (hasOutline) {
        applyShape(writer, { kind: "polygon", points: roundedPts, strokeWidth, fillMode: "outline" }, outlineCol, 0);
      }
    }
  } else if (type === "arc" || type === "chord") {
    // Legacy float box semantics: centre/radii stay unrounded (PIL takes the
    // box as-is). Legacy arc colour precedence: color ?? outline ?? fill.
    const arcBox = cmd.box ?? cmd.bbox;
    if (!arcBox) {
      console.warn(`[transpile] ${type} without box, skipped`);
    } else {
      const acx = (arcBox[0][0] + arcBox[1][0]) / 2;
      const acy = (arcBox[0][1] + arcBox[1][1]) / 2;
      const arx = (arcBox[1][0] - arcBox[0][0]) / 2;
      const ary = (arcBox[1][1] - arcBox[0][1]) / 2;
      const aStart = cmd.start ?? 0;
      const aEnd = cmd.end ?? 360;
      const paintArcMask = (mask, packed) => {
        for (let my = 0; my < mask.h; my++) {
          for (let mx = 0; mx < mask.w; mx++) {
            if (mask.data[my * mask.w + mx] === 1) writer.set(mx, my, packed);
          }
        }
      };
      if (type === "arc") {
        const aCol = fillCol || outlineCol;
        const sw = cmd.width ?? 2;
        paintArcMask(arcMask(acx, acy, arx, ary, aStart, aEnd, sw, width, height), aCol);
      } else {
        const hasFill = fillCol !== 0;
        const hasOutline = outlineCol !== 0;
        const sw = cmd.width ?? 1;
        if (hasFill && hasOutline) {
          paintArcMask(chordFillMask(acx, acy, arx, ary, aStart, aEnd, width, height), fillCol);
          paintArcMask(chordOutlineMask(acx, acy, arx, ary, aStart, aEnd, sw, width, height), outlineCol);
        } else if (hasFill) {
          paintArcMask(chordFillMask(acx, acy, arx, ary, aStart, aEnd, width, height), fillCol);
        } else if (hasOutline) {
          paintArcMask(chordOutlineMask(acx, acy, arx, ary, aStart, aEnd, sw, width, height), outlineCol);
        }
      }
    }
  } else if (type === "radial_gradient") {
    const stops = cmd.stops ?? [];
    if (stops.length === 0) {
      console.warn(`[transpile] radial_gradient without stops, skipped`);
    } else {
      const [gcx, gcy] = cmd.center ?? [64, 64];
      // Span ctx.width/height (the STORED document), not the recipe canvas:
      // when export.canvas pads the document the mask and the paint loop must
      // cover the same extent, or the gradient is written at the wrong stride.
      const grad = radialGradientMask({ cx: gcx, cy: gcy, r0: cmd.r0 ?? 0, r1: cmd.r1 ?? 16, stops, w: width, h: height });
      for (let gy = 0; gy < height; gy++) {
        for (let gx = 0; gx < width; gx++) {
          const packed = grad.data[gy * width + gx];
          if (packed !== 0) writer.set(gx, gy, packed);
        }
      }
    }
  } else if (type === "text") {
    const txt = cmd.text ?? "";
    const glyphs = GLYPH_SETS[cmd.font ?? "monogram-240"]?.glyphs;
    if (!txt || !glyphs || ![...txt].every((ch) => glyphs[ch])) {
      console.warn(`[transpile] unsupported text ${JSON.stringify(txt)}, skipped`);
    } else {
      // Legacy centers recipe texts via textbbox (diamond), while baked
      // logo fixtures carry pen origins: pos = center, x/y (pen/xy) = pen.
      let penX;
      let penY;
      if (cmd.pos) {
        const m = measureText(glyphs, txt);
        penX = cmd.pos[0] - Math.floor(m.w / 2);
        penY = cmd.pos[1] - Math.floor(m.h / 2) - 2;
      } else {
        [penX, penY] = cmd.pen ?? cmd.xy ?? [0, 0];
      }
      const tFill = resolveColor(cmd.fill ?? cmd.color ?? "#FFFFFF", palette);
      const tStroke = resolveColor(cmd.stroke_fill ?? cmd.stroke ?? tFill, palette);
      const tSw = cmd.stroke_width ?? cmd.strokeWidth ?? cmd.width ?? 0;
      // Mask spans ctx.width/height (the STORED document), not the viewport
      // crop: the painter loop below iterates the same extent.
      const tm = textMask({ text: txt, glyphs, x: penX, y: penY, fill: tFill, stroke: tStroke, strokeWidth: tSw, strokeMode: cmd.stroke_mode, w: width, h: height });
      for (let ty = 0; ty < height; ty++) {
        for (let tx = 0; tx < width; tx++) {
          const packed = tm.data[ty * width + tx];
          if (packed !== 0) writer.set(tx, ty, packed);
        }
      }
    }
  } else {
    console.warn(`[transpile] unsupported type ${type}, skipped (will be wired in T-5/T-6/T-7)`);
  }
}

// <META - ROLE : Render a command list into one packed-RGBA tile | L217-237>
/**
 * Render a list of recipe commands into a single packed-RGBA tile.
 *
 * Each atlas slot is rendered independently at a fixed cell size, then the tiles
 * are assembled by assembleSheet(). Rendering into a plain ChunkStore keeps the
 * tiles independent: a slot never overwrites a neighbour.
 *
 * @param {object[]} commands recipe commands ({ cmd | type, ... })
 * @param {object} palette resolved token map for this tile
 * @param {number} cellW tile width in pixels
 * @param {number} cellH tile height in pixels
 * @returns {{w:number,h:number,data:Uint32Array}} packed RGBA tile, 0 = transparent
 */
export function renderTile(commands, palette, cellW, cellH) {
  const store = new ChunkStore(cellW, cellH);
  const writer = new PixelWriter(store, "tile");
  const ctx = { writer, palette, width: cellW, height: cellH };
  for (const cmd of commands) {
    applyCommand(ctx, cmd);
  }
  writer.finish();
  const data = new Uint32Array(cellW * cellH);
  for (let i = 0; i < cellW * cellH; i++) {
    data[i] = store.getPixel(i % cellW, Math.floor(i / cellW));
  }
  return { w: cellW, h: cellH, data };
}