// <META - FILE SUMMARY - Recipe converter: legacy recipe <-> draw_tool_v2 Document JSON>
//
// Converts legacy recipe JSON to draw_tool_v2 Document JSON (layer-based format)
// and vice versa. This enables:
//   1. Legacy recipe -> Document JSON (layer set)
//   2. Document JSON -> Legacy recipe (reverse conversion)
//   3. Document JSON -> PNG (rendering)
//   4. PNG -> Document JSON (not supported - raster to vector is lossy)
//
// Usage:
//   node tools/recipe/recipe_converter.mjs --to-doc --recipe <path> --out <path>
//   node tools/recipe/recipe_converter.mjs --to-legacy --doc <path> --out <path>
//   node tools/recipe/recipe_converter.mjs --to-png --doc <path> --out <path>

import fs from "node:fs";
import path from "node:path";

import { Session } from "../../src/model/session.js";
import { PixelWriter } from "../../src/core/chunkstore.js";
import { exportPngBytes } from "../../src/io/export_png.js";
import { documentToJson, jsonToDocument } from "../../src/io/serialize.js";
import { assembleSheet } from "../../src/core/raster/atlas.js";
import { applyCommand, renderTile, setGlyphSets } from "./render_tile.js";
import { loadGlyphSet } from "./text_glyphs.mjs";
import { packRGBA } from "../../src/core/pixel.js";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
setGlyphSets(loadGlyphSet(path.join(__dirname, "text_glyphs.json")));

// Pantograph keycap profile detection
function isPantographProfile(recipe) {
  return Boolean(recipe && recipe.geometry && recipe.colors && Array.isArray(recipe.slots));
}

function pantographRoundedCmd(bboxFlat, radius, fill, outline, width) {
  const [x0, y0, x1, y1] = bboxFlat;
  const cmd = { cmd: "rounded_rect", box: [[x0, y0], [x1, y1]], radius };
  if (fill) cmd.fill = fill;
  if (outline) cmd.outline = outline;
  if (width !== null && width !== undefined) cmd.width = width;
  return cmd;
}

function pantographCommandsForLayers(profile, layers) {
  const geometry = profile.geometry;
  const colors = profile.colors;
  const cmds = [];
  if (layers.includes("outer_frame")) {
    const spec = geometry.outer_frame;
    cmds.push(pantographRoundedCmd(spec.bbox, spec.corner_radius,
      colors.outer_frame_bg_hex, colors.outer_frame_border_hex, spec.border_width));
  }
  if (layers.includes("socket_well")) {
    const spec = geometry.socket_well;
    cmds.push(pantographRoundedCmd(spec.bbox, spec.corner_radius,
      colors.socket_well_bg_hex, null, null));
  }
  const innerByLayer = {
    inner_keycap_normal: ["normal_bbox", "keycap_body_normal_hex", "keycap_border_normal_hex"],
    inner_keycap_pressed: ["pressed_bbox", "keycap_body_pressed_hex", "keycap_border_pressed_hex"],
    inner_keycap_hover: ["normal_bbox", "keycap_body_hover_hex", "keycap_border_hover_hex"],
    inner_keycap_disabled: ["normal_bbox", "keycap_body_disabled_hex", "keycap_border_disabled_hex"],
  };
  for (const [layer, [bboxName, bodyKey, borderKey]] of Object.entries(innerByLayer)) {
    if (layers.includes(layer)) {
      const spec = geometry.inner_keycap;
      cmds.push(pantographRoundedCmd(spec[bboxName], spec.corner_radius,
        colors[bodyKey], colors[borderKey], spec.border_width));
    }
  }
  if (layers.includes("visor_window_normal") || layers.includes("visor_window_pressed")) {
    const pressed = layers.includes("visor_window_pressed");
    const spec = geometry.visor_window;
    const bbox = pressed ? spec.pressed_bbox : spec.normal_bbox;
    cmds.push(pantographRoundedCmd(bbox, spec.corner_radius,
      colors.visor_bg_hex, colors.visor_border_hex, spec.border_width));
  }
  return cmds;
}

// Surface manifest detection
function isSurfaceManifest(recipe) {
  return Boolean(recipe && Array.isArray(recipe.surfaces));
}

