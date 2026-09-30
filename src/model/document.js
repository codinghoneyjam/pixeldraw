import { isValidCanvasSize, MAX_LAYERS } from "../core/constants.js";
import { DrawToolError } from "../core/errors.js";
import { parseHex } from "../core/pixel.js";
import { Layer, validateLayerName } from "./layer.js";

const LAYER_ID_RE = /^layer-(\d+)$/;

export class IdGen {
  constructor() {
    this._counter = 0;
  }

  seed(ids) {
    for (const id of ids) {
      const m = LAYER_ID_RE.exec(id);
      if (m) {
        const n = parseInt(m[1], 10);
        if (n > this._counter) this._counter = n;
      }
    }
  }

  next() {
    this._counter += 1;
    return `layer-${String(this._counter).padStart(4, "0")}`;
  }
}

let docCounter = 0;

export function validateBackground(v) {
  if (v === "transparent") return;
  const c = parseHex(v);
  if (c === null) {
    throw new DrawToolError("INVALID_STATE", "background must be transparent or hex color");
  }
}

export class Document {
  constructor({ id, name, canvas, layers, activeLayerId, ids }) {
    if (!isValidCanvasSize(canvas.widthPx, canvas.heightPx)) {
      throw new DrawToolError("CANVAS_SIZE_INVALID", "invalid canvas size");
    }
    validateBackground(canvas.background);
    if (!Array.isArray(layers) || layers.length < 1 || layers.length > MAX_LAYERS) {
      throw new DrawToolError("LAYER_LIMIT", "document must hold 1-64 layers");
    }
    const seen = new Set();
    for (const layer of layers) {
      if (!(layer instanceof Layer)) {
        throw new DrawToolError("INVALID_STATE", "layers must be Layer instances");
      }
      if (seen.has(layer.id)) {
        throw new DrawToolError("DUP_LAYER_ID", `duplicate layer id ${layer.id}`);
      }
      seen.add(layer.id);
      if (layer.store.widthPx !== canvas.widthPx || layer.store.heightPx !== canvas.heightPx) {
        throw new DrawToolError("INVALID_STATE", "layer store size must equal canvas size");
      }
    }
    if (!seen.has(activeLayerId)) {
      throw new DrawToolError("ACTIVE_LAYER_MISSING", `active layer ${activeLayerId} missing`);
    }
    this.id = id;
    this.name = name;
    this.canvas = { widthPx: canvas.widthPx, heightPx: canvas.heightPx, background: canvas.background };
    this.layers = layers.slice();
    this.activeLayerId = activeLayerId;
    this.ids = ids;
  }

  getLayer(id) {
    const layer = this.findLayer(id);
    if (!layer) throw new DrawToolError("LAYER_NOT_FOUND", `layer ${id} not found`);
    return layer;
  }

  findLayer(id) {
    for (const layer of this.layers) {
      if (layer.id === id) return layer;
    }
    return null;
  }

  indexOf(id) {
    for (let i = 0; i < this.layers.length; i++) {
      if (this.layers[i].id === id) return i;
    }
    return -1;
  }

  get activeLayer() {
    return this.getLayer(this.activeLayerId);
  }

  _checkStoreFits(layer) {
    if (layer.store.widthPx !== this.canvas.widthPx || layer.store.heightPx !== this.canvas.heightPx) {
      throw new DrawToolError("INVALID_STATE", "layer store size must equal canvas size");
    }
  }

  _insert(layer, index) {
    if (!(layer instanceof Layer)) {
      throw new DrawToolError("INVALID_STATE", "can only insert Layer instances");
    }
    if (this.findLayer(layer.id)) {
      throw new DrawToolError("DUP_LAYER_ID", `duplicate layer id ${layer.id}`);
    }
    if (this.layers.length >= MAX_LAYERS) {
      throw new DrawToolError("LAYER_LIMIT", "layer limit 64 reached");
    }
    this._checkStoreFits(layer);
    const at = index === undefined ? this.layers.length : index;
    if (!Number.isInteger(at) || at < 0 || at > this.layers.length) {
      throw new DrawToolError("OUT_OF_RANGE", `insert index ${String(index)} out of range`);
    }
    this.layers.splice(at, 0, layer);
  }

  _remove(id) {
    const index = this.indexOf(id);
    if (index === -1) throw new DrawToolError("LAYER_NOT_FOUND", `layer ${id} not found`);
    if (this.layers.length <= 1) {
      throw new DrawToolError("INVALID_STATE", "cannot remove the last layer");
    }
    const [layer] = this.layers.splice(index, 1);
    if (this.activeLayerId === id) {
      const next = this.layers[Math.min(index, this.layers.length - 1)];
      this.activeLayerId = next.id;
    }
    return { layer, index };
  }

  _move(id, toIndex) {
    const from = this.indexOf(id);
    if (from === -1) throw new DrawToolError("LAYER_NOT_FOUND", `layer ${id} not found`);
    if (!Number.isInteger(toIndex) || toIndex < 0 || toIndex >= this.layers.length) {
      throw new DrawToolError("OUT_OF_RANGE", `move index ${String(toIndex)} out of range`);
    }
    const [layer] = this.layers.splice(from, 1);
    this.layers.splice(toIndex, 0, layer);
  }

  _setActive(id) {
    if (!this.findLayer(id)) {
      throw new DrawToolError("ACTIVE_LAYER_MISSING", `active layer ${id} missing`);
    }
    this.activeLayerId = id;
  }

  _setCanvasSize(w, h) {
    if (!isValidCanvasSize(w, h)) {
      throw new DrawToolError("CANVAS_SIZE_INVALID", `invalid canvas size ${w}x${h}`);
    }
    this.canvas.widthPx = w;
    this.canvas.heightPx = h;
  }

  _setBackground(v) {
    validateBackground(v);
    this.canvas.background = v;
  }

  _rename(name) {
    validateLayerName(name);
    this.name = name;
  }
}

export function newDocument({ widthPx = 512, heightPx = 512, background = "transparent", name = "Untitled", id } = {}) {
  if (!isValidCanvasSize(widthPx, heightPx)) {
    throw new DrawToolError("CANVAS_SIZE_INVALID", `invalid canvas size ${widthPx}x${heightPx}`);
  }
  validateBackground(background);
  docCounter += 1;
  const ids = new IdGen();
  const firstId = ids.next();
  const layer = Layer.create({ id: firstId, name: "Layer 1", widthPx, heightPx });
  return new Document({
    id: id ?? `doc-${String(docCounter).padStart(4, "0")}`,
    name,
    canvas: { widthPx, heightPx, background },
    layers: [layer],
    activeLayerId: firstId,
    ids,
  });
}
