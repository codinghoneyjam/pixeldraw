// <META - FILE SUMMARY - Legacy recipe JSON import: bake surface/pantograph/atlas/standard recipes>
// Split out of serialize.js: recipeJsonToDocument mirrors the transpile branches
// in-browser so a recipe file imports identically to the Node-side transpiler.
// <META - SUMMARY CONT - Routing order is surface manifest -> pantograph profile ->
// <META - SUMMARY CONT - atlas slots -> standard recipe; shiftCommand keeps
// <META - SUMMARY CONT - per-slot commands laid out left to right on one sheet.
import { ChunkStore, PixelWriter } from "../core/chunkstore.js";
import { packRGBA } from "../core/pixel.js";
import { DrawToolError } from "../core/errors.js";
import { Document, IdGen } from "../features/document/document.js";
import { Layer } from "../features/layers/layer.js";
import { applyCommand, renderTile } from "../../tools/recipe/render_tile.js";
import { assembleSheet } from "../core/raster/atlas.js";
import { isPantographProfile, isSurfaceManifest, pantographCommandsForLayers, surfaceCommandsFor } from "../../tools/recipe/schema.mjs";

// <META - ROLE : Shift one point or [x,y]/{x,y} list by a slot offset | L17-19>
function shiftPointList(pts, dx, dy) {
  return pts.map((p) => (Array.isArray(p) ? [p[0] + dx, p[1] + dy] : { x: p.x + dx, y: p.y + dy }));
}

// <META - ROLE : Translate the geometry fields of one recipe command | L22-44>
export function shiftCommand(cmd, dx, dy) {
  const out = { ...cmd };
  if (out.box) out.box = [[out.box[0][0] + dx, out.box[0][1] + dy], [out.box[1][0] + dx, out.box[1][1] + dy]];
  if (out.bbox) out.bbox = [[out.bbox[0][0] + dx, out.bbox[0][1] + dy], [out.bbox[1][0] + dx, out.bbox[1][1] + dy]];
  if (out.center) out.center = [out.center[0] + dx, out.center[1] + dy];
  if (out.points) out.points = shiftPointList(out.points, dx, dy);
  if (out.pts) out.pts = shiftPointList(out.pts, dx, dy);
  if (out.pen) out.pen = [out.pen[0] + dx, out.pen[1] + dy];
  if (out.xy) out.xy = [out.xy[0] + dx, out.xy[1] + dy];
  if (out.pos) out.pos = [out.pos[0] + dx, out.pos[1] + dy];
  const segKey = out.segments ? "segments" : out.lines ? "lines" : null;
  if (segKey) {
    out[segKey] = out[segKey].map((seg) => {
      if (Array.isArray(seg)) return shiftPointList(seg, dx, dy);
      const s = { ...seg };
      if (s.points) s.points = shiftPointList(s.points, dx, dy);
      if (s.pts) s.pts = shiftPointList(s.pts, dx, dy);
      return s;
    });
  }
  if (out.shape && typeof out.shape === "object") out.shape = shiftCommand(out.shape, dx, dy);
  return out;
}

// <META - ROLE : Render a command list into a fresh document layer | L47-53>
function bakeCommandsIntoLayer(store, layerId, commands, palette, width, height) {
  const writer = new PixelWriter(store, layerId);
  const ctx = { writer, palette, width, height };
  for (const cmd of commands) applyCommand(ctx, cmd);
  writer.finish();
  return writer;
}

