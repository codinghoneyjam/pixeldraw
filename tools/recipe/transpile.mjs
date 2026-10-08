// <META - FILE SUMMARY - Recipe JSON transpiler: legacy recipe -> draw_tool_v2 Document + PNG
//
// Lives in tools/ (not src/): recipe schema resolution stays out of core (P-1).

import fs from "node:fs";
import path from "node:path";

import { Session } from "../../src/features/document/session.js";
import { PixelWriter } from "../../src/core/chunkstore.js";
import { exportPngBytes } from "../../src/io/export_png.js";
import { documentToJson } from "../../src/io/serialize.js";
import { assembleSheet } from "../../src/core/raster/atlas.js";
import { applyCommand, renderTile, setGlyphSets } from "./render_tile.js";
import { loadGlyphSet } from "./text_glyphs.mjs";
import { packRGBA } from "../../src/core/pixel.js";
import { isPantographProfile, isSurfaceManifest, pantographCommandsForLayers, surfaceCommandsFor } from "./schema.mjs";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
setGlyphSets(loadGlyphSet(path.join(__dirname, "text_glyphs.json")));

async function renderPantographProfile({ recipe, layerKey, slotId, paletteOverrides, outputDocPath, outputPngPath }) {
  const key = slotId ?? layerKey;
  let docW;
  let docH;
  let commands = [];
  if (key && recipe.nine_patch?.[key]) {
    const entry = recipe.nine_patch[key];
    docW = entry.width;
    docH = entry.height;
    commands = pantographCommandsForLayers(recipe, entry.layers);
    console.log(`[Transpiler] Processing ${commands.length} commands for pantograph nine_patch '${key}'...`);
  } else {
    const canvas = recipe.canvas;
    const cellW = canvas.slot_width;
    docW = canvas.total_width;
    docH = canvas.total_height;
    for (const slot of recipe.slots) {
      const dx = slot.index * cellW;
      for (const cmd of pantographCommandsForLayers(recipe, slot.layers)) {
        const shifted = { ...cmd, box: [[cmd.box[0][0] + dx, cmd.box[0][1]], [cmd.box[1][0] + dx, cmd.box[1][1]]] };
        commands.push(shifted);
      }
    }
    console.log(`[Transpiler] Processing ${commands.length} commands for pantograph sheet (${recipe.slots.length} slots)...`);
  }
  const session = new Session();
  session.newDocument({ widthPx: docW, heightPx: docH });
  const doc = session.doc;
  const activeLayer = doc.getLayer(doc.activeLayerId);
  const writer = new PixelWriter(activeLayer.store, activeLayer.id);
  const ctx = { writer, palette: { ...(paletteOverrides ?? {}) }, width: docW, height: docH };
  for (const cmd of commands) {
    applyCommand(ctx, cmd);
  }
  writer.finish();
  return await emitOutputs(doc, outputDocPath, outputPngPath, null);
}

// Composite-surface manifest (asset_work/tidy U36-U39 evidence:
// hud_composite_surfaces.json) is the SSOT for U36-U39 (+U37 scanline entry,
// which the mapping card prescribes adding). Surface layers already speak the
// transpiler dialect under legacy names: `type` for the command, `bbox` for
// the box. Colors are literal #RRGGBB(AA); the manifest carries no $tokens.
async function renderSurfaceManifest({ recipe, layerKey, slotId, paletteOverrides, outputDocPath, outputPngPath }) {
  const key = slotId ?? layerKey;
  const { surface, commands } = surfaceCommandsFor(recipe, key);
  const docW = surface.canvas.width;
  const docH = surface.canvas.height;
  console.log(`[Transpiler] Processing ${commands.length} commands for surface '${surface.id}'...`);
  const session = new Session();
  session.newDocument({ widthPx: docW, heightPx: docH });
  const doc = session.doc;
  const activeLayer = doc.getLayer(doc.activeLayerId);
  const writer = new PixelWriter(activeLayer.store, activeLayer.id);
  const ctx = { writer, palette: { ...(paletteOverrides ?? {}) }, width: docW, height: docH };
  for (const cmd of commands) {
    applyCommand(ctx, cmd);
  }
  writer.finish();
  return await emitOutputs(doc, outputDocPath, outputPngPath, null);
}