// <META - ROLE : Convert legacy recipe to draw_tool_v2 Document | L75-180>
async function legacyRecipeToDocument(recipe) {
  const session = new Session();

  if (isPantographProfile(recipe)) {
    const canvas = recipe.canvas;
    const cellW = canvas.slot_width;
    const docW = canvas.total_width;
    const docH = canvas.total_height;

    session.newDocument({ widthPx: docW, heightPx: docH });
    const doc = session.doc;
    const activeLayer = doc.getLayer(doc.activeLayerId);
    const writer = new PixelWriter(activeLayer.store, activeLayer.id);

    const commands = [];
    for (const slot of recipe.slots) {
      const dx = slot.index * cellW;
      for (const cmd of pantographCommandsForLayers(recipe, slot.layers)) {
        const shifted = { ...cmd, box: [[cmd.box[0][0] + dx, cmd.box[0][1]], [cmd.box[1][0] + dx, cmd.box[1][1]]] };
        commands.push(shifted);
      }
    }

    const ctx = { writer, palette: {}, width: docW, height: docH };
    for (const cmd of commands) {
      applyCommand(ctx, cmd);
    }
    writer.finish();
    return doc;
  }

  if (isSurfaceManifest(recipe)) {
    const surface = recipe.surfaces[0];
    const docW = surface.canvas.width;
    const docH = surface.canvas.height;

    session.newDocument({ widthPx: docW, heightPx: docH });
    const doc = session.doc;
    const activeLayer = doc.getLayer(doc.activeLayerId);
    const writer = new PixelWriter(activeLayer.store, activeLayer.id);

    const commands = (surface.layers ?? []).map((layer) => {
      const cmd = { ...layer, cmd: layer.type ?? layer.cmd };
      if (cmd.bbox && !cmd.box) cmd.box = cmd.bbox;
      return cmd;
    });

    const ctx = { writer, palette: {}, width: docW, height: docH };
    for (const cmd of commands) {
      applyCommand(ctx, cmd);
    }
    writer.finish();
    return doc;
  }

  // Standard recipe
  const width = recipe.canvas?.width ?? (Array.isArray(recipe.canvas) ? recipe.canvas[0] : 128);
  const height = recipe.canvas?.height ?? (Array.isArray(recipe.canvas) ? recipe.canvas[1] : 128);
  const palette = Object.assign({}, recipe.palette ?? {}, recipe.dynamic_colors ?? {});

  const ex = recipe.export ?? {};
  const docW = ex.canvas?.width ?? width;
  const docH = ex.canvas?.height ?? height;

  const vp = ex.viewport ?? (recipe.crop
    ? { x: 0, y: 0, w: recipe.crop[0], h: recipe.crop[1] }
    : null);

  session.newDocument({ widthPx: docW, heightPx: docH, viewport: vp });
  const doc = session.doc;
  const activeLayer = doc.getLayer(doc.activeLayerId);
  const writer = new PixelWriter(activeLayer.store, activeLayer.id);

  let commands = [];

  // Atlas-with-slots schema
  const slots = recipe.export?.atlas?.slots ?? recipe.atlas?.slots;
  const atlasDef = recipe.export?.atlas ?? recipe.atlas;
  if (slots) {
    const cellW = atlasDef.cell_width ?? atlasDef.cellWidth ?? docW;
    const cellH = atlasDef.cell_height ?? atlasDef.cellHeight ?? docH;
    const tiles = slots.map((slot) => {
      const slotPalette = Object.assign({}, palette, slot.palette ?? {});
      const tileCmds = [];
      for (const lyr of (slot.layers ?? [])) {
        tileCmds.push(...(lyr.commands ?? []));
      }
      return renderTile(tileCmds, slotPalette, cellW, cellH);
    });
    const sheet = assembleSheet(tiles);
    session.newDocument({ widthPx: sheet.w, heightPx: sheet.h });
    const sheetDoc = session.doc;
    const sheetLayer = sheetDoc.getLayer(sheetDoc.activeLayerId);
    const sw = new PixelWriter(sheetLayer.store, sheetLayer.id);
    for (let i = 0; i < sheet.w * sheet.h; i++) {
      const r = sheet.data[i * 4];
      const g = sheet.data[i * 4 + 1];
      const b = sheet.data[i * 4 + 2];
      const a = sheet.data[i * 4 + 3];
      if (a === 0) continue;
      sw.set(i % sheet.w, Math.floor(i / sheet.w), packRGBA(r, g, b, a));
    }
    sw.finish();
    return sheetDoc;
  }

  // Standard layers
  if (recipe.layers) {
    if (Array.isArray(recipe.layers)) {
      for (const lyr of recipe.layers) {
        commands.push(...(lyr.commands ?? []));
      }
    } else if (typeof recipe.layers === "object") {
      const target = Object.values(recipe.layers)[0];
      commands = Array.isArray(target) ? target : (target?.commands ?? []);
    }
  } else if (recipe.mask_layers) {
    commands = recipe.mask_layers ?? [];
  } else if (recipe.albedo_layers) {
    commands = recipe.albedo_layers ?? [];
  }

  const ctx = { writer, palette, width: docW, height: docH };
  for (const cmd of commands) {
    applyCommand(ctx, cmd);
  }
  writer.finish();

  return doc;
}

