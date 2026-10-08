// H3 headless integration: Session + ToolManager + fake ToolEvent (Node, no DOM).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { Session } from "../src/model/session.js";
import { ToolManager } from "../src/tools/tool_manager.js";
import { PenTool } from "../src/tools/pen.js";
import { ShapeTool } from "../src/features/shape/shape.js";
import { EVENTS } from "../src/core/events.js";
import { flattenToRgba, exportPngBytes } from "../src/io/export_png.js";
import { documentToJson, jsonToDocument, layerToJson, importLayerJson } from "../src/io/serialize.js";
import { decodePng } from "../src/io/png.js";
import { toolEvent } from "./helpers/tool_event.js";

function env(session) {
  return { session, getView: () => ({ zoom: 1, offsetX: 0, offsetY: 0 }), requestRender: () => {} };
}

function hashDoc(doc) {
  return createHash("sha256").update(Buffer.from(flattenToRgba(doc))).digest("hex");
}

function diag(x0, y0, x1, y1, step = 8) {
  const pts = [];
  const n = Math.max(1, Math.ceil(Math.hypot(x1 - x0, y1 - y0) / step));
  for (let i = 0; i <= n; i++) pts.push({ x: Math.round(x0 + ((x1 - x0) * i) / n), y: Math.round(y0 + ((y1 - y0) * i) / n) });
  return pts;
}

function stroke(mgr, pts) {
  mgr.pointerDown(toolEvent({ x: pts[0].x, y: pts[0].y }));
  if (pts.length > 1) {
    const last = pts[pts.length - 1];
    mgr.pointerMove(toolEvent({ x: last.x, y: last.y, coalesced: pts.slice(1) }));
  }
  const last = pts[pts.length - 1];
  mgr.pointerUp(toolEvent({ x: last.x, y: last.y }));
}

function histFp(session) {
  return { canUndo: session.history.canUndo(), canRedo: session.history.canRedo(), undoLabel: session.history.undoLabel(), redoLabel: session.history.redoLabel() };
}

function makePenSession(w, h) {
  const session = new Session();
  session.newDocument({ widthPx: w, heightPx: h });
  const mgr = new ToolManager({ session });
  const e = env(session);
  mgr.register(new PenTool(e, { mode: "draw" }));
  mgr.register(new PenTool(e, { mode: "erase" }));
  return { session, mgr };
}

describe("H3.1+H3.2 diagonal scenario with hash-verified undo/redo", () => {
  it("pen5px diagonal, add layer, second stroke, eraser, undo x3, redo x1", () => {
    const { session, mgr } = makePenSession(512, 512);
    const hashes = [hashDoc(session.doc)];
    session.setSetting("penSize", 5);
    session.setSetting("primaryColor", "#ff0000");
    session.setSetting("activeTool", "pen");
    stroke(mgr, diag(20, 20, 220, 220));
    hashes.push(hashDoc(session.doc));
    assert.notEqual(hashes[1], hashes[0]);
    session.addLayer("second");
    assert.equal(session.doc.layers.length, 2);
    hashes.push(hashDoc(session.doc));
    session.setSetting("primaryColor", "#0000ff");
    stroke(mgr, diag(300, 300, 420, 360));
    hashes.push(hashDoc(session.doc));
    assert.notEqual(hashes[3], hashes[2]);
    session.setSetting("activeTool", "eraser");
    stroke(mgr, diag(300, 300, 360, 330));
    hashes.push(hashDoc(session.doc));
    assert.notEqual(hashes[4], hashes[3]);
    session.undo();
    assert.equal(hashDoc(session.doc), hashes[3]);
    session.undo();
    assert.equal(hashDoc(session.doc), hashes[2]);
    session.undo();
    assert.equal(hashDoc(session.doc), hashes[1]);
    assert.equal(session.doc.layers.length, 1);
    session.redo();
    assert.equal(hashDoc(session.doc), hashes[2]);
    assert.equal(session.doc.layers.length, 2);
    mgr.dispose();
  });
});

