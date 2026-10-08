// <META - FILE SUMMARY - Document/layer JSON serialize + import per schema 2.0.0>
// Write: schema key order, bottom-first layers, (cy,cx) chunks, skip empty, 64-batches.
import { TILE_PX, UNIT_PX, CHUNK_PX, SCHEMA_VERSION } from "../core/constants.js";
import { isAllZero } from "../core/chunkstore.js";
import { ChunkStore, PixelWriter } from "../core/chunkstore.js";
import { packRGBA } from "../core/pixel.js";
import { DrawToolError } from "../core/errors.js";
import { Document, IdGen } from "../model/document.js";
import { Layer } from "../features/layers/layer.js";
import { base64ToBytes, bytesToBase64 } from "./base64.js";
import { decodePng, encodePng } from "./png.js";
import { validateDocument, validateLayerFile } from "./validate.js";
import { applyCommand, renderTile } from "../../tools/recipe/render_tile.js";
import { assembleSheet } from "../core/raster/atlas.js";
import { isPantographProfile, isSurfaceManifest, pantographCommandsForLayers, surfaceCommandsFor } from "../../tools/recipe/schema.mjs";

const ENCODE_BATCH = 64;

// <META - ROLE : Zero out RGB of fully transparent pixels | L16-24>
function normalizeAlpha(rgba) {
  for (let i = 0; i < rgba.length; i += 4) {
    if (rgba[i + 3] === 0) {
      rgba[i] = 0;
      rgba[i + 1] = 0;
      rgba[i + 2] = 0;
    }
  }
}

// <META - ROLE : Collect non-empty chunks sorted by cy,cx | L26-42>
function collectChunks(layer) {
  const out = [];
  layer.store.forEachChunk((cx, cy, data) => {
    if (!isAllZero(data)) out.push({ cx, cy, data });
  });
  out.sort((a, b) => a.cy - b.cy || a.cx - b.cx);
  return out;
}

// <META - ROLE : Encode chunk list with batched progress | L44-60>
async function encodeChunks(list, onProgress) {
  const total = list.length;
  let done = 0;
  const encoded = new Array(total);
  for (let s = 0; s < total; s += ENCODE_BATCH) {
    const slice = list.slice(s, s + ENCODE_BATCH);
    const parts = await Promise.all(slice.map(async (c) => {
      const png = await encodePng(c.data, CHUNK_PX, CHUNK_PX);
      return { cx: c.cx, cy: c.cy, png: bytesToBase64(png) };
    }));
    for (let k = 0; k < parts.length; k++) encoded[s + k] = parts[k];
    done += parts.length;
    if (onProgress) onProgress(done, total);
  }
  return encoded;
}

// <META - ROLE : Build one raster layer JSON object | L62-84>
async function layerToJsonObject(layer, onProgress) {
  const list = collectChunks(layer);
  const chunks = await encodeChunks(list, onProgress);
  const obj = {
    layer_id: layer.id,
    name: layer.name,
    type: "raster",
    visible: layer.visible,
    locked: layer.locked,
    opacity: layer.opacity,
    blend: layer.blend,
    raster: { chunk_px: CHUNK_PX, encoding: "png_base64", chunks },
  };
  return { obj, count: list.length };
}

// <META - ROLE : Serialize whole document to schema JSON | L86-116>
export async function documentToJson(doc, { onProgress } = {}) {
  const perLayer = [];
  let total = 0;
  for (const layer of doc.layers) {
    const list = collectChunks(layer);
    perLayer.push(list);
    total += list.length;
  }
  let done = 0;
  const layers = [];
  const report = onProgress ? (d, t) => onProgress(d, t) : null;
  for (let i = 0; i < doc.layers.length; i++) {
    const layer = doc.layers[i];
    const chunks = await encodeChunks(perLayer[i], report ? (d) => report(done + d, total) : undefined);
    done += perLayer[i].length;
    layers.push({
      layer_id: layer.id,
      name: layer.name,
      type: "raster",
      visible: layer.visible,
      locked: layer.locked,
      opacity: layer.opacity,
      blend: layer.blend,
      raster: { chunk_px: CHUNK_PX, encoding: "png_base64", chunks },
    });
  }
  const obj = {
    schema_version: SCHEMA_VERSION,
    format: "draw_tool.document",
    document_id: doc.id,
  };
  if (doc.name !== undefined) obj.name = doc.name;
  obj.canvas = {
    tile_px: TILE_PX,
    unit_px: UNIT_PX,
    width_px: doc.canvas.widthPx,
    height_px: doc.canvas.heightPx,
    background: doc.canvas.background,
  };
  // Viewport is optional; omit the key entirely when unset so a document with
  // no viewport serializes exactly as it did before viewport support existed.
  if (doc.canvas.viewport) {
    obj.canvas.viewport = {
      x: doc.canvas.viewport.x,
      y: doc.canvas.viewport.y,
      w: doc.canvas.viewport.w,
      h: doc.canvas.viewport.h,
    };
  }
  obj.active_layer_id = doc.activeLayerId;
  obj.layers = layers;
  return obj;
}

