import { PixelWriter } from "../core/chunkstore.js";
import { isValidCanvasSize } from "../core/constants.js";
import { DrawToolError } from "../core/errors.js";
import { EVENTS, SETTING_KEYS } from "../core/events.js";
import { parseHex, toHex } from "../core/pixel.js";
import { newDocument as createDocument, Document } from "./document.js";
import { validateLayerName, validateLayerOpacity } from "./layer.js";
import { Layer } from "./layer.js";
import {
  AddLayerCommand,
  MergeDownCommand,
  MoveLayerCommand,
  PaintCommand,
  RemoveLayerCommand,
  ResizeCanvasCommand,
  SetLayerPropCommand,
} from "./commands.js";
import { UndoManager } from "./history.js";

export const TOOL_IDS = Object.freeze([
  "pen",
  "eraser",
  "eyedropper",
  "fill",
  "line",
  "rect",
  "rrect",
  "ellipse",
  "hand",
]);

const GRID_MODES = Object.freeze(["off", "unit", "tile", "pixel"]);
// "both" was removed: it flooded the shape's outer area with the BACKGROUND colour.
// Only the foreground may paint; the background slot is a buffer.
const SHAPE_FILLS = Object.freeze(["outline", "fill"]);

function validateColor(value) {
  const c = parseHex(value);
  if (c === null || c.a !== 255) {
    throw new DrawToolError("OUT_OF_RANGE", `invalid color ${String(value)}`);
  }
  return toHex(c.r, c.g, c.b);
}

function validateInt(value, min, max, what) {
  if (!Number.isInteger(value) || value < min || value > max) {
    throw new DrawToolError("OUT_OF_RANGE", `invalid ${what} ${String(value)}`);
  }
  return value;
}

export function validateSetting(key, value) {
  if (!SETTING_KEYS.includes(key)) {
    throw new DrawToolError("INVALID_STATE", `unknown setting ${String(key)}`);
  }
  if (key === "primaryColor" || key === "secondaryColor") return validateColor(value);
  if (key === "penSize") return validateInt(value, 1, 64, "penSize");
  if (key === "activeTool") {
    if (!TOOL_IDS.includes(value)) throw new DrawToolError("OUT_OF_RANGE", `unknown tool ${String(value)}`);
    return value;
  }
  if (key === "gridMode") {
    if (!GRID_MODES.includes(value)) throw new DrawToolError("OUT_OF_RANGE", `unknown gridMode ${String(value)}`);
    return value;
  }
  if (key === "snapUnit" || key === "shapeLockAspect") {
    if (typeof value !== "boolean") throw new DrawToolError("OUT_OF_RANGE", `invalid ${key}`);
    return value;
  }
  if (key === "shapeFill") {
    if (!SHAPE_FILLS.includes(value)) throw new DrawToolError("OUT_OF_RANGE", `unknown shapeFill ${String(value)}`);
    return value;
  }
  if (key === "shapeRadius") return validateInt(value, 0, 2048, "shapeRadius");
  throw new DrawToolError("INVALID_STATE", `unknown setting ${String(key)}`);
}

const DEFAULT_SETTINGS = Object.freeze({
  primaryColor: "#000000",
  secondaryColor: "#ffffff",
  penSize: 1,
  activeTool: "pen",
  gridMode: "off",
  snapUnit: false,
  shapeFill: "outline",
  shapeRadius: 8,
  shapeLockAspect: false,
});

export class Session extends EventTarget {
  constructor() {
    super();
    this._settings = { ...DEFAULT_SETTINGS };
    this._doc = null;
    this._history = new UndoManager();
    this._edit = null;
    this._lastCmd = null;
    this._history.subscribe(({ kind, command }) => {
      if (kind !== "clear") this._lastCmd = command;
    });
  }

  get doc() {
    return this._doc;
  }

  get history() {
    return this._history;
  }

  get settings() {
    return Object.freeze({ ...this._settings });
  }

  get isEditing() {
    return this._edit !== null;
  }

  _requireDoc() {
    if (!this._doc) throw new DrawToolError("INVALID_STATE", "no document loaded");
    return this._doc;
  }

  _historyState() {
    return {
      canUndo: this._history.canUndo(),
      canRedo: this._history.canRedo(),
      undoLabel: this._history.undoLabel(),
      redoLabel: this._history.redoLabel(),
      dirty: this._history.isDirty(),
    };
  }