describe("H3.3 serialize roundtrip determinism", () => {
  it("json roundtrip keeps hash, double serialize identical", async () => {
    const { session, mgr } = makePenSession(512, 512);
    session.setSetting("penSize", 3);
    session.setSetting("primaryColor", "#00ff00");
    stroke(mgr, diag(10, 400, 400, 10));
    const before = hashDoc(session.doc);
    const o1 = await documentToJson(session.doc);
    const s1 = JSON.stringify(o1);
    const o2 = await documentToJson(session.doc);
    assert.equal(JSON.stringify(o2), s1);
    const { document: back } = await jsonToDocument(JSON.parse(s1));
    assert.equal(hashDoc(back), before);
    mgr.dispose();
  });
});

describe("H3.4 dirty flag lifecycle", () => {
  it("saved=false dirty, stroke=true, undo-to-save=false", () => {
    const { session, mgr } = makePenSession(64, 64);
    session.history.markSaved();
    assert.equal(session.history.isDirty(), false);
    session.setSetting("primaryColor", "#ff0000");
    stroke(mgr, diag(5, 5, 40, 40));
    assert.equal(session.history.isDirty(), true);
    session.undo();
    assert.equal(session.history.isDirty(), false);
    mgr.dispose();
  });
});

describe("H3.5 layer export-import pixel equality", () => {
  it("exported layer reimports with identical pixels", async () => {
    const { session, mgr } = makePenSession(128, 128);
    session.setSetting("penSize", 2);
    session.setSetting("primaryColor", "#123456");
    stroke(mgr, diag(10, 10, 100, 100));
    const srcId = session.doc.activeLayerId;
    const file = await layerToJson(session.doc, srcId);
    const target = new Session();
    target.newDocument({ widthPx: 128, heightPx: 128 });
    target.addLayer("spacer");
    const { layer, dropped } = await importLayerJson(target.doc, JSON.parse(JSON.stringify(file)));
    assert.equal(dropped, 0);
    assert.notEqual(layer.id, srcId);
    target.insertLayer(layer, target.doc.layers.length);
    const src = session.doc.getLayer(srcId).store;
    const dst = target.doc.getLayer(layer.id).store;
    assert.equal(dst.chunkCount(), src.chunkCount());
    src.forEachChunk((cx, cy, data) => assert.deepEqual([...dst.getChunk(cx, cy)], [...data], `chunk ${cx},${cy}`));
    mgr.dispose();
  });
});

describe("H3.6 shapes 4 kinds x 3 fills undo restores hash", () => {
  for (const kind of ["line", "rect", "rrect", "ellipse"]) {
    for (const fill of ["outline", "fill"]) {
      it(`${kind}/${fill}`, () => {
        const session = new Session();
        session.newDocument({ widthPx: 64, heightPx: 64 });
        session.setSetting("primaryColor", "#ff0000");
        session.setSetting("secondaryColor", "#0000ff");
        session.setSetting("shapeFill", fill);
        session.setSetting("penSize", 3);
        session.setSetting("shapeRadius", 3);
        const tool = new ShapeTool(env(session), kind);
        const pre = hashDoc(session.doc);
        tool.pointerDown(toolEvent({ x: 4, y: 4 }));
        tool.pointerMove(toolEvent({ x: 11, y: 10 }));
        tool.pointerUp(toolEvent({ x: 11, y: 10 }));
        assert.equal(tool.hasPending(), true);
        assert.equal(tool.keyDown({ key: "Enter" }), true);
        assert.equal(session.history.canUndo(), true);
        assert.notEqual(hashDoc(session.doc), pre);
        session.undo();
        assert.equal(hashDoc(session.doc), pre);
      });
    }
  }
});

