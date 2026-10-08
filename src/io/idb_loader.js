// <META - FILE SUMMARY - Document restoration and meta validation from IndexedDB>
import { ChunkStore } from "../core/chunkstore.js";
import { isValidCanvasSize } from "../core/constants.js";
import { Document, IdGen } from "../model/document.js";
import { Layer } from "../features/layers/layer.js";
import { promisify } from "./idb_schema.js";
import { chunkRecordKey } from "./idb_record_builder.js";

const META_KEY = "current";

// <META - ROLE : Validate stored meta shape | L14-28>
export function isMetaValid(meta) {
  if (!meta || typeof meta !== "object") return false;
  if (meta.schema !== 2) return false;
  if (typeof meta.documentId !== "string" || meta.documentId.length === 0) return false;
  if (!meta.canvas || !isValidCanvasSize(meta.canvas.widthPx, meta.canvas.heightPx)) return false;
  if (!Array.isArray(meta.layers) || meta.layers.length < 1 || meta.layers.length > 64) return false;
  for (const l of meta.layers) {
    if (!l || typeof l.id !== "string" || typeof l.name !== "string") return false;
    if (typeof l.visible !== "boolean" || typeof l.locked !== "boolean") return false;
    if (typeof l.opacity !== "number" || !Number.isFinite(l.opacity) || l.opacity < 0 || l.opacity > 1) return false;
  }
  return true;
}

// <META - ROLE : Rebuild document from storage | L30-96>
export async function load(db) {
  const tx = db.transaction(["meta", "chunks"], "readonly");
  const metaStore = tx.objectStore("meta");
  const chunkStore = tx.objectStore("chunks");
  const meta = await promisify(metaStore.get(META_KEY));
  if (!meta) return null;
  if (!isMetaValid(meta)) {
    await clear(db);
    return null;
  }
  const records = await promisify(chunkStore.getAll());
  const layers = [];
  const ids = new IdGen();
  ids.seed(meta.layers.map((l) => l.id));
  for (const ml of meta.layers) {
    const store = new ChunkStore(meta.canvas.widthPx, meta.canvas.heightPx);
    const layer = new Layer(ml.id, ml.name, ml.visible, ml.locked, ml.opacity, "normal", store);
    layers.push(layer);
  }
  const byId = new Map(layers.map((l) => [l.id, l]));
  for (const rec of records) {
    const layer = byId.get(rec.layerId);
    if (!layer) continue;
    if (!Number.isInteger(rec.cx) || !Number.isInteger(rec.cy)) continue;
    if (!(rec.data instanceof ArrayBuffer) || rec.data.byteLength !== 4096) continue;
    layer.store.putChunk(rec.cx, rec.cy, new Uint8ClampedArray(rec.data.slice(0)));
  }
  const active = byId.has(meta.activeLayerId) ? meta.activeLayerId : layers[layers.length - 1].id;
  try {
    return new Document({
      id: meta.documentId,
      name: meta.name ?? "Untitled",
      canvas: { widthPx: meta.canvas.widthPx, heightPx: meta.canvas.heightPx, background: meta.canvas.background },
      layers,
      activeLayerId: active,
      ids,
    });
  } catch {
    await clear(db);
    return null;
  }
}

// <META - ROLE : Clear both stores | L98-108>
export async function clear(db) {
  const tx = db.transaction(["meta", "chunks"], "readwrite");
  await promisify(tx.objectStore("meta").clear());
  await promisify(tx.objectStore("chunks").clear());
}

export const _INTERNALS = { META_KEY };