  _emitHistoryChanged() {
    this.dispatchEvent(new CustomEvent(EVENTS.HISTORY_CHANGED, { detail: this._historyState() }));
  }

  _dispatchEffects(cmd) {
    const fx = cmd.effects();
    if (fx.layers) {
      this.dispatchEvent(
        new CustomEvent(EVENTS.LAYERS_CHANGED, {
          detail: { reason: fx.layers.reason, layerIds: fx.layers.layerIds.slice() },
        }),
      );
    }
    for (const p of fx.pixels ?? []) {
      const detail =
        p.all === true ? { layerId: p.layerId, all: true } : { layerId: p.layerId, chunks: p.chunks.slice() };
      this.dispatchEvent(new CustomEvent(EVENTS.PIXELS_CHANGED, { detail }));
    }
    this._emitHistoryChanged();
  }

  setSetting(key, value) {
    const next = validateSetting(key, value);
    if (this._settings[key] === next) return false;
    this._settings[key] = next;
    this.dispatchEvent(new CustomEvent(EVENTS.SETTINGS_CHANGED, { detail: { key, value: next } }));
    return true;
  }

  newDocument(opts = {}) {
    const doc = createDocument(opts);
    this._doc = doc;
    this._history.setDocument(doc);
    this._history.markSaved();
    this._edit = null;
    this.dispatchEvent(new CustomEvent(EVENTS.DOCUMENT_REPLACED, { detail: { document: doc } }));
    this._emitHistoryChanged();
    return doc;
  }

  loadDocument(doc, { markSaved = true } = {}) {
    if (!(doc instanceof Document)) {
      throw new DrawToolError("INVALID_STATE", "loadDocument requires a Document");
    }
    this._doc = doc;
    this._history.setDocument(doc);
    if (markSaved) this._history.markSaved();
    this._edit = null;
    this.dispatchEvent(new CustomEvent(EVENTS.DOCUMENT_REPLACED, { detail: { document: doc } }));
    this._emitHistoryChanged();
    return doc;
  }

  execute(cmd) {
    this._history.commit(cmd, { applied: false });
    this._dispatchEffects(cmd);
  }

  record(cmd) {
    this._history.commit(cmd, { applied: true });
    this._dispatchEffects(cmd);
  }

  undo() {
    if (this.isEditing) return false;
    const ok = this._history.undo();
    if (ok) this._dispatchEffects(this._lastCmd);
    return ok;
  }

  redo() {
    if (this.isEditing) return false;
    const ok = this._history.redo();
    if (ok) this._dispatchEffects(this._lastCmd);
    return ok;
  }

  addLayer(name) {
    const doc = this._requireDoc();
    const layerName = name ?? `Layer ${doc.layers.length + 1}`;
    const layer = Layer.create({ id: doc.ids.next(), name: layerName, widthPx: doc.canvas.widthPx, heightPx: doc.canvas.heightPx });
    this.execute(new AddLayerCommand(layer, doc.layers.length, { makeActive: true }));
    return layer.id;
  }

  duplicateLayer(id) {
    const doc = this._requireDoc();
    const src = doc.getLayer(id);
    const clone = src.cloneWith({ id: doc.ids.next(), name: `${src.name} copy` });
    this.execute(new AddLayerCommand(clone, doc.indexOf(id) + 1, { makeActive: true, label: "레이어 복제" }));
    return clone.id;
  }

  removeLayer(id) {
    this.execute(new RemoveLayerCommand(id));
  }

  moveLayer(id, toIndex) {
    const doc = this._requireDoc();
    const from = doc.indexOf(id);
    if (from === -1) throw new DrawToolError("LAYER_NOT_FOUND", `layer ${id} not found`);
    if (from === toIndex) return false;
    if (!Number.isInteger(toIndex) || toIndex < 0 || toIndex >= doc.layers.length) {
      throw new DrawToolError("OUT_OF_RANGE", `move index ${String(toIndex)} out of range`);
    }
    this.execute(new MoveLayerCommand(id, from, toIndex));
    return true;
  }

  mergeDown(id) {
    this.execute(new MergeDownCommand(id));
  }

  insertLayer(layer, index) {
    const doc = this._requireDoc();
    const at = index ?? doc.indexOf(doc.activeLayerId) + 1;
    this.execute(new AddLayerCommand(layer, at, { makeActive: true }));
    return layer.id;
  }

