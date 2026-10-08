import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { packRGBA } from "../src/core/pixel.js";
import { EVENTS } from "../src/core/events.js";
import { Layer } from "../src/features/layers/layer.js";
import { IdGen, newDocument } from "../src/model/document.js";
import { Session } from "../src/model/session.js";
import {
  AddLayerCommand,
  MoveLayerCommand,
  RemoveLayerCommand,
  ResizeCanvasCommand,
  SetLayerPropCommand,
} from "../src/features/layers/commands.js";
import { assertDrawError, collectEvents, paintPixel, snapshotStore } from "./helpers/model.js";

describe("B5.1 document invariants", () => {
  it("rejects removing the last layer", () => {
    const doc = newDocument({});
    assertDrawError(() => new RemoveLayerCommand(doc.activeLayerId).do(doc), "INVALID_STATE");
    assert.equal(doc.layers.length, 1);
  });

  it("rejects the 65th layer and duplicate ids", () => {
    const doc = newDocument({});
    for (let i = 1; i < 64; i++) {
      const layer = Layer.create({ id: doc.ids.next(), name: `L${i}`, widthPx: 512, heightPx: 512 });
      doc._insert(layer, doc.layers.length);
    }
    assert.equal(doc.layers.length, 64);
    const extra = Layer.create({ id: doc.ids.next(), name: "X", widthPx: 512, heightPx: 512 });
    assertDrawError(() => doc._insert(extra, 64), "LAYER_LIMIT");
    const dup = Layer.create({ id: doc.layers[0].id, name: "dup", widthPx: 512, heightPx: 512 });
    assertDrawError(() => doc._insert(dup, 0), "DUP_LAYER_ID");
  });

  it("rejects unknown active layer and bad canvas", () => {
    const doc = newDocument({});
    assertDrawError(() => doc._setActive("layer-9999"), "ACTIVE_LAYER_MISSING");
    assertDrawError(() => doc._setCanvasSize(33, 32), "CANVAS_SIZE_INVALID");
    assertDrawError(() => newDocument({ widthPx: 31, heightPx: 32 }), "CANVAS_SIZE_INVALID");
    assertDrawError(() => Layer.create({ id: "a", name: "   ", widthPx: 32, heightPx: 32 }), "INVALID_STATE");
    assertDrawError(() => Layer.create({ id: "a", name: "x".repeat(129), widthPx: 32, heightPx: 32 }), "INVALID_STATE");
  });
});

describe("B5.2 IdGen", () => {
  it("seeds from loaded ids and ignores non-standard ids", () => {
    const gen = new IdGen();
    gen.seed(["layer-0007", "background", "layer-0003", "shape-9"]);
    assert.equal(gen.next(), "layer-0008");
    assert.equal(gen.next(), "layer-0009");
  });

  it("pads to 4 digits then grows", () => {
    const gen = new IdGen();
    gen.seed(["layer-9999"]);
    assert.equal(gen.next(), "layer-10000");
  });
});