// <META - ROLE : Transpile recipe JSON into a rendered Document + PNG | L13-176>
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

  if (isPantographProfile(recipe)) {
    return await renderPantographProfile({
      recipe, layerKey, slotId, paletteOverrides, outputDocPath, outputPngPath,
    });
  }

  if (isSurfaceManifest(recipe)) {
    return await renderSurfaceManifest({
      recipe, layerKey, slotId, paletteOverrides, outputDocPath, outputPngPath,
    });
  }

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

  const session = new Session();
  session.newDocument({ widthPx: docW, heightPx: docH, viewport: vp });
  const doc = session.doc;
  const activeLayer = doc.getLayer(doc.activeLayerId);
  const writer = new PixelWriter(activeLayer.store, activeLayer.id);

  let commands = [];

  // Atlas-with-slots schema (enemy_shadow_recipe, wheel_recipe).
  const slots = recipe.export?.atlas?.slots ?? recipe.atlas?.slots;
  const atlasDef = recipe.export?.atlas ?? recipe.atlas;
  if (slots && !slotId) {
    // SHEET MODE: one invocation renders every slot and assembles the strip.
    // The legacy enemy baker bakes 16 disjoint 128px cells and pastes each tile
    // through ITSELF as the mask, which squares alpha and premultiplies colour.
    // Plain source-over does not reproduce it -- see src/core/raster/atlas.js.
    const cellW = atlasDef.cell_width ?? atlasDef.cellWidth ?? docW;
    const cellH = atlasDef.cell_height ?? atlasDef.cellHeight ?? docH;
    const tiles = slots.map((slot) => {
      const slotPalette = Object.assign({}, palette, slot.palette ?? {});
      const tileCmds = [];
      for (const lyr of (slot.layers ?? [])) {
        if (!layerKey || lyr.id === layerKey) {
          tileCmds.push(...(lyr.commands ?? []));
        }
      }
      return renderTile(tileCmds, slotPalette, cellW, cellH);
    });
    const sheet = assembleSheet(tiles);
    // The strip is cells.size * cellW wide, so it needs its own document; the
    // single-cell document created above cannot hold it.
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
    return await emitOutputs(sheetDoc, outputDocPath, outputPngPath, null);
  }
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

  // SINGLE dispatch site: render_tile.js owns the command switch. Keeping a
  // second copy here is what let the atlas path drift from the document path.
  const ctx = { writer, palette, width: docW, height: docH };
  for (const cmd of commands) {
    applyCommand(ctx, cmd);
  }

  writer.finish();

  return await emitOutputs(doc, outputDocPath, outputPngPath, vp);
}

// <META - ROLE : Write the document JSON and PNG for a rendered document | L141-164>
/**
 * Serialise a document to disk and return both artefacts.
 * @param {object} doc draw_tool_v2 document
 * @param {string} outputDocPath destination for the document JSON
 * @param {string} outputPngPath destination for the PNG
 * @param {?object} vp logical viewport, or null to emit the whole canvas
 * @returns {Promise<{docJson:object, pngBytes:Uint8Array}>}
 */
async function emitOutputs(doc, outputDocPath, outputPngPath, vp) {
  const docJson = await documentToJson(doc);
  fs.writeFileSync(outputDocPath, JSON.stringify(docJson, null, 2), "utf-8");
  console.log(`[Transpiler] Exported Document JSON -> ${outputDocPath}`);

  // A recipe that declares a viewport (or a legacy crop) gets the cropped size,
  // so the emitted PNG matches the legacy asset's dimensions.
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
  let layerExplicit = false;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--recipe") recipeRel = args[++i];
    if (args[i] === "--layer") { layer = args[++i]; layerExplicit = true; }
    if (args[i] === "--slot") slot = args[++i];
    if (args[i] === "--out") outPath = args[++i];
  }

  // An atlas recipe renders every slot when no slot is named, so the CLI's
  // default layer must not filter it out. Pass null unless --layer was given.
  if (!layerExplicit) layer = null;

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