// <META - ROLE : Build a Document from a legacy recipe JSON (mirror of transpile branches) | L56-178>
export function recipeJsonToDocument(obj, { layerKey = null, slotId = null, paletteOverrides = {} } = {}) {
  if (!obj || typeof obj !== "object") {
    throw new DrawToolError("RECIPE_INVALID", "recipe JSON must be an object");
  }
  const palette = { ...(obj.palette ?? {}), ...(obj.dynamic_colors ?? {}), ...paletteOverrides };
  const ids = new IdGen();
  const mkLayer = (id, name, store, visible = true) => new Layer(id, name, visible, false, 1.0, "normal", store);
  const bg = obj.canvas?.background === "white" ? "white" : "transparent";

  if (isSurfaceManifest(obj)) {
    const key = slotId ?? layerKey ?? obj.surfaces[0]?.id;
    const { surface, commands } = surfaceCommandsFor(obj, key);
    const w = surface.canvas.width;
    const h = surface.canvas.height;
    const store = new ChunkStore(w, h);
    bakeCommandsIntoLayer(store, key, commands, palette, w, h);
    const layer = mkLayer(key, key, store);
    return new Document({ name: obj.manifest_id ?? key, canvas: { widthPx: w, heightPx: h, background: bg }, layers: [layer], activeLayerId: layer.id, ids });
  }

  if (isPantographProfile(obj)) {
    const key = slotId ?? layerKey;
    if (key && obj.nine_patch?.[key]) {
      const entry = obj.nine_patch[key];
      const store = new ChunkStore(entry.width, entry.height);
      bakeCommandsIntoLayer(store, key, pantographCommandsForLayers(obj, entry.layers), palette, entry.width, entry.height);
      const layer = mkLayer(key, key, store);
      return new Document({ name: obj.name ?? key, canvas: { widthPx: entry.width, heightPx: entry.height, background: bg }, layers: [layer], activeLayerId: layer.id, ids });
    }
    const cellW = obj.canvas.slot_width;
    const w = obj.canvas.total_width;
    const h = obj.canvas.total_height;
    const store = new ChunkStore(w, h);
    const all = [];
    for (const slot of obj.slots) {
      const dx = slot.index * cellW;
      for (const cmd of pantographCommandsForLayers(obj, slot.layers)) all.push(shiftCommand(cmd, dx, 0));
    }
    bakeCommandsIntoLayer(store, "sheet", all, palette, w, h);
    const layer = mkLayer("sheet", "sheet", store);
    return new Document({ name: obj.name ?? "pantograph_sheet", canvas: { widthPx: w, heightPx: h, background: bg }, layers: [layer], activeLayerId: layer.id, ids });
  }

  const atlasDef = obj.export?.atlas ?? obj.atlas;
  const slots = atlasDef?.slots;
  if (slots && slots.length > 0) {
    const cellW = atlasDef.cell_width ?? atlasDef.cellWidth ?? obj.canvas?.width ?? 128;
    const cellH = atlasDef.cell_height ?? atlasDef.cellHeight ?? obj.canvas?.height ?? 128;
    const tiles = slots.map((slot) => {
      const tileCmds = [];
      for (const lyr of slot.layers ?? []) {
        if (!layerKey || lyr.id === layerKey) tileCmds.push(...(lyr.commands ?? []));
      }
      return renderTile(tileCmds, { ...palette, ...(slot.palette ?? {}) }, cellW, cellH);
    });
    const sheet = assembleSheet(tiles);
    const store = new ChunkStore(sheet.w, sheet.h);
    const writer = new PixelWriter(store, "sheet");
    for (let i = 0; i < sheet.w * sheet.h; i++) {
      const a = sheet.data[i * 4 + 3];
      if (a === 0) continue;
      writer.set(i % sheet.w, Math.floor(i / sheet.w), packRGBA(sheet.data[i * 4], sheet.data[i * 4 + 1], sheet.data[i * 4 + 2], a));
    }
    writer.finish();
    const sheetLayer = mkLayer("sheet", "sheet", store);
    const layers = [sheetLayer];
    // Per-slot layers are hidden editing references; the visible sheet layer
    // keeps the PIL paste-through-self alpha law the legacy baker used.
    slots.forEach((slot, i) => {
      const slotStore = new ChunkStore(sheet.w, sheet.h);
      const sw = new PixelWriter(slotStore, slot.id ?? `slot_${i}`);
      const tile = tiles[i];
      for (let y = 0; y < tile.h; y++) {
        for (let x = 0; x < tile.w; x++) {
          const px = tile.data[y * tile.w + x];
          const a = (px >>> 24) & 255;
          if (a === 0) continue;
          sw.set(x + i * cellW, y, px);
        }
      }
      sw.finish();
      layers.push(mkLayer(slot.id ?? `slot_${i}`, slot.id ?? `slot_${i}`, slotStore, false));
    });
    return new Document({ name: obj.name ?? "sheet", canvas: { widthPx: sheet.w, heightPx: sheet.h, background: bg }, layers, activeLayerId: sheetLayer.id, ids });
  }

  // Standard recipes: one Document layer per recipe layer, same canvas.
  const ex = obj.export ?? {};
  const w = ex.canvas?.width ?? obj.canvas?.width ?? (Array.isArray(obj.canvas) ? obj.canvas[0] : 128);
  const h = ex.canvas?.height ?? obj.canvas?.height ?? (Array.isArray(obj.canvas) ? obj.canvas[1] : 128);
  const vp = ex.viewport ?? (obj.crop ? { x: 0, y: 0, w: obj.crop[0], h: obj.crop[1] } : null);

  let layerDefs = [];
  if (Array.isArray(obj.layers)) {
    layerDefs = obj.layers.filter((l) => !layerKey || l.id === layerKey).map((l) => ({ id: l.id, commands: l.commands ?? [] }));
  } else if (obj.layers && typeof obj.layers === "object") {
    layerDefs = Object.entries(obj.layers)
      .filter(([k]) => !layerKey || k === layerKey)
      .map(([k, v]) => ({ id: k, commands: Array.isArray(v) ? v : (v?.commands ?? []) }));
  } else if (layerKey === "mask" || layerKey === "mask_layers") {
    layerDefs = [{ id: "mask", commands: obj.mask_layers ?? [] }];
  } else if (obj.albedo_layers || obj.mask_layers) {
    if (!layerKey || layerKey === "albedo") layerDefs.push({ id: "albedo", commands: obj.albedo_layers ?? [] });
    if (layerKey === "mask" || layerKey === "mask_layers") layerDefs = [{ id: "mask", commands: obj.mask_layers ?? [] }];
    else if (!layerKey || layerKey === "albedo") { if (obj.mask_layers) layerDefs.push({ id: "mask", commands: obj.mask_layers }); }
  } else {
    for (const [key, value] of Object.entries(obj)) {
      if (Array.isArray(value) && value.length > 0 && value.every((v) => v && typeof v === "object" && (v.cmd || v.type))) {
        if (!layerKey || key === layerKey) layerDefs.push({ id: key, commands: value });
      }
    }
  }
  if (layerDefs.length === 0) {
    throw new DrawToolError("RECIPE_INVALID", "recipe has no layers to import");
  }

  const layers = layerDefs.map((def) => {
    const store = new ChunkStore(w, h);
    bakeCommandsIntoLayer(store, def.id, def.commands, palette, w, h);
    return mkLayer(def.id, def.id, store);
  });
  return new Document({ name: obj.name ?? "recipe", canvas: { widthPx: w, heightPx: h, background: bg, viewport: vp }, layers, activeLayerId: layers[0].id, ids });
}
