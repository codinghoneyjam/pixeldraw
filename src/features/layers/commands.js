// <META - FILE SUMMARY - Undo commands: paint chunk-set + layer structure/property history entries>
//
// Pixel-rewriting commands (merge-down, canvas resize) live in commands_pixel.js.

import { CHUNK_LEN } from "../../core/constants.js";
import { DrawToolError } from "../../core/errors.js";
import { changeSetBytes } from "../../core/chunkstore.js";
import { LAYER_PROP_FIELDS } from "./layer.js";

export { MergeDownCommand, ResizeCanvasCommand } from "./commands_pixel.js";

function chunkList(cs) {
  return cs.chunks.map((c) => ({ cx: c.cx, cy: c.cy }));
}

export class PaintCommand {
  constructor(changeSet, label = "연필") {
    this.changeSet = changeSet;
    this.label = label;
    this.type = "paint";
  }

  do(doc) {
    const layer = doc.getLayer(this.changeSet.layerId);
    for (const c of this.changeSet.chunks) {
      layer.store.putChunk(c.cx, c.cy, c.after);
    }
  }

  undo(doc) {
    const layer = doc.getLayer(this.changeSet.layerId);
    for (const c of this.changeSet.chunks) {
      layer.store.putChunk(c.cx, c.cy, c.before);
    }
  }

  byteSize() {
    return changeSetBytes(this.changeSet);
  }

  effects() {
    return { layers: null, pixels: [{ layerId: this.changeSet.layerId, chunks: chunkList(this.changeSet) }] };
  }
}

export class AddLayerCommand {
  constructor(layer, index, { makeActive = true, label = "레이어 추가" } = {}) {
    this.layer = layer;
    this.index = index;
    this.makeActive = makeActive;
    this.label = label;
    this.type = "add-layer";
    this._prevActiveId = null;
  }

  do(doc) {
    this._prevActiveId = doc.activeLayerId;
    doc._insert(this.layer, this.index);
    if (this.makeActive) doc._setActive(this.layer.id);
  }

  undo(doc) {
    doc._remove(this.layer.id);
    doc._setActive(this._prevActiveId);
  }

  byteSize() {
    return this.layer.store.chunkCount() * CHUNK_LEN;
  }

  effects() {
    const pixels = this.layer.store.chunkCount() > 0 ? [{ layerId: this.layer.id, all: true }] : [];
    return { layers: { reason: "add", layerIds: [this.layer.id] }, pixels };
  }
}

export class RemoveLayerCommand {
  constructor(layerId, label = "레이어 삭제") {
    this.layerId = layerId;
    this.label = label;
    this.type = "remove-layer";
    this._removed = null;
    this._prevActiveId = null;
  }

  do(doc) {
    this._prevActiveId = doc.activeLayerId;
    this._removed = doc._remove(this.layerId);
  }

  undo(doc) {
    doc._insert(this._removed.layer, this._removed.index);
    doc._setActive(this._prevActiveId);
  }

  byteSize() {
    return this._removed ? this._removed.layer.store.chunkCount() * CHUNK_LEN : 0;
  }

  effects() {
    const hasPixels = this._removed && this._removed.layer.store.chunkCount() > 0;
    const pixels = hasPixels ? [{ layerId: this.layerId, all: true }] : [];
    return { layers: { reason: "remove", layerIds: [this.layerId] }, pixels };
  }
}

export class MoveLayerCommand {
  constructor(layerId, from, to, label = "레이어 순서 변경") {
    this.layerId = layerId;
    this.from = from;
    this.to = to;
    this.label = label;
    this.type = "move-layer";
  }

  do(doc) {
    doc._move(this.layerId, this.to);
  }

  undo(doc) {
    doc._move(this.layerId, this.from);
  }

  byteSize() {
    return 0;
  }

  effects() {
    return { layers: { reason: "move", layerIds: [this.layerId] }, pixels: [] };
  }
}

export class SetLayerPropCommand {
  constructor(layerId, field, before, after, label = "레이어 속성 변경") {
    if (!LAYER_PROP_FIELDS.includes(field)) {
      throw new DrawToolError("INVALID_STATE", `unknown layer field ${field}`);
    }
    this.layerId = layerId;
    this.field = field;
    this.before = before;
    this.after = after;
    this.label = label;
    this.type = "set-layer-prop";
  }

  do(doc) {
    doc.getLayer(this.layerId).setProp(this.field, this.after);
  }

  undo(doc) {
    doc.getLayer(this.layerId).setProp(this.field, this.before);
  }

  byteSize() {
    return 0;
  }

  effects() {
    return { layers: { reason: "prop", layerIds: [this.layerId] }, pixels: [] };
  }

  mergeWith(prev) {
    if (!(prev instanceof SetLayerPropCommand)) return false;
    if (prev.layerId !== this.layerId || prev.field !== this.field) return false;
    if (this.field !== "opacity") return false;
    prev.after = this.after;
    return true;
  }
}