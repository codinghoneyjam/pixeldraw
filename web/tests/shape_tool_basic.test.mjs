// ShapeTool state machine (docs/tools.md, shape.js). Preview/overlay cases: shape_preview.test.mjs
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { drag, ev, key, pixel, setup, statuses, toolStates } from "./helpers/shape_tool.js";

describe("shape tool state machine", () => {
  it("drag => pending with 0 history; arrows; Enter => exactly 1 entry; undo restores", () => {
    const { session, tool } = setup("rect");
    const id = session.doc.activeLayerId;
    const states = toolStates(session);
    drag(tool, 4, 4, 11, 10);
    assert.equal(tool.hasPending(), true);
    assert.deepEqual(tool.getPending(), { x: 4, y: 4, w: 8, h: 7, radius: 8 });
    assert.equal(session.history.canUndo(), false);
    assert.ok(states.length >= 1);
    assert.equal(tool.keyDown(key("ArrowRight")), true);
    assert.equal(tool.getPending().x, 5);
    assert.equal(tool.keyDown(key("Enter")), true);
    assert.equal(tool.hasPending(), false);
    assert.equal(session.history.canUndo(), true);
    session.undo();
    assert.equal(session.history.canUndo(), false);
    assert.equal(pixel(session, id, 5, 4), 0);
    session.redo();
    assert.equal(session.history.canUndo(), true);
    assert.ok(pixel(session, id, 5, 4) !== 0);
    session.undo();
    assert.equal(session.history.canUndo(), false);
  });

  it("Esc discards pending with 0 history; zero-move click cancels", () => {
    const { session, tool } = setup("ellipse");
    drag(tool, 4, 4, 11, 10);
    assert.equal(tool.hasPending(), true);
    assert.equal(tool.keyDown(key("Escape")), true);
    assert.equal(tool.hasPending(), false);
    assert.equal(session.history.canUndo(), false);
    tool.pointerDown(ev(20, 20));
    tool.pointerUp(ev(20, 20));
    assert.equal(tool.hasPending(), false);
    assert.equal(session.history.canUndo(), false);
  });

  it("outside down commits current then starts a new shape", () => {
    const { session, tool } = setup("rect");
    drag(tool, 4, 4, 11, 10);
    tool.pointerDown(ev(40, 40));
    assert.equal(session.history.canUndo(), true);
    tool.pointerMove(ev(45, 46));
    tool.pointerUp(ev(45, 46));
    assert.equal(tool.hasPending(), true);
    assert.deepEqual(tool.getPending(), { x: 40, y: 40, w: 6, h: 7, radius: 8 });
    assert.equal(tool.keyDown(key("Enter")), true);
    session.undo();
    session.undo();
    assert.equal(session.history.canUndo(), false);
  });

  it("layer switch commits to the old layer; locked layer drops pending with warn", () => {
    const { session, tool } = setup("rect");
    const a = session.doc.activeLayerId;
    const b = session.addLayer("top");
    session.setActiveLayer(a);
    drag(tool, 4, 4, 11, 10);
    session.setActiveLayer(b);
    assert.equal(tool.hasPending(), false);
    assert.equal(session.history.canUndo(), true);
    assert.ok(pixel(session, a, 4, 4) !== 0);
    assert.equal(pixel(session, b, 4, 4), 0);
    const warn = statuses(session);
    drag(tool, 4, 4, 11, 10);
    session.setLayerLocked(b, true);
    assert.equal(tool.keyDown(key("Enter")), true);
    assert.equal(tool.hasPending(), false);
    assert.equal(pixel(session, b, 4, 4), 0);
    assert.ok(warn.some((m) => m.code === "LAYER_LOCKED"));
  });

  it("pending discard path never calls session.undo", () => {
    const { session, tool } = setup("rect");
    let undos = 0;
    const orig = session.undo.bind(session);
    session.undo = (...a) => { undos++; return orig(...a); };
    drag(tool, 4, 4, 11, 10);
    assert.equal(tool.hasPending(), true);
    tool.discardPending();
    assert.equal(undos, 0);
    assert.equal(session.history.canUndo(), false);
    session.undo = orig;
  });

  it("deactivate commits pending; cancel during move keeps pending", () => {
    const { session, tool } = setup("rect");
    drag(tool, 4, 4, 11, 10);
    tool.deactivate();
    assert.equal(tool.hasPending(), false);
    assert.equal(session.history.canUndo(), true);
    drag(tool, 4, 4, 11, 10);
    tool.pointerDown(ev(6, 6));
    tool.pointerMove(ev(8, 8));
    tool.cancel();
    assert.equal(tool.hasPending(), true);
    assert.deepEqual(tool.getPending(), { x: 4, y: 4, w: 8, h: 7, radius: 8 });
  });

  it("setPending validates and moves geometry; bad sizes throw OUT_OF_RANGE", () => {
    const { session, tool } = setup("rrect");
    tool.setPending({ x: 2, y: 2, w: 10, h: 6, radius: 2 });
    assert.deepEqual(tool.getPending(), { x: 2, y: 2, w: 10, h: 6, radius: 2 });
    assert.throws(() => tool.setPending({ x: 0, y: 0, w: 0, h: 5 }), (e) => e.code === "OUT_OF_RANGE");
    assert.throws(() => tool.setPending({ x: 0, y: 0, w: 4, h: -1 }), (e) => e.code === "OUT_OF_RANGE");
    assert.equal(tool.keyDown(key("Enter")), true);
    assert.equal(session.history.canUndo(), true);
  });

  it("line: drag creates 2-endpoint pending; resize moves one end; move shifts both", () => {
    const { session, tool } = setup("line");
    session.setSetting("penSize", 1);
    drag(tool, 4, 4, 20, 4);
    assert.deepEqual(tool.getPending(), { x0: 4, y0: 4, x1: 20, y1: 4 });
    tool.pointerDown(ev(20, 4));
    tool.pointerMove(ev(22, 6));
    tool.pointerUp(ev(22, 6));
    assert.deepEqual(tool.getPending(), { x0: 4, y0: 4, x1: 22, y1: 6 });
    tool.pointerDown(ev(13, 5));
    tool.pointerMove(ev(14, 6));
    tool.pointerUp(ev(14, 6));
    assert.deepEqual(tool.getPending(), { x0: 5, y0: 5, x1: 23, y1: 7 });
    assert.equal(tool.keyDown(key("Enter")), true);
    assert.equal(session.history.canUndo(), true);
  });

  it("shift angle-snaps lines; shift squares boxes; alt centers; snapUnit gives 32 multiples", () => {
    const s1 = setup("line");
    s1.tool.pointerDown(ev(4, 4));
    s1.tool.pointerMove(ev(10, 5, { shift: true }));
    s1.tool.pointerUp(ev(10, 5, { shift: true }));
    assert.deepEqual(s1.tool.getPending(), { x0: 4, y0: 4, x1: 10, y1: 6 });
    const s2 = setup("rect");
    drag(s2.tool, 2, 2, 9, 5, { shift: true });
    const sq = s2.tool.getPending();
    assert.equal(sq.w, sq.h);
    const s3 = setup("rect");
    drag(s3.tool, 20, 20, 23, 22, { alt: true });
    const c = s3.tool.getPending();
    assert.equal(c.x + Math.floor((c.w - 1) / 2), 20);
    assert.equal(c.y + Math.floor((c.h - 1) / 2), 20);
    const s4 = setup("rrect");
    s4.session.setSetting("snapUnit", true);
    drag(s4.tool, 3, 5, 40, 45);
    const sn = s4.tool.getPending();
    for (const v of [sn.x, sn.y, sn.w, sn.h]) assert.equal(v % 32, 0);
  });

  it("settings change live-updates pending radius and emits tool state", () => {
    const { session, tool } = setup("rrect");
    const states = toolStates(session);
    drag(tool, 4, 4, 11, 10);
    const n = states.length;
    session.setSetting("shapeRadius", 3);
    assert.equal(tool.getPending().radius, 3);
    assert.ok(states.length > n);
  });
});