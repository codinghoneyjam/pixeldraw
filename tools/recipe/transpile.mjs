// <META - FILE SUMMARY - Recipe JSON transpiler: legacy recipe -> draw_tool_v2 Document + PNG
//
// Lives in tools/ (not src/): recipe schema resolution stays out of core (P-1).

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { Session } from "../../src/model/session.js";
import { applyShape } from "../../src/core/raster/shape_raster.js";
import { arcMask, chordFillMask, chordOutlineMask } from "../../src/core/raster/arc.js";
import { radialGradientMask } from "../../src/core/raster/gradient.js";
import { measureText, textMask } from "../../src/core/raster/text_mask.js";
import { loadGlyphSet } from "./text_glyphs.mjs";
import { PixelWriter } from "../../src/core/chunkstore.js";
import { exportPngBytes } from "../../src/io/export_png.js";
import { documentToJson } from "../../src/io/serialize.js";
import { resolveColor } from "./color_tokens.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Committed bitmap glyphs (no runtime font parsing). Loaded once.
const GLYPH_SETS = loadGlyphSet(path.join(__dirname, "text_glyphs.json"));

// <META - ROLE : Transpile recipe JSON into a rendered Document + PNG | L26-307>
export async function transpileAndRender({
  recipePath,
  layerKey = null,
  slotId = null,
  paletteOverrides = {},
  canvasWidth = 128,
  canvasHeight = 128,
  outputDocPath,
  outputPngPath
}) {
  const content = fs.readFileSync(recipePath, "utf-8");
  const recipe = JSON.parse(content);

  const width = recipe.canvas?.width ?? (Array.isArray(recipe.canvas) ? recipe.canvas[0] : canvasWidth);
  const height = recipe.canvas?.height ?? (Array.isArray(recipe.canvas) ? recipe.canvas[1] : canvasHeight);

  let palette = Object.assign({}, recipe.palette ?? {}, recipe.dynamic_colors ?? {}, paletteOverrides);

  // export.canvas overrides the stored document size (it must stay a 32
  // multiple for chunk alignment); export.viewport is the logical output crop
  // for a non-aligned asset size. Commands keep their own coordinates.
  const ex = recipe.export ?? {};
  const docW = ex.canvas?.width ?? width;
  const docH = ex.canvas?.height ?? height;

  // Legacy bakers often drew on a padded canvas then trimmed the transparent
  // margin, recording the trimmed size as `crop: [w, h]` (title_logo_text_*).
  // The trim is anchored top-left, which for those assets is exact. Model it as
  // a viewport so the same crop path serves both spellings.
  const vp = ex.viewport ?? (recipe.crop
    ? { x: 0, y: 0, w: recipe.crop[0], h: recipe.crop[1] }
    : null);

  // Initialize draw_tool_v2 Session
  const session = new Session();
  session.newDocument({ widthPx: docW, heightPx: docH, viewport: vp });
  const doc = session.doc;
  const activeLayer = doc.getLayer(doc.activeLayerId);
  const writer = new PixelWriter(activeLayer.store, activeLayer.id);

  let commands = [];

  // 1. Check if recipe is Atlas with slots (enemy_shadow_recipe, wheel_recipe)
  const slots = recipe.export?.atlas?.slots ?? recipe.atlas?.slots;
  if (slots && slotId) {
    const slot = slots.find((s) => s.id === slotId) ?? slots[0];
    if (slot) {
      palette = Object.assign({}, palette, slot.palette ?? {});
      for (const lyr of (slot.layers ?? [])) {
        if (!layerKey || lyr.id === layerKey) {
          commands.push(...(lyr.commands ?? []));
        }
      }
    }
  } else if (recipe.layers) {
    if (Array.isArray(recipe.layers)) {
      for (const lyr of recipe.layers) {
        if (!layerKey || lyr.id === layerKey) {
          commands.push(...(lyr.commands ?? []));
        }
      }
    } else if (typeof recipe.layers === "object") {
      const target = layerKey ? recipe.layers[layerKey] : Object.values(recipe.layers)[0];
      commands = Array.isArray(target) ? target : (target?.commands ?? []);
    }
  } else if (layerKey === "mask" || layerKey === "mask_layers") {
    commands = recipe.mask_layers ?? [];
  } else if (recipe.albedo_layers) {
    commands = recipe.albedo_layers ?? [];
  } else if (layerKey && Array.isArray(recipe[layerKey])) {
    // Player-shape schema: layers are top-level keys (shadow, body, ...).
    commands = recipe[layerKey] ?? [];
  }

  console.log(`[Transpiler] Processing ${commands.length} commands for '${recipePath}' (layer: ${layerKey}, slot: ${slotId})...`);

  for (const cmd of commands) {
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
      // Legacy float box semantics: center/radii stay unrounded (PIL takes
      // the box as-is). Legacy arc color: color ?? outline ?? fill.
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
          paintArcMask(arcMask(acx, acy, arx, ary, aStart, aEnd, sw, docW, docH), aCol);
        } else {
          const hasFill = fillCol !== 0;
          const hasOutline = outlineCol !== 0;
          const sw = cmd.width ?? 1;
          if (hasFill && hasOutline) {
            paintArcMask(chordFillMask(acx, acy, arx, ary, aStart, aEnd, docW, docH), fillCol);
            paintArcMask(chordOutlineMask(acx, acy, arx, ary, aStart, aEnd, sw, docW, docH), outlineCol);
          } else if (hasFill) {
            paintArcMask(chordFillMask(acx, acy, arx, ary, aStart, aEnd, docW, docH), fillCol);
          } else if (hasOutline) {
            paintArcMask(chordOutlineMask(acx, acy, arx, ary, aStart, aEnd, sw, docW, docH), outlineCol);
          }
        }
      }
    } else if (type === "radial_gradient") {
      const stops = cmd.stops ?? [];
      if (stops.length === 0) {
        console.warn(`[transpile] radial_gradient without stops, skipped`);
      } else {
        const [gcx, gcy] = cmd.center ?? [64, 64];
        // Span the STORED document, not the recipe canvas: when export.canvas
        // pads the document the mask and the paint loop must cover the same
        // extent, or the gradient is written at the wrong stride.
        const grad = radialGradientMask({ cx: gcx, cy: gcy, r0: cmd.r0 ?? 0, r1: cmd.r1 ?? 16, stops, w: docW, h: docH });
        for (let gy = 0; gy < docH; gy++) {
          for (let gx = 0; gx < docW; gx++) {
            const packed = grad.data[gy * docW + gx];
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
        // Mask spans the STORED document, not the viewport crop: the painter
        // loop below iterates the same extent.
        const tm = textMask({ text: txt, glyphs, x: penX, y: penY, fill: tFill, stroke: tStroke, strokeWidth: tSw, strokeMode: cmd.stroke_mode, w: docW, h: docH });
        for (let ty = 0; ty < docH; ty++) {
          for (let tx = 0; tx < docW; tx++) {
            const packed = tm.data[ty * docW + tx];
            if (packed !== 0) writer.set(tx, ty, packed);
          }
        }
      }
    } else {
      console.warn(`[transpile] unsupported type ${type}, skipped (will be wired in T-5/T-6/T-7)`);
    }
  }

  writer.finish();

  // 1. Export draw_tool_v2 Document JSON
  const docJson = await documentToJson(doc);
  fs.writeFileSync(outputDocPath, JSON.stringify(docJson, null, 2), "utf-8");
  console.log(`[Transpiler] Exported Document JSON -> ${outputDocPath}`);

  // 2. Export PNG. A recipe that declares a viewport (or a legacy crop) gets the
  // cropped size, so the emitted PNG matches the legacy asset's dimensions.
  const cropToViewport = Boolean(vp);
  const pngBytes = await exportPngBytes(doc, { cropToViewport });
  fs.writeFileSync(outputPngPath, Buffer.from(pngBytes));
  console.log(`[Transpiler] Exported PNG -> ${outputPngPath}${cropToViewport ? " (viewport cropped)" : ""}`);

  return { docJson, pngBytes };
}

// CLI execution test
if (process.argv[1] && process.argv[1].endsWith("transpile.mjs")) {
  const args = process.argv.slice(2);
  let recipeRel = "../../assetdb/world/object/portal_keycap_master.json";
  let layer = "socket";
  let slot = null;
  let outPath = "transpile_out.png";

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--recipe") recipeRel = args[++i];
    if (args[i] === "--layer") layer = args[++i];
    if (args[i] === "--slot") slot = args[++i];
    if (args[i] === "--out") outPath = args[++i];
  }

  const recipePath = path.resolve(process.cwd(), recipeRel);
  const outputPngPath = path.resolve(process.cwd(), outPath);
  const outputDocPath = outputPngPath.replace(/\.png$/i, "") + "_drawtool.json";

  await transpileAndRender({
    recipePath,
    layerKey: layer,
    slotId: slot,
    outputDocPath,
    outputPngPath
  });
}
