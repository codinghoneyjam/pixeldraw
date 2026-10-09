// <META - FILE SUMMARY - Build IndexedDB records from document data>

const META_KEY = "current";

// <META - ROLE : Stable chunk record key | L8-10>
export function chunkRecordKey(layerId, cx, cy) {
  return `${layerId}|${cy}|${cx}`;
}

// <META - ROLE : Build meta record from a document | L12-32>
export function buildMetaRecord(doc) {
  return {
    key: META_KEY,
    schema: 2,
    documentId: doc.id,
    name: doc.name,
    canvas: { widthPx: doc.canvas.widthPx, heightPx: doc.canvas.heightPx, background: doc.canvas.background },
    // `blend` must be written: it is a Layer constructor argument, so dropping it
    // here silently demotes a multiply layer to normal after a reload. Keep this
    // list in step with the Layer fields that are not pixel data.
    layers: doc.layers.map((l) => ({
      id: l.id, name: l.name, visible: l.visible, locked: l.locked,
      opacity: l.opacity, blend: l.blend ?? "normal",
    })),
    activeLayerId: doc.activeLayerId,
    updatedAt: Date.now(),
  };
}

// <META - ROLE : Build chunk records for given keys | L34-50>
export function buildChunkRecords(layer, keys) {
  const out = [];
  for (const entry of keys) {
    const data = layer.store.getChunk(entry.cx, entry.cy);
    if (!data) continue;
    const copy = data.slice();
    out.push({
      key: chunkRecordKey(layer.id, entry.cx, entry.cy),
      layerId: layer.id,
      cx: entry.cx,
      cy: entry.cy,
      data: copy.buffer,
    });
  }
  return out;
}

export const _INTERNALS = { META_KEY };
