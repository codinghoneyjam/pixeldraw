import { DrawToolError } from "../core/errors.js";
import { EVENTS } from "../core/events.js";
import { newDocument as createDocument, Document } from "./document.js";
import { UndoManager } from "./history.js";
import { validateSetting } from "./settings_validator.js";
import {
  addLayer,
  duplicateLayer,
  removeLayer,
  moveLayer,
  mergeDown,
  insertLayer,
  resizeCanvas,
  renameLayer,
  setLayerVisible,
  setLayerLocked,
  setLayerOpacity,
  setActiveLayer,
} from "../features/layers/layer_operations.js";
import { beginEdit } from "../features/layers/edit_session.js";

export { TOOL_IDS, validateSetting } from "./settings_validator.js";

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

  addLayer(name) { return addLayer(this, name); }
  duplicateLayer(id) { return duplicateLayer(this, id); }
  removeLayer(id) { return removeLayer(this, id); }
  moveLayer(id, toIndex) { return moveLayer(this, id, toIndex); }
  mergeDown(id) { return mergeDown(this, id); }
  insertLayer(layer, index) { return insertLayer(this, layer, index); }
  resizeCanvas(w, h) { return resizeCanvas(this, w, h); }
  renameLayer(id, name) { return renameLayer(this, id, name); }
  setLayerVisible(id, b) { return setLayerVisible(this, id, b); }
  setLayerLocked(id, b) { return setLayerLocked(this, id, b); }
  setLayerOpacity(id, value, opts) { return setLayerOpacity(this, id, value, opts); }
  setActiveLayer(id) { return setActiveLayer(this, id); }
  beginEdit(opts) { return beginEdit(this, opts); }

  notify(level, text, code) {
    this.dispatchEvent(new CustomEvent(EVENTS.STATUS_MESSAGE, { detail: { level, text, code } }));
  }

  emitToolState(tool, pending) {
    this.dispatchEvent(new CustomEvent(EVENTS.TOOL_STATE, { detail: { tool, pending } }));
  }
}