describe("B5.3 commands do/undo/do byte restore", () => {
  it("paint restores bytes exactly", () => {
    const doc = newDocument({ widthPx: 64, heightPx: 64 });
    const cmd = paintPixel(doc, doc.activeLayerId, 40, 10, packRGBA(255, 0, 0, 255));
    const afterApply = snapshotStore(doc.getLayer(doc.activeLayerId).store);
    cmd.undo(doc);
    assert.deepEqual(snapshotStore(doc.getLayer(doc.activeLayerId).store), []);
    cmd.do(doc);
    assert.deepEqual(snapshotStore(doc.getLayer(doc.activeLayerId).store), afterApply);
    assert.ok(cmd.byteSize() >= 4096);
  });

  it("remove restores the same object at the same index", () => {
    const doc = newDocument({});
    const second = Layer.create({ id: doc.ids.next(), name: "Second", widthPx: 512, heightPx: 512 });
    doc._insert(second, 1);
    const cmd = new RemoveLayerCommand(second.id);
    cmd.do(doc);
    assert.equal(doc.layers.length, 1);
    cmd.undo(doc);
    assert.equal(doc.layers.length, 2);
    assert.equal(doc.layers[1], second);
    assert.equal(doc.layers[1].id, second.id);
    cmd.do(doc);
    assert.equal(doc.layers.length, 1);
  });

  it("add/move/prop/resize round-trip", () => {
    const doc = newDocument({ widthPx: 64, heightPx: 64 });
    const before = snapshotStore(doc.getLayer(doc.activeLayerId).store);
    const layer = Layer.create({ id: doc.ids.next(), name: "Top", widthPx: 64, heightPx: 64 });
    const add = new AddLayerCommand(layer, 1);
    add.do(doc);
    add.undo(doc);
    assert.deepEqual(snapshotStore(doc.getLayer(doc.activeLayerId).store), before);
    assert.equal(doc.layers.length, 1);
    add.do(doc);
    assert.equal(doc.layers.length, 2);

    const move = new MoveLayerCommand(layer.id, 1, 0);
    move.do(doc);
    assert.equal(doc.layers[0].id, layer.id);
    move.undo(doc);
    assert.equal(doc.layers[1].id, layer.id);

    const prop = new SetLayerPropCommand(layer.id, "name", "Top", "Renamed", "x");
    prop.do(doc);
    assert.equal(doc.getLayer(layer.id).name, "Renamed");
    prop.undo(doc);
    assert.equal(doc.getLayer(layer.id).name, "Top");

    const paint = paintPixel(doc, layer.id, 60, 60, packRGBA(0, 255, 0, 255));
    paint.do(doc);
    const resize = new ResizeCanvasCommand(32, 32);
    resize.do(doc);
    assert.equal(doc.canvas.widthPx, 32);
    assert.equal(doc.getLayer(layer.id).store.getPixel(60, 60), 0);
    resize.undo(doc);
    assert.equal(doc.canvas.widthPx, 64);
    assert.equal(doc.getLayer(layer.id).store.getPixel(60, 60), packRGBA(0, 255, 0, 255));
  });
});

