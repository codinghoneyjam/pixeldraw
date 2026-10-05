import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { packRGBA } from "../src/core/pixel.js";
import { ToolManager } from "../src/tools/tool_manager.js";
import { PenTool } from "../src/tools/pen.js";
import { EyedropperTool } from "../src/tools/eyedropper.js";
import { HandTool } from "../src/tools/hand.js";
import { WheelAccumulator } from "../src/tools/input_controller.js";
import { toolEvent } from "./helpers/tool_event.js";
import { cursorFallback, envFor, makeSession } from "./helpers/tool_session.js";

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