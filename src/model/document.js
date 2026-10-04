import { isValidCanvasSize, MAX_LAYERS } from "../core/constants.js";
import { DrawToolError, ERROR_CODES } from "../core/errors.js";
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

// <META - ROLE : Validate a logical viewport against its canvas | L39-57>
/**
 * Validate an optional logical viewport. The stored canvas must stay a 32
 * multiple so ChunkStore / chunkKey / PixelWriter keep their alignment
 * invariant; a viewport is how an arbitrary export size is expressed without
 * disturbing that. Absent or undefined means "export the whole canvas".
 * @param {*} v viewport or null/undefined
 * @param {number} canvasW canvas width in px
 * @param {number} canvasH canvas height in px
 * @returns {{x:number,y:number,w:number,h:number}|null} normalized viewport
 */
export function validateViewport(v, canvasW, canvasH) {
  if (v === null || v === undefined) return null;
  if (typeof v !== "object" || Array.isArray(v)) {
    throw new DrawToolError(ERROR_CODES.VIEWPORT_INVALID, "viewport must be an object");
  }
  const { x, y, w, h } = v;
  for (const [k, n] of [["x", x], ["y", y], ["w", w], ["h", h]]) {
    if (!Number.isInteger(n)) {
      throw new DrawToolError(ERROR_CODES.VIEWPORT_INVALID, `viewport ${k} must be an integer`);
    }
  }
  if (w < 1 || h < 1) {
    throw new DrawToolError(ERROR_CODES.VIEWPORT_INVALID, `viewport size must be positive, got ${w}x${h}`);
  }
  if (x < 0 || y < 0 || x + w > canvasW || y + h > canvasH) {
    throw new DrawToolError(
      ERROR_CODES.VIEWPORT_OUT_OF_RANGE,
      `viewport ${x},${y} ${w}x${h} exceeds canvas ${canvasW}x${canvasH}`,
    );
  }
  return { x, y, w, h };
}

export class Document {
  constructor({ id, name, canvas, layers, activeLayerId, ids }) {
    if (!isValidCanvasSize(canvas.widthPx, canvas.heightPx)) {
      throw new DrawToolError("CANVAS_SIZE_INVALID", "invalid canvas size");
    }
    validateBackground(canvas.background);
    const viewport = validateViewport(canvas.viewport, canvas.widthPx, canvas.heightPx);
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
    this.canvas = {
      widthPx: canvas.widthPx,
      heightPx: canvas.heightPx,
      background: canvas.background,
      viewport: viewport === null ? null : { ...viewport },
    };
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

  // <META - ROLE : Set or clear the logical export viewport | L196-204>
  /**
   * Set the logical export viewport, or clear it with null/undefined to export
   * the whole canvas. Re-validated against the current canvas size, so a resize
   * that leaves the viewport out of range must clear or move it first.
   * @param {*} v viewport object, or null/undefined to clear
   * @returns {{x:number,y:number,w:number,h:number}|null} the stored viewport
   */
  _setViewport(v) {
    const vp = validateViewport(v, this.canvas.widthPx, this.canvas.heightPx);
    this.canvas.viewport = vp === null ? null : { ...vp };
    return this.canvas.viewport;
  }

  _rename(name) {
    validateLayerName(name);
    this.name = name;
  }
}

export function newDocument({ widthPx = 512, heightPx = 512, background = "transparent", viewport = null, name = "Untitled", id } = {}) {
  if (!isValidCanvasSize(widthPx, heightPx)) {
    throw new DrawToolError("CANVAS_SIZE_INVALID", `invalid canvas size ${widthPx}x${heightPx}`);
  }
  validateBackground(background);
  validateViewport(viewport, widthPx, heightPx);
  docCounter += 1;
  const ids = new IdGen();
  const firstId = ids.next();
  const layer = Layer.create({ id: firstId, name: "Layer 1", widthPx, heightPx });
  return new Document({
    id: id ?? `doc-${String(docCounter).padStart(4, "0")}`,
    name,
    canvas: { widthPx, heightPx, background, viewport },
    layers: [layer],
    activeLayerId: firstId,
    ids,
  });
}