describe("B5.5 session", () => {
  it("emits layers -> pixels -> history order on record/execute", () => {
    const session = new Session();
    session.newDocument({ widthPx: 32, heightPx: 32 });
    const edit = session.beginEdit({});
    edit.writer.set(1, 1, packRGBA(255, 0, 0, 255));
    const { log, done } = collectEvents(session, [EVENTS.LAYERS_CHANGED, EVENTS.PIXELS_CHANGED, EVENTS.HISTORY_CHANGED]);
    edit.commit();
    done();
    const kinds = log.map((e) => e.type);
    assert.deepEqual(kinds, [EVENTS.PIXELS_CHANGED, EVENTS.PIXELS_CHANGED, EVENTS.HISTORY_CHANGED]);
    assert.equal(log[1].detail.layerId, session.doc.activeLayerId);
    assert.ok(Array.isArray(log[1].detail.chunks));
  });

  it("duplicate of painted layer emits layers/pixels/history in order", () => {
    const session = new Session();
    session.newDocument({ widthPx: 32, heightPx: 32 });
    const edit = session.beginEdit({});
    edit.writer.set(0, 0, packRGBA(1, 2, 3, 255));
    edit.commit();
    const { log, done } = collectEvents(session, [EVENTS.LAYERS_CHANGED, EVENTS.PIXELS_CHANGED, EVENTS.HISTORY_CHANGED]);
    session.duplicateLayer(session.doc.activeLayerId);
    done();
    assert.deepEqual(log.map((e) => e.type), [EVENTS.LAYERS_CHANGED, EVENTS.PIXELS_CHANGED, EVENTS.HISTORY_CHANGED]);
    assert.equal(log[0].detail.reason, "add");
    assert.equal(log[1].detail.all, true);
  });

  it("load/new emit only replaced + history", () => {
    const session = new Session();
    session.newDocument({});
    const { log, done } = collectEvents(session, [
      EVENTS.DOCUMENT_REPLACED,
      EVENTS.LAYERS_CHANGED,
      EVENTS.PIXELS_CHANGED,
      EVENTS.HISTORY_CHANGED,
    ]);
    session.newDocument({});
    session.loadDocument(newDocument({}));
    done();
    assert.deepEqual(log.map((e) => e.type), [
      EVENTS.DOCUMENT_REPLACED,
      EVENTS.HISTORY_CHANGED,
      EVENTS.DOCUMENT_REPLACED,
      EVENTS.HISTORY_CHANGED,
    ]);
  });

  it("locked/hidden beginEdit notifies and throws; undo ignored while editing", () => {
    const session = new Session();
    session.newDocument({ widthPx: 32, heightPx: 32 });
    const id = session.doc.activeLayerId;
    const statuses = [];
    session.addEventListener(EVENTS.STATUS_MESSAGE, (e) => statuses.push(e.detail));
    session.setLayerLocked(id, true);
    assertDrawError(() => session.beginEdit({}), "LAYER_LOCKED");
    assert.equal(statuses[statuses.length - 1].code, "LAYER_LOCKED");
    session.setLayerLocked(id, false);
    session.setLayerVisible(id, false);
    assertDrawError(() => session.beginEdit({}), "LAYER_HIDDEN");
    assert.equal(statuses[statuses.length - 1].code, "LAYER_HIDDEN");
    session.setLayerVisible(id, true);
    session.beginEdit({});
    assert.equal(session.isEditing, true);
    const { log, done } = collectEvents(session, [EVENTS.HISTORY_CHANGED]);
    assert.equal(session.undo(), false);
    assert.deepEqual(log, []);
    done();
  });

  it("validates settings and skips event when unchanged", () => {
    const session = new Session();
    let count = 0;
    session.addEventListener(EVENTS.SETTINGS_CHANGED, () => count++);
    assertDrawError(() => session.setSetting("penSize", 200), "OUT_OF_RANGE");
    assertDrawError(() => session.setSetting("primaryColor", "red"), "OUT_OF_RANGE");
    assertDrawError(() => session.setSetting("activeTool", "airbrush"), "OUT_OF_RANGE");
    assertDrawError(() => session.setSetting("nope", 1), "INVALID_STATE");
    assert.equal(session.setSetting("penSize", 1), false);
    assert.equal(count, 0);
    assert.equal(session.setSetting("penSize", 8), true);
    assert.equal(count, 1);
    assert.equal(session.settings.penSize, 8);
    session.setSetting("primaryColor", "#FF0000");
    assert.equal(session.settings.primaryColor, "#ff0000");
  });

  it("cancel restores pixels", () => {
    const session = new Session();
    session.newDocument({ widthPx: 32, heightPx: 32 });
    const id = session.doc.activeLayerId;
    const edit = session.beginEdit({});
    edit.writer.set(5, 5, packRGBA(9, 9, 9, 255));
    const { log, done } = collectEvents(session, [EVENTS.PIXELS_CHANGED]);
    edit.cancel();
    done();
    assert.equal(session.isEditing, false);
    assert.equal(session.doc.getLayer(id).store.getPixel(5, 5), 0);
    assert.equal(log.length, 1);
    assertDrawError(() => edit.commit(), "INVALID_STATE");
  });

  it("opacity drag merges until final", () => {
    const session = new Session();
    session.newDocument({ widthPx: 32, heightPx: 32 });
    const id = session.doc.activeLayerId;
    session.setLayerOpacity(id, 0.5, { final: false });
    session.setLayerOpacity(id, 0.3, { final: false });
    session.setLayerOpacity(id, 0.1, { final: true });
    assert.equal(session.doc.getLayer(id).opacity, 0.1);
    session.undo();
    assert.equal(session.doc.getLayer(id).opacity, 1);
    assert.equal(session.history.canUndo(), false);
  });
});