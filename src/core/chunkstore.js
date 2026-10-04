import { CHUNK_LEN, CHUNK_PX, chunkKey, chunkCoords, isValidCanvasSize } from "./constants.js";
import { DrawToolError } from "./errors.js";

export function isAllZero(data) {
  const view = new Uint32Array(data.buffer, data.byteOffset, data.length / 4);
  for (let i = 0; i < view.length; i++) {
    if (view[i] !== 0) return false;
  }
  return true;
}

function checkChunkRange(store, cx, cy) {
  if (!Number.isInteger(cx) || !Number.isInteger(cy)) return false;
  return cx >= 0 && cy >= 0 && cx < store.chunksX && cy < store.chunksY;
}

export class ChunkStore {
  constructor(widthPx, heightPx) {
    if (!isValidCanvasSize(widthPx, heightPx)) {
      throw new DrawToolError("CANVAS_SIZE_INVALID", `invalid canvas size ${widthPx}x${heightPx}`);
    }
    this.widthPx = widthPx;
    this.heightPx = heightPx;
    this.chunksX = widthPx / CHUNK_PX;
    this.chunksY = heightPx / CHUNK_PX;
    this._map = new Map();
  }

  _offset(x, y) {
    const lx = x - Math.floor(x / CHUNK_PX) * CHUNK_PX;
    const ly = y - Math.floor(y / CHUNK_PX) * CHUNK_PX;
    return (ly * CHUNK_PX + lx) * 4;
  }

  getPixel(x, y) {
    if (x < 0 || y < 0 || x >= this.widthPx || y >= this.heightPx) return 0;
    const data = this._map.get(chunkKey(x >> 5, y >> 5));
    if (!data) return 0;
    const o = this._offset(x, y);
    return (data[o] | (data[o + 1] << 8) | (data[o + 2] << 16) | (data[o + 3] << 24)) >>> 0;
  }

  _setPixel(x, y, packed) {
    if (x < 0 || y < 0 || x >= this.widthPx || y >= this.heightPx) return;
    const key = chunkKey(x >> 5, y >> 5);
    let data = this._map.get(key);
    if (packed === 0) {
      if (!data) return;
      const o = this._offset(x, y);
      data[o] = 0;
      data[o + 1] = 0;
      data[o + 2] = 0;
      data[o + 3] = 0;
      return;
    }
    if (!data) {
      data = new Uint8ClampedArray(CHUNK_LEN);
      this._map.set(key, data);
    }
    const o = this._offset(x, y);
    const p = packed >>> 0;
    data[o] = p & 0xff;
    data[o + 1] = (p >>> 8) & 0xff;
    data[o + 2] = (p >>> 16) & 0xff;
    data[o + 3] = (p >>> 24) & 0xff;
  }

  getChunk(cx, cy) {
    const data = this._map.get(chunkKey(cx, cy));
    return data === undefined ? null : data;
  }

  copyChunk(cx, cy) {
    const data = this._map.get(chunkKey(cx, cy));
    return data === undefined ? null : data.slice();
  }

  putChunk(cx, cy, data) {
    if (!checkChunkRange(this, cx, cy)) {
      throw new DrawToolError("OUT_OF_RANGE", `chunk (${cx},${cy}) out of canvas`);
    }
    if (data === null) {
      this._map.delete(chunkKey(cx, cy));
      return;
    }
    if (!(data instanceof Uint8ClampedArray) || data.length !== CHUNK_LEN) {
      throw new DrawToolError("OUT_OF_RANGE", "chunk data must be Uint8ClampedArray(4096)");
    }
    this._map.set(chunkKey(cx, cy), data.slice());
  }

  forEachChunk(fn) {
    const keys = [...this._map.keys()].sort((a, b) => a - b);
    for (const key of keys) {
      const { cx, cy } = chunkCoords(key);
      fn(cx, cy, this._map.get(key));
    }
  }

