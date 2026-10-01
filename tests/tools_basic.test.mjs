import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { brushFootprint } from "../src/core/brush.js";
import { EVENTS } from "../src/core/events.js";
import { packRGBA, parseHex, unpackRGBA } from "../src/core/pixel.js";
import { Session } from "../src/model/session.js";
import { ToolManager } from "../src/tools/tool_manager.js";
import { PenTool } from "../src/tools/pen.js";
import { EyedropperTool } from "../src/tools/eyedropper.js";
import { FillTool, floodFillScanline } from "../src/tools/fill.js";
import { HandTool } from "../src/tools/hand.js";
import { WheelAccumulator } from "../src/tools/input_controller.js";
import { toolEvent } from "./helpers/tool_event.js";

// Cursors are inline SVG data URLs with a CSS keyword fallback
// (`url("data:image/svg+xml,...") <hx> <hy>, <fallback>`). Docs treat SVG cursors
// as normative, so assert the fallback keyword and URL shape rather than the
// exact payload, which would break on any visual tweak.
const cursorFallback = (cursor) => {
  assert.match(cursor, /^url\("data:image\/svg\+xml,/, "cursor must be an SVG data URL");
  return cursor.slice(cursor.lastIndexOf(",") + 1).trim();
};

const G = JSON.parse(readFileSync(new URL("./fixtures/raster_golden.json", import.meta.url)));

function makeSession(w = 64, h = 64) {
  const s = new Session();
  s.newDocument({ widthPx: w, heightPx: h });
  return s;
}

function envFor(session) {
  return { session, getView: () => ({ zoom: 1, offsetX: 0, offsetY: 0 }), requestRender: () => {} };
}

function snapshotDoc(doc) {
  const out = [];
  for (const layer of doc.layers) {
    const chunks = [];
    layer.store.forEachChunk((cx, cy, data) => chunks.push([cx, cy, [...data]]));
    out.push([layer.id, chunks]);
  }
  return JSON.stringify(out);
}

function undoCount(session) {
  let n = 0;
  while (session.history.canUndo()) {
    session.undo();
    n++;
  }
  return n;
}

describe("pen masks match brush golden", () => {
  for (const n of [1, 2, 3, 5, 6]) {
    it(`size ${n}`, () => {
      const session = makeSession();
      session.setSetting("primaryColor", "#000000");
      session.setSetting("penSize", n);
      const pen = new PenTool(envFor(session), { mode: "draw" });
      pen.pointerDown(toolEvent({ x: 20, y: 20 }));
      pen.pointerUp(toolEvent({ x: 20, y: 20 }));
      const fp = brushFootprint(n);
      const store = session.doc.getLayer(session.doc.activeLayerId).store;
      const black = packRGBA(0, 0, 0, 255);
      for (let by = 0; by < n; by++) {
        for (let bx = 0; bx < n; bx++) {
          const px = 20 - fp.offset + bx;
          const py = 20 - fp.offset + by;
          const want = fp.mask[by * n + bx] === 1 ? black : 0;
          assert.equal(store.getPixel(px, py), want, `cell ${bx},${by}`);
        }
      }
    });
  }
});

describe("pen stroke undo/redo byte equal, one history", () => {
  it("64px stroke across chunk boundary", () => {
    const session = makeSession();
    session.setSetting("penSize", 3);
    session.setSetting("primaryColor", "#ff0000");
    const pen = new PenTool(envFor(session), { mode: "draw" });
    const before = snapshotDoc(session.doc);
    pen.pointerDown(toolEvent({ x: 0, y: 10 }));
    pen.pointerMove(toolEvent({ x: 40, y: 10, coalesced: [{ x: 20, y: 10 }, { x: 40, y: 10 }] }));
    pen.pointerUp(toolEvent({ x: 40, y: 10 }));
    const after = snapshotDoc(session.doc);
    assert.notEqual(before, after);
    assert.equal(session.history.canUndo(), true);
    session.undo();
    assert.equal(snapshotDoc(session.doc), before);
    session.redo();
    assert.equal(snapshotDoc(session.doc), after);
    assert.equal(undoCount(session), 1);
  });
});

describe("pen locked/hidden/cancel/same-color/shift/oob", () => {
  it("locked/hidden warn + invariant", () => {
    const session = makeSession();
    const id = session.doc.activeLayerId;
    const statuses = [];
    session.addEventListener(EVENTS.STATUS_MESSAGE, (e) => statuses.push(e.detail));
    const pen = new PenTool(envFor(session), { mode: "draw" });
    const before = snapshotDoc(session.doc);
    session.setLayerLocked(id, true);
    pen.pointerDown(toolEvent({ x: 5, y: 5 }));
    pen.pointerUp(toolEvent({ x: 5, y: 5 }));
    assert.equal(statuses.at(-1).code, "LAYER_LOCKED");
    assert.equal(snapshotDoc(session.doc), before);
    assert.equal(session.isEditing, false);
    session.setLayerLocked(id, false);
    session.setLayerVisible(id, false);
    pen.pointerDown(toolEvent({ x: 5, y: 5 }));
    pen.pointerUp(toolEvent({ x: 5, y: 5 }));
    assert.equal(statuses.at(-1).code, "LAYER_HIDDEN");
    assert.equal(snapshotDoc(session.doc), before);
  });

  it("cancel restores + history unchanged", () => {
    const session = makeSession();
    const pen = new PenTool(envFor(session), { mode: "draw" });
    const before = snapshotDoc(session.doc);
    pen.pointerDown(toolEvent({ x: 5, y: 5 }));
    pen.pointerMove(toolEvent({ x: 8, y: 5, coalesced: [{ x: 8, y: 5 }] }));
    pen.cancel();
    assert.equal(snapshotDoc(session.doc), before);
    assert.equal(session.history.canUndo(), false);
  });

  it("same color overpaint => no history", () => {
    const session = makeSession();
    const pen = new PenTool(envFor(session), { mode: "draw" });
    pen.pointerDown(toolEvent({ x: 5, y: 5 }));
    pen.pointerUp(toolEvent({ x: 5, y: 5 }));
    pen.pointerDown(toolEvent({ x: 5, y: 5 }));
    pen.pointerUp(toolEvent({ x: 5, y: 5 }));
    assert.equal(undoCount(session), 1);
  });

  it("shift line = strokeSegment(lastPoint->click)", () => {
    const session = makeSession();
    const pen = new PenTool(envFor(session), { mode: "draw" });
    pen.pointerDown(toolEvent({ x: 5, y: 5 }));
    pen.pointerUp(toolEvent({ x: 5, y: 5 }));
    pen.pointerDown(toolEvent({ x: 10, y: 5, shift: true }));
    const store = session.doc.getLayer(session.doc.activeLayerId).store;
    const black = packRGBA(0, 0, 0, 255);
    for (let x = 5; x <= 10; x++) assert.equal(store.getPixel(x, 5), black, `x=${x}`);
    assert.equal(undoCount(session), 2);
  });

  it("OOB start draws inner part", () => {
    const session = makeSession();
    const pen = new PenTool(envFor(session), { mode: "draw" });
    pen.pointerDown(toolEvent({ x: -5, y: 5 }));
    pen.pointerMove(toolEvent({ x: 4, y: 5, coalesced: [{ x: 0, y: 5 }, { x: 4, y: 5 }] }));
    pen.pointerUp(toolEvent({ x: 4, y: 5 }));
    const store = session.doc.getLayer(session.doc.activeLayerId).store;
    assert.equal(store.getPixel(2, 5), packRGBA(0, 0, 0, 255));
  });
});

describe("eraser sparsity", () => {
  it("chunkCount returns to 0", () => {
    const session = makeSession(32, 32);
    const pen = new PenTool(envFor(session), { mode: "draw" });
    session.setSetting("penSize", 1);
    pen.pointerDown(toolEvent({ x: 10, y: 10 }));
    pen.pointerUp(toolEvent({ x: 10, y: 10 }));
    const store = session.doc.getLayer(session.doc.activeLayerId).store;
    assert.equal(store.chunkCount(), 1);
    const eraser = new PenTool(envFor(session), { mode: "erase" });
    assert.equal(eraser.id, "eraser");
    eraser.pointerDown(toolEvent({ x: 10, y: 10 }));
    eraser.pointerUp(toolEvent({ x: 10, y: 10 }));
    assert.equal(store.chunkCount(), 0);
  });
});

describe("eyedropper", () => {
  function twoLayer() {
    const session = makeSession(32, 32);
    const bottomId = session.doc.activeLayerId;
    session.doc.getLayer(bottomId).store._setPixel(5, 5, packRGBA(255, 0, 0, 255));
    const topId = session.addLayer("top");
    session.doc.getLayer(topId).store._setPixel(5, 5, packRGBA(0, 0, 255, 255));
    session.setActiveLayer(topId);
    return { session, bottomId, topId };
  }
  it("composite color", () => {
    const { session } = twoLayer();
    const eye = new EyedropperTool(envFor(session));
    eye.pointerDown(toolEvent({ x: 5, y: 5 }));
    assert.equal(session.settings.primaryColor, "#0000ff");
  });
  it("transparent pixel no change + notify", () => {
    const { session } = twoLayer();
    const notes = [];
    session.addEventListener(EVENTS.STATUS_MESSAGE, (e) => notes.push(e.detail));
    const before = session.settings.primaryColor;
    const eye = new EyedropperTool(envFor(session));
    eye.pointerDown(toolEvent({ x: 0, y: 0 }));
    assert.equal(session.settings.primaryColor, before);
    assert.ok(notes.some((n) => n.text === "투명 픽셀"));
  });
  it("alt samples active layer", () => {
    const { session, bottomId } = twoLayer();
    session.setActiveLayer(bottomId);
    const eye = new EyedropperTool(envFor(session));
    eye.pointerDown(toolEvent({ x: 5, y: 5, alt: true }));
    assert.equal(session.settings.primaryColor, "#ff0000");
  });
  it("shift writes secondary", () => {
    const { session } = twoLayer();
    const eye = new EyedropperTool(envFor(session));
    eye.pointerDown(toolEvent({ x: 5, y: 5, shift: true }));
    assert.equal(session.settings.secondaryColor, "#0000ff");
  });
  it("drag updates + cursor + overlay", () => {
    const { session } = twoLayer();
    const eye = new EyedropperTool(envFor(session));
    assert.equal(cursorFallback(eye.cursor), "crosshair");
    eye.pointerDown(toolEvent({ x: 5, y: 5 }));
    eye.pointerMove(toolEvent({ x: 0, y: 0 }));
    const calls = [];
    eye.hover(toolEvent({ x: 5, y: 5 }));
    eye.overlay({ fillRect() {}, strokeRect(...a) { calls.push(a); } }, { view: { zoom: 1 }, dpr: 1, pixelToDevice: (x, y) => ({ x, y }) });
    assert.ok(calls.length >= 1);
  });
});

describe("fill", () => {
  it("golden 3 cases", () => {
    for (const c of G.fill) {
      const grid = c.grid.map((r) => r.slice());
      const get = (x, y) => grid[y][x];
      const set = (x, y, v) => void (grid[y][x] = v);
      floodFillScanline(set, get, grid[0].length, grid.length, c.x, c.y, grid[c.y][c.x], c.new);
      assert.deepEqual(grid, c.expect);
    }
  });
  it("tool fill + undo + oob + locked", () => {
    const session = makeSession(32, 32);
    session.setSetting("primaryColor", "#00ff00");
    const fill = new FillTool(envFor(session));
    assert.equal(cursorFallback(fill.cursor), "copy");
    fill.pointerDown(toolEvent({ x: 5, y: 5 }));
    const store = session.doc.getLayer(session.doc.activeLayerId).store;
    assert.equal(store.getPixel(0, 0), packRGBA(0, 255, 0, 255));
    assert.equal(session.history.undoLabel(), "페인트통");
    session.undo();
    assert.equal(store.getPixel(0, 0), 0);
    assert.equal(session.history.canUndo(), false);
    fill.pointerDown(toolEvent({ x: -1, y: 0 }));
    fill.pointerDown(toolEvent({ x: 32, y: 0 }));
    assert.equal(session.history.canUndo(), false);
    // locked rejection on a separate session (layer prop commands add history)
    const locked = makeSession(32, 32);
    const lid = locked.doc.activeLayerId;
    locked.setLayerLocked(lid, true);
    const notes = [];
    locked.addEventListener(EVENTS.STATUS_MESSAGE, (e) => notes.push(e.detail));
    new FillTool(envFor(locked)).pointerDown(toolEvent({ x: 5, y: 5 }));
    assert.equal(locked.doc.getLayer(lid).store.getPixel(5, 5), 0);
    assert.ok(notes.some((n) => n.code === "LAYER_LOCKED"));
    // same-color second fill adds no history
    fill.pointerDown(toolEvent({ x: 5, y: 5 }));
    fill.pointerDown(toolEvent({ x: 5, y: 5 }));
    assert.equal(undoCount(session), 1);
  });
  it("same color => notify, no history", () => {
    const session = makeSession(32, 32);
    session.setSetting("primaryColor", "#000000");
    const pen = new PenTool(envFor(session), { mode: "draw" });
    pen.pointerDown(toolEvent({ x: 3, y: 3 }));
    pen.pointerUp(toolEvent({ x: 3, y: 3 }));
    const notes = [];
    session.addEventListener(EVENTS.STATUS_MESSAGE, (e) => notes.push(e.detail));
    const fill = new FillTool(envFor(session));
    fill.pointerDown(toolEvent({ x: 3, y: 3 }));
    assert.ok(notes.some((n) => n.text === "이미 같은 색입니다"));
    assert.equal(undoCount(session), 1);
  });
});

describe("tool manager", () => {
  it("switch deactivates old + activates new; cancel first mid-stroke; invalid id", () => {
    const session = makeSession();
    const mgr = new ToolManager({ session });
    const log = [];
    const pen = new PenTool(envFor(session), { mode: "draw" });
    const hand = new HandTool(envFor(session));
    const origPenDe = pen.deactivate.bind(pen);
    const origPenCancel = pen.cancel.bind(pen);
    pen.deactivate = () => void log.push("deactivate-pen") || origPenDe();
    pen.cancel = () => void log.push("cancel-pen") || origPenCancel();
    const origHandAct = hand.activate.bind(hand);
    hand.activate = () => void log.push("activate-hand") || origHandAct();
    mgr.register(pen);
    mgr.register(hand);
    session.setSetting("activeTool", "hand");
    assert.deepEqual(log, ["deactivate-pen", "activate-hand"]);
    assert.equal(mgr.active.id, "hand");
    assert.equal(mgr.cursor, "grab");
    assert.ok(mgr.overlay === null || typeof mgr.overlay === "function" || mgr.overlay === undefined);
    // mid-stroke switch: pen drawing, switch to hand => cancel before deactivate
    log.length = 0;
    session.setSetting("activeTool", "pen");
    log.length = 0;
    mgr.pointerDown(toolEvent({ x: 5, y: 5 }));
    assert.equal(session.isEditing, true);
    session.setSetting("activeTool", "hand");
    assert.deepEqual(log.slice(0, 2), ["cancel-pen", "deactivate-pen"]);
    assert.equal(session.isEditing, false);
    mgr.dispose();
  });
  it("unregistered id => INVALID_STATE on use", () => {
    const session = makeSession();
    const mgr = new ToolManager({ session });
    mgr.register(new PenTool(envFor(session), { mode: "draw" }));
    session.setSetting("activeTool", "fill");
    assert.throws(() => mgr.active, (e) => e.code === "INVALID_STATE");
    assert.throws(() => mgr.pointerDown(toolEvent({ x: 0, y: 0 })), (e) => e.code === "INVALID_STATE");
    mgr.dispose();
  });
});

describe("WheelAccumulator", () => {
  it("accumulates small deltas", () => {
    const w = new WheelAccumulator();
    assert.equal(w.feed(-30, 0, 0), 0);
    assert.equal(w.feed(-30, 0, 0), 1);
  });
  it("direction flip resets", () => {
    const w = new WheelAccumulator();
    assert.equal(w.feed(-40, 0, 0), 0);
    assert.equal(w.feed(40, 0, 0), 0);
    assert.equal(w.feed(40, 0, 0), -1);
  });
  it("deltaMode 1 and 2", () => {
    const a = new WheelAccumulator();
    assert.equal(a.feed(-4, 1, 0), 1); // -64px => +1
    const b = new WheelAccumulator();
    assert.equal(b.feed(1, 2, 100), -1); // +100px => -1
  });
});

describe("misc tool contracts", () => {
  it("hand no-ops, pen keyDown false, alt-eyedrop on pen", () => {
    const session = makeSession(32, 32);
    session.doc.getLayer(session.doc.activeLayerId).store._setPixel(2, 2, packRGBA(10, 20, 30, 255));
    const hand = new HandTool(envFor(session));
    hand.pointerDown(toolEvent({ x: 1, y: 1 }));
    hand.pointerMove(toolEvent({ x: 2, y: 2 }));
    hand.pointerUp(toolEvent({ x: 2, y: 2 }));
    hand.cancel();
    assert.equal(session.history.canUndo(), false);
    const pen = new PenTool(envFor(session), { mode: "draw" });
    assert.equal(pen.keyDown({ key: "x" }), false);
    assert.equal(cursorFallback(pen.cursor), "crosshair");
    pen.pointerDown(toolEvent({ x: 2, y: 2, alt: true }));
    assert.equal(session.settings.primaryColor, "#0a141e");
    assert.equal(session.history.canUndo(), false);
  });
  it("pen button!=0 ignored; eyedrop button!=0 ignored", () => {
    const session = makeSession();
    const pen = new PenTool(envFor(session), { mode: "draw" });
    pen.pointerDown(toolEvent({ x: 5, y: 5, button: 2 }));
    assert.equal(session.isEditing, false);
    const eye = new EyedropperTool(envFor(session));
    eye.pointerDown(toolEvent({ x: 5, y: 5, button: 1 }));
    assert.equal(session.settings.primaryColor, "#000000");
  });
});
