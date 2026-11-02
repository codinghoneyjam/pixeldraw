// <META - FILE SUMMARY - Document/layer JSON serialize + import per schema 2.0.0>
// Write: schema key order, bottom-first layers, (cy,cx) chunks, skip empty, 64-batches.
import { TILE_PX, UNIT_PX, CHUNK_PX, SCHEMA_VERSION } from "../core/constants.js";
import { ChunkStore, isAllZero } from "../core/chunkstore.js";
import { DrawToolError } from "../core/errors.js";
import { Document, IdGen } from "../features/document/document.js";
import { Layer } from "../features/layers/layer.js";
import { base64ToBytes, bytesToBase64 } from "./base64.js";
import { decodePng, encodePng } from "./png.js";
import { validateDocument, validateLayerFile } from "./validate.js";

// Barrel: the vector and recipe halves of import/export live in sibling modules.
// Re-exported so src/ui/actions/file_actions.js and tools/recipe/* keep one import site.
export { documentToVectorJson, layerToVectorJson, vectorJsonToDocument } from "./serialize_vector.js";
export { recipeJsonToDocument, shiftCommand } from "./serialize_recipe.js";

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
    // blend passes straight through: BLEND_MODES is real now, so there is nothing
    // to demote. validate_structural.js has already rejected unknown names.
    const store = new ChunkStore(widthPx, heightPx);
    const layer = new Layer(lj.layer_id, lj.name, lj.visible, lj.locked, lj.opacity, lj.blend, store);
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
  const store = new ChunkStore(doc.canvas.widthPx, doc.canvas.heightPx);
  const layer = new Layer(doc.ids.next(), lj.name, lj.visible, lj.locked, lj.opacity, lj.blend, store);
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