// <META - ROLE : Convert draw_tool_v2 Document to legacy recipe format | L185-260)>
function documentToLegacyRecipe(doc) {
  const recipe = {
    canvas: {
      width: doc.canvas.widthPx,
      height: doc.canvas.heightPx
    },
    palette: {},
    layers: []
  };

  if (doc.canvas.viewport) {
    recipe.export = {
      viewport: {
        x: doc.canvas.viewport.x,
        y: doc.canvas.viewport.y,
        w: doc.canvas.viewport.w,
        h: doc.canvas.viewport.h
      }
    };
  }

  for (const layer of doc.layers) {
    const layerObj = {
      id: layer.name || layer.layer_id,
      commands: []
    };

    // Extract commands from chunk data
    const commands = [];
    layer.store.forEachChunk((cx, cy, data) => {
      for (let y = 0; y < 32; y++) {
        for (let x = 0; x < 32; x++) {
          const idx = (y * 32 + x) * 4;
          const r = data[idx];
          const g = data[idx + 1];
          const b = data[idx + 2];
          const a = data[idx + 3];
          if (a > 0) {
            commands.push({
              cmd: "pixel",
              x: cx * 32 + x,
              y: cy * 32 + y,
              color: `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`,
              alpha: a
            });
          }
        }
      }
    });

    layerObj.commands = commands;
    recipe.layers.push(layerObj);
  }

  return recipe;
}

// <META - ROLE : Convert Document JSON to legacy recipe (reverse) | L265-300)>
async function documentJsonToLegacyRecipe(docJson) {
  const { document } = await jsonToDocument(docJson);
  return documentToLegacyRecipe(document);
}

// <META - ROLE : Main CLI entry point | L305-380)>
async function main() {
  const args = process.argv.slice(2);
  let mode = null;
  let inputPath = null;
  let outputPath = null;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--to-doc") mode = "to-doc";
    if (args[i] === "--to-legacy") mode = "to-legacy";
    if (args[i] === "--to-png") mode = "to-png";
    if (args[i] === "--recipe") inputPath = args[++i];
    if (args[i] === "--doc") inputPath = args[++i];
    if (args[i] === "--out") outputPath = args[++i];
  }

  if (!mode || !inputPath || !outputPath) {
    console.error("Usage:");
    console.error("  node recipe_converter.mjs --to-doc --recipe <path> --out <path>");
    console.error("  node recipe_converter.mjs --to-legacy --doc <path> --out <path>");
    console.error("  node recipe_converter.mjs --to-png --doc <path> --out <path>");
    process.exit(1);
  }

  const input = fs.readFileSync(inputPath, "utf-8");
  const inputJson = JSON.parse(input);

  if (mode === "to-doc") {
    // Legacy recipe -> Document JSON
    const doc = await legacyRecipeToDocument(inputJson);
    const docJson = await documentToJson(doc);
    fs.writeFileSync(outputPath, JSON.stringify(docJson, null, 2), "utf-8");
    console.log(`[Converter] Legacy recipe -> Document JSON: ${outputPath}`);
    console.log(`[Converter] Layers: ${docJson.layers.length}`);
    console.log(`[Converter] Canvas: ${docJson.canvas.width_px}x${docJson.canvas.height_px}`);

  } else if (mode === "to-legacy") {
    // Document JSON -> Legacy recipe
    const legacyRecipe = await documentJsonToLegacyRecipe(inputJson);
    fs.writeFileSync(outputPath, JSON.stringify(legacyRecipe, null, 2), "utf-8");
    console.log(`[Converter] Document JSON -> Legacy recipe: ${outputPath}`);
    console.log(`[Converter] Layers: ${legacyRecipe.layers.length}`);

  } else if (mode === "to-png") {
    // Document JSON -> PNG
    const { document } = await jsonToDocument(inputJson);
    const pngBytes = await exportPngBytes(document, { cropToViewport: false });
    fs.writeFileSync(outputPath, Buffer.from(pngBytes));
    console.log(`[Converter] Document JSON -> PNG: ${outputPath}`);
  }
}

main().catch(err => {
  console.error(`[Converter] Error: ${err.message}`);
  process.exit(1);
});
