import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { packRGBA, unpackRGBA } from "../src/core/pixel.js";
import { PixelWriter } from "../src/core/chunkstore.js";
import { over } from "../src/core/blend.js";
import { EVENTS } from "../src/core/events.js";
import { Layer } from "../src/model/layer.js";
import { IdGen, newDocument } from "../src/model/document.js";
import { UndoManager } from "../src/model/history.js";
import { Session } from "../src/model/session.js";
import {
  AddLayerCommand,
  MergeDownCommand,
  MoveLayerCommand,
  PaintCommand,
  RemoveLayerCommand,
  ResizeCanvasCommand,
  SetLayerPropCommand,
} from "../src/model/commands.js";

function assertDrawError(fn, code) {
  try {
    fn();
  } catch (err) {
    assert.equal(err.name, "DrawToolError", `expected DrawToolError, got ${err}`);
    assert.equal(err.code, code, `expected code ${code}, got ${err.code}`);
    return;
  }
  assert.fail(`expected DrawToolError(${code})`);
}

function snapshotStore(store) {
  const out = [];
  store.forEachChunk((cx, cy, data) => out.push([cx, cy, [...data]]));
  return out;
}

function paintPixel(doc, layerId, x, y, packed) {
  const layer = doc.getLayer(layerId);
  const writer = new PixelWriter(layer.store, layerId);
  writer.set(x, y, packed);
  const cs = writer.finish();
  assert.ok(cs, "expected non-null changeset");
  return new PaintCommand(cs, "연필");
}

function collectEvents(session, names) {
  const log = [];
  const offs = names.map((n) => {
    const fn = (e) => log.push({ type: n, detail: e.detail });
    session.addEventListener(n, fn);
    return () => session.removeEventListener(n, fn);
  });
  return { log, done: () => offs.forEach((f) => f()) };
}

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