// <META - ROLE : Serialize one layer to layer-file JSON | L118-140>
export async function layerToJson(doc, layerId) {
  const layer = doc.getLayer(layerId);
  const { obj } = await layerToJsonObject(layer);
  return {
    schema_version: SCHEMA_VERSION,
    format: "draw_tool.layer",
    source_canvas: {
      tile_px: TILE_PX,
      unit_px: UNIT_PX,
      width_px: doc.canvas.widthPx,
      height_px: doc.canvas.heightPx,
      background: doc.canvas.background,
    },
    layer: obj,
  };
}

// <META - ROLE : Decode one stored chunk with normalization | L142-162>
async function decodeStoredChunk(pngB64) {
  const bytes = base64ToBytes(pngB64);
  let dec;
  try {
    dec = await decodePng(bytes);
  } catch (e) {
    if (e instanceof DrawToolError) throw new DrawToolError("CHUNK_BAD_PNG", "chunk png decode failed");
    throw e;
  }
  if (dec.width !== 32 || dec.height !== 32) {
    throw new DrawToolError("CHUNK_BAD_PNG", "chunk png must be 32x32");
  }
  normalizeAlpha(dec.rgba);
  if (isAllZero(dec.rgba)) return null;
  return dec.rgba;
}

// <META - ROLE : Parse validated document JSON into a Document | L164-230>
export async function jsonToDocument(obj, { onProgress } = {}) {
  const res = await validateDocument(obj);
  if (!res.ok) {
    const first = res.errors[0];
    throw new DrawToolError(first.code, first.message, { errors: res.errors });
  }
  const vectors = obj.layers.filter((l) => l.type === "vector").map((l) => l.name);
  if (vectors.length > 0) {
    throw new DrawToolError("UNSUPPORTED_LAYER_TYPE", "vector layers are not supported", { layerNames: vectors });
  }
  const warnings = [];
  const widthPx = obj.canvas.width_px;
  const heightPx = obj.canvas.height_px;
  const background = obj.canvas.background ?? "transparent";
  const layers = [];
  const jobs = [];
  for (const lj of obj.layers) jobs.push(...lj.raster.chunks.map((c) => c));
  let done = 0;
  const total = jobs.length;
  for (const lj of obj.layers) {
    let blend = lj.blend;
    if (blend !== "normal") {
      warnings.push(`layer "${lj.name}" blend ${blend} demoted to normal`);
      blend = "normal";
    }
    const store = new ChunkStore(widthPx, heightPx);
    const layer = new Layer(lj.layer_id, lj.name, lj.visible, lj.locked, lj.opacity, blend, store);
    const decoded = [];
    for (let s = 0; s < lj.raster.chunks.length; s += ENCODE_BATCH) {
      const slice = lj.raster.chunks.slice(s, s + ENCODE_BATCH);
      const parts = await Promise.all(slice.map((c) => decodeStoredChunk(c.png)));
      for (let k = 0; k < slice.length; k++) {
        if (parts[k] !== null) decoded.push({ cx: slice[k].cx, cy: slice[k].cy, data: parts[k] });
      }
      done += slice.length;
      if (onProgress) onProgress(done, total);
    }
    for (const d of decoded) store.putChunk(d.cx, d.cy, d.data);
    layers.push(layer);
  }
  const ids = new IdGen();
  ids.seed(obj.layers.map((l) => l.layer_id));
  const active = obj.active_layer_id ?? layers[layers.length - 1].id;
  const document = new Document({
    id: obj.document_id,
    name: obj.name ?? "Untitled",
    canvas: { widthPx, heightPx, background, viewport: obj.canvas.viewport ?? null },
    layers,
    activeLayerId: active,
    ids,
  });
  return { document, warnings };
}

