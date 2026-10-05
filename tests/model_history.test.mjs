import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { packRGBA, unpackRGBA } from "../src/core/pixel.js";
import { over } from "../src/core/blend.js";
import { Layer } from "../src/model/layer.js";
import { newDocument } from "../src/model/document.js";
import { UndoManager } from "../src/model/history.js";
import { Session } from "../src/model/session.js";
import { MergeDownCommand, SetLayerPropCommand } from "../src/model/commands.js";
import { assertDrawError } from "./helpers/model.js";

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