describe("B5.4 history", () => {
  function opacityDoc() {
    const doc = newDocument({ widthPx: 32, heightPx: 32 });
    const hist = new UndoManager({});
    hist.setDocument(doc);
    return { doc, hist };
  }

  it("merges 30 opacity commits into one entry", () => {
    const { doc, hist } = opacityDoc();
    for (let i = 0; i < 30; i++) {
      const layer = doc.getLayer(doc.activeLayerId);
      hist.commit(new SetLayerPropCommand(doc.activeLayerId, "opacity", layer.opacity, (i + 1) / 30, "o"));
    }
    let n = 0;
    while (hist.undo()) n++;
    assert.equal(n, 1);
    assert.equal(doc.getLayer(doc.activeLayerId).opacity, 1);
  });

  it("breakMerge starts a new entry", () => {
    const { doc, hist } = opacityDoc();
    const id = doc.activeLayerId;
    hist.commit(new SetLayerPropCommand(id, "opacity", 1, 0.5, "o"));
    hist.breakMerge();
    hist.commit(new SetLayerPropCommand(id, "opacity", 0.5, 0.2, "o"));
    assert.equal(hist.undoLabel(), "o");
    hist.undo();
    assert.equal(doc.getLayer(id).opacity, 0.5);
    hist.undo();
    assert.equal(doc.getLayer(id).opacity, 1);
    assert.equal(hist.canUndo(), false);
  });

  it("evicts oldest under budget but keeps one", () => {
    const doc = newDocument({ widthPx: 32, heightPx: 32 });
    const hist = new UndoManager({ limitSteps: 3 });
    hist.setDocument(doc);
    for (let i = 0; i < 5; i++) {
      hist.commit(new SetLayerPropCommand(doc.activeLayerId, "name", `n${i}`, `n${i + 1}`, `c${i}`));
    }
    let n = 0;
    while (hist.undo()) n++;
    assert.equal(n, 3);
  });

  it("dirty/savedLost semantics", () => {
    const doc = newDocument({ widthPx: 32, heightPx: 32 });
    const hist = new UndoManager({ limitSteps: 2 });
    hist.setDocument(doc);
    assert.equal(hist.isDirty(), false);
    hist.commit(new SetLayerPropCommand(doc.activeLayerId, "name", "a", "b", "c0"));
    assert.equal(hist.isDirty(), true);
    hist.markSaved();
    assert.equal(hist.isDirty(), false);
    hist.commit(new SetLayerPropCommand(doc.activeLayerId, "name", "b", "c", "c1"));
    hist.commit(new SetLayerPropCommand(doc.activeLayerId, "name", "c", "d", "c2"));
    assert.equal(hist.isDirty(), true);
    hist.markSaved();
    assert.equal(hist.isDirty(), false);
  });

  it("commit clears redo and notifies in order, tolerating failures", () => {
    const doc = newDocument({ widthPx: 32, heightPx: 32 });
    const hist = new UndoManager({});
    hist.setDocument(doc);
    const order = [];
    hist.subscribe(() => order.push("first"));
    hist.subscribe(() => {
      throw new Error("boom");
    });
    hist.subscribe(() => order.push("third"));
    const errors = [];
    const orig = console.error;
    console.error = (...a) => errors.push(a);
    try {
      hist.commit(new SetLayerPropCommand(doc.activeLayerId, "name", "a", "b", "c"));
    } finally {
      console.error = orig;
    }
    assert.deepEqual(order, ["first", "third"]);
    assert.equal(errors.length, 1);
    hist.undo();
    assert.equal(hist.canRedo(), true);
    hist.commit(new SetLayerPropCommand(doc.activeLayerId, "name", "b", "c", "c2"));
    assert.equal(hist.canRedo(), false);
  });

  it("failed do does not mutate the stack", () => {
    const doc = newDocument({ widthPx: 32, heightPx: 32 });
    const hist = new UndoManager({});
    hist.setDocument(doc);
    hist.commit(new SetLayerPropCommand(doc.activeLayerId, "name", "a", "b", "ok"));
    const failing = {
      type: "x",
      label: "fail",
      do() {
        throw new Error("do-fail");
      },
      undo() {},
      byteSize() {
        return 0;
      },
      effects() {
        return { layers: null, pixels: [] };
      },
    };
    assert.throws(() => hist.commit(failing), /do-fail/);
    assert.equal(hist.undoLabel(), "ok");
    assert.equal(hist.canRedo(), false);
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

describe("B5.6 mergeDown", () => {
  it("matches golden composite cases and round-trips undo", () => {
    const session = new Session();
    session.newDocument({ widthPx: 32, heightPx: 32 });
    const lowerId = session.doc.activeLayerId;
    const lower = session.doc.getLayer(lowerId);
    lower.store._setPixel(0, 0, packRGBA(0, 0, 0, 255));
    const upperId = session.addLayer("upper");
    const upper = session.doc.getLayer(upperId);
    upper.store._setPixel(0, 0, packRGBA(255, 255, 255, 255));
    upper.opacity = 1;
    lower.opacity = 1;
    session.mergeDown(upperId);
    assert.deepEqual(
      unpackRGBA(session.doc.getLayer(lowerId).store.getPixel(0, 0)),
      over(over([0, 0, 0, 0], [0, 0, 0, 255], 1), [255, 255, 255, 255], 1),
    );
    assert.equal(session.doc.layers.length, 1);

    session.undo();
    assert.equal(session.doc.layers.length, 2);
    assert.deepEqual(unpackRGBA(session.doc.getLayer(lowerId).store.getPixel(0, 0)), [0, 0, 0, 255]);
    assert.deepEqual(unpackRGBA(session.doc.getLayer(upperId).store.getPixel(0, 0)), [255, 255, 255, 255]);
    session.redo();
    assert.equal(session.doc.layers.length, 1);
  });

  it("bakes opacity (golden white@0.5 over black) and sets opacity to 1", () => {
    const session = new Session();
    session.newDocument({ widthPx: 32, heightPx: 32 });
    const lowerId = session.doc.activeLayerId;
    session.doc.getLayer(lowerId).store._setPixel(3, 3, packRGBA(0, 0, 0, 255));
    const upperId = session.addLayer("upper");
    const upper = session.doc.getLayer(upperId);
    upper.store._setPixel(3, 3, packRGBA(255, 255, 255, 255));
    upper.opacity = 0.5;
    session.mergeDown(upperId);
    const got = unpackRGBA(session.doc.getLayer(lowerId).store.getPixel(3, 3));
    assert.deepEqual(got, [128, 128, 128, 255]);
    assert.equal(session.doc.getLayer(lowerId).opacity, 1);
    assert.equal(session.doc.activeLayerId, lowerId);
    session.undo();
    assert.equal(session.doc.getLayer(lowerId).opacity, 1);
    assert.equal(session.doc.getLayer(upperId).opacity, 0.5);
  });

  it("rejects locked/hidden and bottom layer", () => {
    const session = new Session();
    session.newDocument({ widthPx: 32, heightPx: 32 });
    const bottom = session.doc.activeLayerId;
    const top = session.addLayer("top");
    assertDrawError(() => session.mergeDown(bottom), "INVALID_STATE");
    session.setLayerLocked(top, true);
    assertDrawError(() => session.mergeDown(top), "LAYER_LOCKED");
    session.setLayerLocked(top, false);
    session.setLayerVisible(bottom, false);
    assertDrawError(() => session.mergeDown(top), "LAYER_HIDDEN");
  });

  it("direct MergeDownCommand class round-trips bytes", () => {
    const doc = newDocument({ widthPx: 32, heightPx: 32 });
    const lowerId = doc.activeLayerId;
    doc.getLayer(lowerId).store._setPixel(1, 1, packRGBA(255, 0, 0, 255));
    const upper = Layer.create({ id: doc.ids.next(), name: "U", widthPx: 32, heightPx: 32 });
    upper.store._setPixel(1, 1, packRGBA(0, 0, 255, 255));
    doc._insert(upper, 1);
    const cmd = new MergeDownCommand(upper.id);
    cmd.do(doc);
    assert.deepEqual(unpackRGBA(doc.getLayer(lowerId).store.getPixel(1, 1)), [0, 0, 255, 255]);
    cmd.undo(doc);
    assert.deepEqual(unpackRGBA(doc.getLayer(lowerId).store.getPixel(1, 1)), [255, 0, 0, 255]);
    assert.deepEqual(unpackRGBA(doc.getLayer(upper.id).store.getPixel(1, 1)), [0, 0, 255, 255]);
    cmd.do(doc);
    assert.deepEqual(unpackRGBA(doc.getLayer(lowerId).store.getPixel(1, 1)), [0, 0, 255, 255]);
  });
});