// <META - ROLE : Import a layer file without touching the document | L232-290>
export async function importLayerJson(doc, obj) {
  const res = await validateLayerFile(obj);
  if (!res.ok) {
    const first = res.errors[0];
    throw new DrawToolError(first.code, first.message, { errors: res.errors });
  }
  const warnings = [];
  const lj = obj.layer;
  if (obj.source_canvas.width_px !== doc.canvas.widthPx || obj.source_canvas.height_px !== doc.canvas.heightPx) {
    warnings.push(`source canvas ${obj.source_canvas.width_px}x${obj.source_canvas.height_px} differs from document ${doc.canvas.widthPx}x${doc.canvas.heightPx}`);
  }
  let blend = lj.blend;
  if (blend !== "normal") {
    warnings.push(`layer "${lj.name}" blend ${blend} demoted to normal`);
    blend = "normal";
  }
  const store = new ChunkStore(doc.canvas.widthPx, doc.canvas.heightPx);
  const layer = new Layer(doc.ids.next(), lj.name, lj.visible, lj.locked, lj.opacity, blend, store);
  let dropped = 0;
  if (lj.type === "vector") {
    throw new DrawToolError("UNSUPPORTED_LAYER_TYPE", "vector layers are not supported", { layerNames: [lj.name] });
  }
  for (let s = 0; s < lj.raster.chunks.length; s += ENCODE_BATCH) {
    const slice = lj.raster.chunks.slice(s, s + ENCODE_BATCH);
    const parts = await Promise.all(slice.map((c) => decodeStoredChunk(c.png)));
    for (let k = 0; k < slice.length; k++) {
      const c = slice[k];
      if (c.cx * 32 >= doc.canvas.widthPx || c.cy * 32 >= doc.canvas.heightPx) {
        dropped += 1;
        continue;
      }
      if (parts[k] !== null) store.putChunk(c.cx, c.cy, parts[k]);
    }
  }
  return { layer, dropped, warnings };
}

// <META - ROLE : Extract vector commands from Document layers | L255-340)>
export function documentToVectorJson(doc) {
  const result = {
    schema_version: SCHEMA_VERSION,
    format: "draw_tool.vector",
    canvas: {
      width_px: doc.canvas.widthPx,
      height_px: doc.canvas.heightPx,
      background: doc.canvas.background,
    },
    layers: [],
  };

  if (doc.canvas.viewport) {
    result.canvas.viewport = {
      x: doc.canvas.viewport.x,
      y: doc.canvas.viewport.y,
      w: doc.canvas.viewport.w,
      h: doc.canvas.viewport.h,
    };
  }

  for (const layer of doc.layers) {
    const commands = [];
    layer.store.forEachChunk((cx, cy, data) => {
      const shapes = extractShapesFromChunk(data, cx * CHUNK_PX, cy * CHUNK_PX);
      commands.push(...shapes);
    });
    result.layers.push({
      layer_id: layer.id,
      name: layer.name,
      visible: layer.visible,
      locked: layer.locked,
      opacity: layer.opacity,
      blend: layer.blend,
      commands,
    });
  }
  return result;
}

