import { isValidCanvasSize } from "../core/constants.js";
import { DrawToolError } from "../core/errors.js";
import { EVENTS } from "../core/events.js";
import { Layer } from "./layer.js";
import { validateLayerName, validateLayerOpacity } from "./layer.js";
import {
  AddLayerCommand,
  MergeDownCommand,
  MoveLayerCommand,
  RemoveLayerCommand,
  ResizeCanvasCommand,
  SetLayerPropCommand,
} from "./commands.js";

export function addLayer(session, name) {
  const doc = session._requireDoc();
  const layerName = name ?? `Layer ${doc.layers.length + 1}`;
  const layer = Layer.create({
    id: doc.ids.next(),
    name: layerName,
    widthPx: doc.canvas.widthPx,
    heightPx: doc.canvas.heightPx,
  });
  session.execute(new AddLayerCommand(layer, doc.layers.length, { makeActive: true }));
  return layer.id;
}

export function duplicateLayer(session, id) {
  const doc = session._requireDoc();
  const src = doc.getLayer(id);
  const clone = src.cloneWith({ id: doc.ids.next(), name: `${src.name} copy` });
  session.execute(new AddLayerCommand(clone, doc.indexOf(id) + 1, { makeActive: true, label: "레이어 복제" }));
  return clone.id;
}

export function removeLayer(session, id) {
  session.execute(new RemoveLayerCommand(id));
}

export function moveLayer(session, id, toIndex) {
  const doc = session._requireDoc();
  const from = doc.indexOf(id);
  if (from === -1) throw new DrawToolError("LAYER_NOT_FOUND", `layer ${id} not found`);
  if (from === toIndex) return false;
  if (!Number.isInteger(toIndex) || toIndex < 0 || toIndex >= doc.layers.length) {
    throw new DrawToolError("OUT_OF_RANGE", `move index ${String(toIndex)} out of range`);
  }
  session.execute(new MoveLayerCommand(id, from, toIndex));
  return true;
}

export function mergeDown(session, id) {
  session.execute(new MergeDownCommand(id));
}

export function insertLayer(session, layer, index) {
  const doc = session._requireDoc();
  const at = index ?? doc.indexOf(doc.activeLayerId) + 1;
  session.execute(new AddLayerCommand(layer, at, { makeActive: true }));
  return layer.id;
}

export function resizeCanvas(session, w, h) {
  const doc = session._requireDoc();
  if (doc.canvas.widthPx === w && doc.canvas.heightPx === h) return false;
  if (!isValidCanvasSize(w, h)) {
    throw new DrawToolError("CANVAS_SIZE_INVALID", `invalid canvas size ${w}x${h}`);
  }
  session.execute(new ResizeCanvasCommand(w, h));
  return true;
}

export function renameLayer(session, id, name) {
  const doc = session._requireDoc();
  validateLayerName(name);
  const layer = doc.getLayer(id);
  if (layer.name === name) return false;
  session.execute(new SetLayerPropCommand(id, "name", layer.name, name, "레이어 이름 변경"));
  return true;
}

export function setLayerVisible(session, id, b) {
  const doc = session._requireDoc();
  const layer = doc.getLayer(id);
  const next = b === true;
  if (layer.visible === next) return false;
  session.execute(new SetLayerPropCommand(id, "visible", layer.visible, next, "레이어 표시 변경"));
  return true;
}

export function setLayerLocked(session, id, b) {
  const doc = session._requireDoc();
  const layer = doc.getLayer(id);
  const next = b === true;
  if (layer.locked === next) return false;
  session.execute(new SetLayerPropCommand(id, "locked", layer.locked, next, "레이어 잠금 변경"));
  return true;
}

export function setLayerOpacity(session, id, value, { final = false } = {}) {
  const doc = session._requireDoc();
  validateLayerOpacity(value);
  const layer = doc.getLayer(id);
  if (layer.opacity === value) {
    if (final) session._history.breakMerge();
    return false;
  }
  session.execute(new SetLayerPropCommand(id, "opacity", layer.opacity, value, "레이어 불투명도"));
  if (final) session._history.breakMerge();
  return true;
}

export function setActiveLayer(session, id) {
  const doc = session._requireDoc();
  doc.getLayer(id);
  if (doc.activeLayerId === id) return false;
  doc._setActive(id);
  session.dispatchEvent(
    new CustomEvent(EVENTS.LAYERS_CHANGED, { detail: { reason: "active", layerIds: [id] } }),
  );
  return true;
}
