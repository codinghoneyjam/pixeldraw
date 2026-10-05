// <META - FILE SUMMARY - Pixel-rewriting commands: MergeDownCommand, ResizeCanvasCommand>
//
// Split out of commands.js: these two rewrite chunk contents (not just layer
// structure), so they carry their own chunk/pixel imports.

import { CHUNK_LEN, isValidCanvasSize } from "../core/constants.js";
import { DrawToolError } from "../core/errors.js";
import { isAllZero } from "../core/chunkstore.js";
import { over } from "../core/blend.js";
import { packRGBA, unpackRGBA } from "../core/pixel.js";

// <META - ROLE : composite lower under upper through both opacities | L15-18>
function mergePixel(lowerArr, lowerOpacity, upperArr, upperOpacity) {
  const mid = over([0, 0, 0, 0], lowerArr, lowerOpacity);
  return over(mid, upperArr, upperOpacity);
}

export class MergeDownCommand {
  constructor(upperId, label = "아래로 병합") {
    this.upperId = upperId;
    this.label = label;
    this.type = "merge-down";
    this._lowerId = null;
    this._lowerPrevOpacity = 1;
    this._lowerBefore = null;
    this._upper = null;
    this._upperIndex = 0;
    this._prevActiveId = null;
  }

  _resolve(doc) {
    const upperIndex = doc.indexOf(this.upperId);
    if (upperIndex === -1) throw new DrawToolError("LAYER_NOT_FOUND", `layer ${this.upperId} not found`);
    if (upperIndex === 0) throw new DrawToolError("INVALID_STATE", "no layer below to merge into");
    return { upper: doc.layers[upperIndex], lower: doc.layers[upperIndex - 1], upperIndex };
  }

  _checkFlags(upper, lower) {
    if (upper.locked || lower.locked) {
      throw new DrawToolError("LAYER_LOCKED", "cannot merge locked layer");
    }
    if (!upper.visible || !lower.visible) {
      throw new DrawToolError("LAYER_HIDDEN", "cannot merge hidden layer");
    }
  }

  do(doc) {
    const { upper, lower, upperIndex } = this._resolve(doc);
    this._checkFlags(upper, lower);
    this._prevActiveId = doc.activeLayerId;
    this._lowerId = lower.id;
    this._lowerPrevOpacity = lower.opacity;
    this._upperIndex = upperIndex;
    this._lowerBefore = [];
    lower.store.forEachChunk((cx, cy, data) => {
      this._lowerBefore.push({ cx, cy, data: data.slice() });
    });
    const w = doc.canvas.widthPx;
    const h = doc.canvas.heightPx;
    for (let cy = 0; cy < h / 32; cy++) {
      for (let cx = 0; cx < w / 32; cx++) {
        const ld = lower.store.getChunk(cx, cy);
        const ud = upper.store.getChunk(cx, cy);
        if (!ld && !ud) continue;
        const out = new Uint8ClampedArray(4096);
        let touched = false;
        for (let i = 0; i < 1024; i++) {
          const lr = ld ? [ld[i * 4], ld[i * 4 + 1], ld[i * 4 + 2], ld[i * 4 + 3]] : [0, 0, 0, 0];
          const ur = ud ? [ud[i * 4], ud[i * 4 + 1], ud[i * 4 + 2], ud[i * 4 + 3]] : [0, 0, 0, 0];
          if (lr[3] === 0 && ur[3] === 0) continue;
          const [r, g, b, a] = mergePixel(lr, lower.opacity, ur, upper.opacity);
          if (a === 0) continue;
          const p = packRGBA(r, g, b, a);
          const [br, bg, bb, ba] = unpackRGBA(p);
          out[i * 4] = br;
          out[i * 4 + 1] = bg;
          out[i * 4 + 2] = bb;
          out[i * 4 + 3] = ba;
          touched = true;
        }
        lower.store.putChunk(cx, cy, touched && !isAllZero(out) ? out : null);
      }
    }
    lower.opacity = 1;
    this._upper = doc._remove(this.upperId).layer;
    doc._setActive(this._lowerId);
  }

  undo(doc) {
    const lower = doc.getLayer(this._lowerId);
    for (let cy = 0; cy < lower.store.chunksY; cy++) {
      for (let cx = 0; cx < lower.store.chunksX; cx++) {
        lower.store.putChunk(cx, cy, null);
      }
    }
    for (const c of this._lowerBefore) {
      lower.store.putChunk(c.cx, c.cy, c.data);
    }
    lower.opacity = this._lowerPrevOpacity;
    doc._insert(this._upper, this._upperIndex);
    doc._setActive(this._prevActiveId);
  }

  byteSize() {
    const lowerBytes = this._lowerBefore ? this._lowerBefore.length * CHUNK_LEN : 0;
    const upperBytes = this._upper ? this._upper.store.chunkCount() * CHUNK_LEN : 0;
    return lowerBytes + upperBytes;
  }

  effects() {
    return {
      layers: { reason: "remove", layerIds: [this.upperId] },
      pixels: [{ layerId: this._lowerId ?? "", all: true }],
    };
  }
}

export class ResizeCanvasCommand {
  constructor(newW, newH, label = "캔버스 크기 변경") {
    if (!isValidCanvasSize(newW, newH)) {
      throw new DrawToolError("CANVAS_SIZE_INVALID", `invalid canvas size ${newW}x${newH}`);
    }
    this.newW = newW;
    this.newH = newH;
    this.label = label;
    this.type = "resize-canvas";
    this._oldW = 0;
    this._oldH = 0;
    this._cut = [];
    this._captured = false;
    this._layerIds = [];
  }

  do(doc) {
    if (!this._captured) {
      this._oldW = doc.canvas.widthPx;
      this._oldH = doc.canvas.heightPx;
      this._captured = true;
    }
    this._cut = [];
    this._layerIds = doc.layers.map((l) => l.id);
    for (const layer of doc.layers) {
      const removed = layer.store.resizeTo(this.newW, this.newH);
      if (removed.length > 0) this._cut.push({ layerId: layer.id, chunks: removed });
    }
    doc._setCanvasSize(this.newW, this.newH);
  }

  undo(doc) {
    doc._setCanvasSize(this._oldW, this._oldH);
    for (const layer of doc.layers) {
      layer.store.resizeTo(this._oldW, this._oldH);
    }
    for (const entry of this._cut) {
      const layer = doc.getLayer(entry.layerId);
      for (const c of entry.chunks) {
        layer.store.putChunk(c.cx, c.cy, c.data);
      }
    }
  }

  byteSize() {
    let total = 0;
    for (const entry of this._cut) {
      total += entry.chunks.length * CHUNK_LEN;
    }
    return total;
  }

  effects() {
    return { layers: null, pixels: this._layerIds.map((id) => ({ layerId: id, all: true })) };
  }
}