describe("H3.7 no anti-aliasing in exported PNG", () => {
  it("unique colors <= n+1 and zero partial alpha", async () => {
    const { session, mgr } = makePenSession(256, 256);
    const colors = ["#ff0000", "#00ff00", "#0000ff", "#ffff00", "#ff00ff"];
    const sizes = [1, 2, 3, 7, 64];
    session.setSetting("activeTool", "pen");
    for (let i = 0; i < sizes.length; i++) {
      session.setSetting("penSize", sizes[i]);
      session.setSetting("primaryColor", colors[i]);
      const y = 20 + i * 40;
      stroke(mgr, diag(10, y, 200, y + 18, 6));
      const curve = [];
      for (let k = 0; k <= 16; k++) {
        const t = k / 16;
        curve.push({ x: Math.round(10 + 190 * t), y: Math.round(y + 30 * t * t) });
      }
      stroke(mgr, curve);
    }
    const png = await exportPngBytes(session.doc);
    const { rgba } = await decodePng(png);
    const seen = new Set();
    let partial = 0;
    for (let i = 0; i < rgba.length; i += 4) {
      const a = rgba[i + 3];
      if (a !== 0 && a !== 255) partial++;
      seen.add(`${rgba[i]},${rgba[i + 1]},${rgba[i + 2]},${a}`);
    }
    assert.equal(partial, 0);
    assert.ok(seen.size <= colors.length + 1, `unique ${seen.size} > ${colors.length + 1}`);
    mgr.dispose();
  });
});

describe("H3.8 locked/hidden stroke rejected", () => {
  it("pixels and history invariant plus STATUS_MESSAGE", () => {
    const { session, mgr } = makePenSession(64, 64);
    session.setSetting("primaryColor", "#ff0000");
    stroke(mgr, diag(5, 5, 20, 20));
    const id = session.doc.activeLayerId;
    const notes = [];
    session.addEventListener(EVENTS.STATUS_MESSAGE, (e) => notes.push(e.detail));
    session.setLayerLocked(id, true);
    let fp = { flat: hashDoc(session.doc), ...histFp(session) };
    session.setSetting("activeTool", "pen");
    mgr.pointerDown(toolEvent({ x: 30, y: 30 }));
    mgr.pointerUp(toolEvent({ x: 30, y: 30 }));
    assert.equal(hashDoc(session.doc), fp.flat);
    assert.deepEqual(histFp(session), { canUndo: fp.canUndo, canRedo: fp.canRedo, undoLabel: fp.undoLabel, redoLabel: fp.redoLabel });
    assert.ok(notes.some((n) => n.code === "LAYER_LOCKED"));
    assert.equal(session.isEditing, false);
    session.setLayerLocked(id, false);
    session.setLayerVisible(id, false);
    fp = { flat: hashDoc(session.doc), ...histFp(session) };
    mgr.pointerDown(toolEvent({ x: 30, y: 30 }));
    mgr.pointerUp(toolEvent({ x: 30, y: 30 }));
    assert.equal(hashDoc(session.doc), fp.flat);
    assert.deepEqual(histFp(session), { canUndo: fp.canUndo, canRedo: fp.canRedo, undoLabel: fp.undoLabel, redoLabel: fp.redoLabel });
    assert.ok(notes.some((n) => n.code === "LAYER_HIDDEN"));
    mgr.dispose();
  });
});

describe("H3.9 small-budget history evicts oldest, keeps min 1", () => {
  it("2MiB budget over 12 large strokes", () => {
    const { session, mgr } = makePenSession(512, 512);
    session.history.limitBytes = 2 * 1024 * 1024;
    session.setSetting("penSize", 64);
    session.setSetting("activeTool", "pen");
    const palette = ["#ff0000", "#00ff00", "#0000ff"];
    for (let i = 0; i < 12; i++) {
      session.setSetting("primaryColor", palette[i % palette.length]);
      if (i % 2 === 0) stroke(mgr, diag(0, 0, 511, 511, 16));
      else stroke(mgr, diag(511, 0, 0, 511, 16));
    }
    assert.equal(session.history.canUndo(), true);
    let n = 0;
    while (session.history.canUndo()) {
      session.undo();
      n++;
    }
    assert.ok(n >= 1, "min 1 retained");
    assert.ok(n < 12, `oldest evicted, kept ${n}/12`);
    mgr.dispose();
  });
});
