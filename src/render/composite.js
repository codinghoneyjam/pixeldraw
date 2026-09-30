// Display/export shared compositing (DOM-free, Node-importable).
// Contract: docs/render.md (composite.js). Background is NOT handled here.

import { CHUNK_PX, CHUNK_LEN } from "../core/constants.js";
import { over } from "../core/blend.js";
import { DrawToolError } from "../core/errors.js";

function assertDoc(doc) {
  if (!doc || !Array.isArray(doc.layers) || !doc.canvas
    || !Number.isFinite(doc.canvas.widthPx) || !Number.isFinite(doc.canvas.heightPx)) {
    throw new DrawToolError("INVALID_STATE", "compositeChunk needs { layers, canvas }");
  }
}

function assertOut(out) {
  if (!(out instanceof Uint8ClampedArray) || out.length !== CHUNK_LEN) {
    throw new DrawToolError("INVALID_STATE", "out must be Uint8ClampedArray(4096)");
  }
}

function visibleSources(doc, cx, cy) {
  const srcs = [];
  for (const layer of doc.layers) {
    if (!layer || layer.visible !== true) continue;
    const opacity = layer.opacity ?? 1;
    if (!(opacity > 0)) continue;
    const data = layer.store ? layer.store.getChunk(cx, cy) : null;
    if (!data) continue;
    srcs.push({ data, opacity });
  }
  return srcs;
}

export function compositeChunk(doc, cx, cy, out) {
  assertDoc(doc);
  assertOut(out);
  const srcs = visibleSources(doc, cx, cy);
  if (srcs.length === 0) {
    out.fill(0);
    return false;
  }
  let anyVisible = false;
  for (let i = 0; i < CHUNK_PX * CHUNK_PX; i++) {
    const o = i * 4;
    let dr = 0;
    let dg = 0;
    let db = 0;
    let da = 0;
    for (let l = 0; l < srcs.length; l++) {
      const data = srcs[l].data;
      const opacity = srcs[l].opacity;
      const sa8 = data[o + 3];
      if (sa8 === 0) continue;
      if (sa8 === 255 && opacity === 1) {
        dr = data[o];
        dg = data[o + 1];
        db = data[o + 2];
        da = 255;
        continue;
      }
      const mixed = over([dr, dg, db, da], [data[o], data[o + 1], data[o + 2], sa8], opacity);
      dr = mixed[0];
      dg = mixed[1];
      db = mixed[2];
      da = mixed[3];
    }
    out[o] = dr;
    out[o + 1] = dg;
    out[o + 2] = db;
    out[o + 3] = da;
    if (da !== 0) {
      anyVisible = true;
    } else {
      out[o] = 0;
      out[o + 1] = 0;
      out[o + 2] = 0;
    }
  }
  return anyVisible;
}

export function samplePixel(doc, x, y) {
  assertDoc(doc);
  const w = doc.canvas.widthPx;
  const h = doc.canvas.heightPx;
  if (!Number.isInteger(x) || !Number.isInteger(y) || x < 0 || y < 0 || x >= w || y >= h) {
    return [0, 0, 0, 0];
  }
  const cx = Math.floor(x / CHUNK_PX);
  const cy = Math.floor(y / CHUNK_PX);
  const o = ((y - cy * CHUNK_PX) * CHUNK_PX + (x - cx * CHUNK_PX)) * 4;
  let acc = [0, 0, 0, 0];
  for (const layer of doc.layers) {
    if (!layer || layer.visible !== true) continue;
    const opacity = layer.opacity ?? 1;
    if (!(opacity > 0)) continue;
    const data = layer.store ? layer.store.getChunk(cx, cy) : null;
    if (!data) continue;
    const sa8 = data[o + 3];
    if (sa8 === 0) continue;
    if (sa8 === 255 && opacity === 1) {
      acc = [data[o], data[o + 1], data[o + 2], 255];
      continue;
    }
    acc = over(acc, [data[o], data[o + 1], data[o + 2], sa8], opacity);
  }
  return acc;
}