// <META - ROLE : Extract shapes from 32x32 chunk data | L342-420)>
function extractShapesFromChunk(data, offsetX, offsetY) {
  const commands = [];
  const width = CHUNK_PX;
  const height = CHUNK_PX;
  const visited = new Array(width * height).fill(false);

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const idx = (y * width + x) * 4;
      const r = data[idx];
      const g = data[idx + 1];
      const b = data[idx + 2];
      const a = data[idx + 3];
      if (a === 0 || visited[y * width + x]) continue;

      let maxX = x;
      let maxY = y;
      while (maxX + 1 < width && !visited[y * width + maxX + 1]) {
        const nextIdx = (y * width + maxX + 1) * 4;
        if (data[nextIdx + 3] === 0) break;
        if (data[nextIdx] !== r || data[nextIdx + 1] !== g || data[nextIdx + 2] !== b) break;
        maxX++;
      }
      let canExpand = true;
      while (canExpand && maxY + 1 < height) {
        for (let checkX = x; checkX <= maxX; checkX++) {
          const checkIdx = ((maxY + 1) * width + checkX) * 4;
          if (data[checkIdx + 3] === 0 || visited[(maxY + 1) * width + checkX]) { canExpand = false; break; }
          if (data[checkIdx] !== r || data[checkIdx + 1] !== g || data[checkIdx + 2] !== b) { canExpand = false; break; }
        }
        if (canExpand) maxY++;
      }
      for (let vy = y; vy <= maxY; vy++) {
        for (let vx = x; vx <= maxX; vx++) visited[vy * width + vx] = true;
      }
      const colorHex = `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
      const w = maxX - x + 1;
      const h = maxY - y + 1;
      if (w === 1 && h === 1) {
        commands.push({ cmd: "pixel", x: offsetX + x, y: offsetY + y, color: colorHex });
      } else {
        commands.push({ cmd: "rect", box: [[offsetX + x, offsetY + y], [offsetX + maxX, offsetY + maxY]], fill: colorHex });
      }
    }
  }
  return commands;
}

// <META - ROLE : Build Document from vector command JSON | L422-480)>
export async function vectorJsonToDocument(obj) {
  if (!obj || typeof obj !== "object") {
    throw new DrawToolError("VECTOR_INVALID", "vector JSON must be an object");
  }
  if (!obj.canvas || typeof obj.canvas !== "object") {
    throw new DrawToolError("VECTOR_INVALID", "vector JSON is missing 'canvas'");
  }
  const { width_px: widthPx, height_px: heightPx } = obj.canvas;
  if (!Number.isFinite(widthPx) || !Number.isFinite(heightPx) || widthPx <= 0 || heightPx <= 0) {
    throw new DrawToolError("VECTOR_INVALID", `invalid canvas size ${widthPx}x${heightPx}`);
  }
  if (!Array.isArray(obj.layers) || obj.layers.length === 0) {
    throw new DrawToolError("VECTOR_INVALID", "vector JSON must have a non-empty 'layers' array");
  }
  const background = obj.canvas.background ?? "transparent";
  const ids = new IdGen();
  const layers = [];
  const usedIds = [];
  let activeLayerId = null;
  for (const layerData of obj.layers) {
    const id = typeof layerData.layer_id === "string" && layerData.layer_id ? layerData.layer_id : `layer_${layers.length + 1}`;
    usedIds.push(id);
    const store = new ChunkStore(widthPx, heightPx);
    const layer = new Layer(
      id,
      typeof layerData.name === "string" ? layerData.name : `Layer ${layers.length + 1}`,
      layerData.visible !== false,
      layerData.locked === true,
      Number.isFinite(layerData.opacity) ? layerData.opacity : 1.0,
      typeof layerData.blend === "string" ? layerData.blend : "normal",
      store,
    );
    const writer = new PixelWriter(store, layer.id);
    const ctx = { writer, palette: obj.palette ?? {}, width: widthPx, height: heightPx };
    const commands = Array.isArray(layerData.commands) ? layerData.commands : [];
    for (const cmd of commands) {
      const resolvedCmd = { ...cmd };
      if (typeof resolvedCmd.fill === "string" && resolvedCmd.fill.startsWith("$")) {
        resolvedCmd.fill = obj.palette?.[resolvedCmd.fill.slice(1)] ?? "#000000";
      }
      if (typeof resolvedCmd.color === "string" && resolvedCmd.color.startsWith("$")) {
        resolvedCmd.color = obj.palette?.[resolvedCmd.color.slice(1)] ?? "#000000";
      }
      applyCommand(ctx, resolvedCmd);
    }
    writer.finish();
    layers.push(layer);
    if (activeLayerId === null) activeLayerId = layer.id;
  }
  ids.seed(usedIds);
  return new Document({
    id: obj.document_id,
    name: obj.name ?? "Untitled",
    canvas: { widthPx, heightPx, background, viewport: obj.canvas.viewport ?? null },
    layers,
    activeLayerId,
    ids,
  });
}

// <META - ROLE : Shift one point or [x,y]/{x,y} list by a slot offset>
function shiftPointList(pts, dx, dy) {
  return pts.map((p) => (Array.isArray(p) ? [p[0] + dx, p[1] + dy] : { x: p.x + dx, y: p.y + dy }));
}

// <META - ROLE : Translate the geometry fields of one recipe command>
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

// <META - ROLE : Render a command list into a fresh document layer>
function bakeCommandsIntoLayer(store, layerId, commands, palette, width, height) {
  const writer = new PixelWriter(store, layerId);
  const ctx = { writer, palette, width, height };
  for (const cmd of commands) applyCommand(ctx, cmd);
  writer.finish();
  return writer;
}

// <META - ROLE : Build a Document from a legacy recipe JSON (mirror of transpile branches)>
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