  chunkCount() {
    return this._map.size;
  }

  pruneEmpty() {
    let removed = 0;
    for (const [key, data] of this._map) {
      if (isAllZero(data)) {
        this._map.delete(key);
        removed++;
      }
    }
    return removed;
  }

  clone() {
    const out = new ChunkStore(this.widthPx, this.heightPx);
    for (const [key, data] of this._map) {
      out._map.set(key, data.slice());
    }
    return out;
  }

  resizeTo(newW, newH) {
    if (!isValidCanvasSize(newW, newH)) {
      throw new DrawToolError("CANVAS_SIZE_INVALID", `invalid canvas size ${newW}x${newH}`);
    }
    const removed = [];
    const nx = newW / CHUNK_PX;
    const ny = newH / CHUNK_PX;
    const keys = [...this._map.keys()].sort((a, b) => a - b);
    for (const key of keys) {
      const { cx, cy } = chunkCoords(key);
      if (cx >= nx || cy >= ny) {
        removed.push({ cx, cy, data: this._map.get(key).slice() });
        this._map.delete(key);
      }
    }
    this.widthPx = newW;
    this.heightPx = newH;
    this.chunksX = nx;
    this.chunksY = ny;
    return removed;
  }
}

export class PixelWriter {
  constructor(store, layerId) {
    this.store = store;
    this.layerId = layerId;
    this._before = new Map();
    this._dirty = [];
    this._dirtySet = new Set();
    this._done = false;
  }

  _touch(x, y) {
    const cx = x >> 5;
    const cy = y >> 5;
    const key = chunkKey(cx, cy);
    if (!this._before.has(key)) {
      this._before.set(key, this.store.copyChunk(cx, cy));
    }
    if (!this._dirtySet.has(key)) {
      this._dirtySet.add(key);
      this._dirty.push({ cx, cy });
    }
  }

  set(x, y, packed) {
    if (this._done) {
      throw new DrawToolError("INVALID_STATE", "PixelWriter is finished");
    }
    if (x < 0 || y < 0 || x >= this.store.widthPx || y >= this.store.heightPx) return;
    this._touch(x, y);
    this.store._setPixel(x, y, packed);
  }

  get(x, y) {
    return this.store.getPixel(x, y);
  }

  takeDirty() {
    const out = this._dirty.slice().sort((a, b) => a.cy - b.cy || a.cx - b.cx);
    this._dirty = [];
    this._dirtySet.clear();
    return out;
  }

  finish() {
    if (this._done) {
      throw new DrawToolError("INVALID_STATE", "PixelWriter is finished");
    }
    this._done = true;
    const chunks = [];
    const keys = [...this._before.keys()].sort((a, b) => a - b);
    for (const key of keys) {
      const { cx, cy } = chunkCoords(key);
      const before = this._before.get(key);
      let after = this.store.copyChunk(cx, cy);
      if (after !== null && isAllZero(after)) {
        after = null;
        this.store.putChunk(cx, cy, null);
      }
      if (chunksEqual(before, after)) continue;
      chunks.push({ cx, cy, before, after });
    }
    if (chunks.length === 0) return null;
    return { layerId: this.layerId, chunks };
  }

  discard() {
    if (this._done) {
      throw new DrawToolError("INVALID_STATE", "PixelWriter is finished");
    }
    this._done = true;
    for (const [key, before] of this._before) {
      const { cx, cy } = chunkCoords(key);
      this.store.putChunk(cx, cy, before);
    }
    return this.takeDirty();
  }
}

function chunksEqual(a, b) {
  if (a === null && b === null) return true;
  if (a === null || b === null) return false;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

export function changeSetBytes(cs) {
  if (!cs) return 0;
  let total = 0;
  for (const c of cs.chunks) {
    if (c.before !== null && c.before !== undefined) total += CHUNK_LEN;
    if (c.after !== null && c.after !== undefined) total += CHUNK_LEN;
  }
  return total;
}