  resizeCanvas(w, h) {
    const doc = this._requireDoc();
    if (doc.canvas.widthPx === w && doc.canvas.heightPx === h) return false;
    if (!isValidCanvasSize(w, h)) {
      throw new DrawToolError("CANVAS_SIZE_INVALID", `invalid canvas size ${w}x${h}`);
    }
    this.execute(new ResizeCanvasCommand(w, h));
    return true;
  }

  renameLayer(id, name) {
    const doc = this._requireDoc();
    validateLayerName(name);
    const layer = doc.getLayer(id);
    if (layer.name === name) return false;
    this.execute(new SetLayerPropCommand(id, "name", layer.name, name, "레이어 이름 변경"));
    return true;
  }

  setLayerVisible(id, b) {
    const doc = this._requireDoc();
    const layer = doc.getLayer(id);
    const next = b === true;
    if (layer.visible === next) return false;
    this.execute(new SetLayerPropCommand(id, "visible", layer.visible, next, "레이어 표시 변경"));
    return true;
  }

  setLayerLocked(id, b) {
    const doc = this._requireDoc();
    const layer = doc.getLayer(id);
    const next = b === true;
    if (layer.locked === next) return false;
    this.execute(new SetLayerPropCommand(id, "locked", layer.locked, next, "레이어 잠금 변경"));
    return true;
  }

  setLayerOpacity(id, value, { final = false } = {}) {
    const doc = this._requireDoc();
    validateLayerOpacity(value);
    const layer = doc.getLayer(id);
    if (layer.opacity === value) {
      if (final) this._history.breakMerge();
      return false;
    }
    this.execute(new SetLayerPropCommand(id, "opacity", layer.opacity, value, "레이어 불투명도"));
    if (final) this._history.breakMerge();
    return true;
  }

  setActiveLayer(id) {
    const doc = this._requireDoc();
    doc.getLayer(id);
    if (doc.activeLayerId === id) return false;
    doc._setActive(id);
    this.dispatchEvent(new CustomEvent(EVENTS.LAYERS_CHANGED, { detail: { reason: "active", layerIds: [id] } }));
    return true;
  }

  beginEdit({ layerId, label = "연필" } = {}) {
    const doc = this._requireDoc();
    if (this.isEditing) throw new DrawToolError("INVALID_STATE", "already editing");
    const layer = doc.getLayer(layerId ?? doc.activeLayerId);
    if (layer.locked) {
      this.notify("warn", "레이어가 잠겨 있습니다", "LAYER_LOCKED");
      throw new DrawToolError("LAYER_LOCKED", "layer is locked");
    }
    if (!layer.visible) {
      this.notify("warn", "레이어가 숨겨져 있습니다", "LAYER_HIDDEN");
      throw new DrawToolError("LAYER_HIDDEN", "layer is hidden");
    }
    const writer = new PixelWriter(layer.store, layer.id);
    const session = this;
    const edit = {
      writer,
      label,
      flush() {
        const dirty = writer.takeDirty();
        if (dirty.length > 0) {
          session.dispatchEvent(
            new CustomEvent(EVENTS.PIXELS_CHANGED, { detail: { layerId: layer.id, chunks: dirty } }),
          );
        }
        return dirty;
      },
      commit(commitLabel) {
        if (session._edit !== edit) throw new DrawToolError("INVALID_STATE", "edit is not active");
        const cs = writer.finish();
        const dirty = writer.takeDirty();
        if (dirty.length > 0) {
          session.dispatchEvent(
            new CustomEvent(EVENTS.PIXELS_CHANGED, { detail: { layerId: layer.id, chunks: dirty } }),
          );
        }
        session._edit = null;
        if (cs) {
          session.record(new PaintCommand(cs, commitLabel ?? label));
          return true;
        }
        return false;
      },
      cancel() {
        if (session._edit !== edit) throw new DrawToolError("INVALID_STATE", "edit is not active");
        const dirty = writer.discard();
        if (dirty.length > 0) {
          session.dispatchEvent(
            new CustomEvent(EVENTS.PIXELS_CHANGED, { detail: { layerId: layer.id, chunks: dirty } }),
          );
        }
        session._edit = null;
      },
    };
    this._edit = edit;
    return edit;
  }

  notify(level, text, code) {
    this.dispatchEvent(new CustomEvent(EVENTS.STATUS_MESSAGE, { detail: { level, text, code } }));
  }

  emitToolState(tool, pending) {
    this.dispatchEvent(new CustomEvent(EVENTS.TOOL_STATE, { detail: { tool, pending } }));
  }
}
