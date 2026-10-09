import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { packRGBA } from "../src/core/pixel.js";
import { ToolManager } from "../src/tools/tool_manager.js";
import { PenTool } from "../src/features/pen/pen.js";
import { EyedropperTool } from "../src/features/eyedropper/eyedropper.js";
import { HandTool } from "../src/features/hand/hand.js";
import { WheelAccumulator } from "../src/tools/input_controller.js";
import { DEFAULT_CURSOR, readCursorState, resolveCursor } from "../src/tools/cursor_state.js";
import { InputController } from "../src/tools/input_controller.js";
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

describe("cursor state machine (cursor_state.js)", () => {
  // The ladder is outside > panning > space > tool. Pure function, so the whole
  // truth table is cheap; a fake host only has to record the CSS write.
  const rows = [
    { inside: true, panning: false, spaceDown: false, toolCursor: "crosshair", want: "crosshair", why: "tool wins when nothing else is active" },
    { inside: true, panning: false, spaceDown: true, toolCursor: "crosshair", want: "grab", why: "space beats the tool" },
    { inside: true, panning: true, spaceDown: true, toolCursor: "crosshair", want: "grabbing", why: "panning beats space" },
    { inside: false, panning: true, spaceDown: true, toolCursor: "crosshair", want: DEFAULT_CURSOR, why: "outside beats everything" },
    { inside: false, panning: false, spaceDown: false, toolCursor: "crosshair", want: DEFAULT_CURSOR, why: "outside alone" },
    { inside: true, panning: false, spaceDown: false, toolCursor: null, want: DEFAULT_CURSOR, why: "missing tool cursor falls back" },
    { inside: true, panning: false, spaceDown: false, toolCursor: "", want: DEFAULT_CURSOR, why: "empty tool cursor falls back" },
    { inside: true, panning: false, spaceDown: false, toolCursor: 42, want: DEFAULT_CURSOR, why: "non-string tool cursor falls back" },
    { want: DEFAULT_CURSOR, why: "an empty state object is all defaults" },
  ];
  for (const r of rows) {
    it(r.why, () => {
      assert.equal(resolveCursor(r), r.want);
    });
  }

  // Tool cursors are inline SVG data URLs, so compare against the registered
  // tool's own value rather than a hardcoded keyword.
  const penCursor = () => new PenTool(envFor(makeSession()), { mode: "draw" }).cursor;

  it("readCursorState reads the underscore fields off an InputController", () => {
    const session = makeSession();
    const mgr = new ToolManager({ session });
    mgr.register(new PenTool(envFor(session), { mode: "draw" }));
    const ctrl = new InputController({ host: null, session, toolManager: mgr });
    assert.equal(readCursorState(ctrl).toolCursor, penCursor());
    assert.equal(readCursorState(ctrl).inside, true);
    ctrl._inside = false;
    assert.equal(readCursorState(ctrl).inside, false);
    mgr.dispose();
  });

  it("readCursorState falls back when the active tool is unregistered", () => {
    const session = makeSession();
    const mgr = new ToolManager({ session });
    const ctrl = new InputController({ host: null, session, toolManager: mgr });
    // ToolManager.cursor throws INVALID_STATE until something is registered.
    assert.equal(readCursorState(ctrl).toolCursor, DEFAULT_CURSOR);
    mgr.dispose();
  });

  it("applyCursor writes the ladder result and tolerates a missing host", () => {
    const session = makeSession();
    const mgr = new ToolManager({ session });
    mgr.register(new PenTool(envFor(session), { mode: "draw" }));
    const writes = [];
    const ctrl = new InputController({ host: { style: {} }, session, toolManager: mgr });
    // record the write through the same property the controller touches
    Object.defineProperty(ctrl.host.style, "cursor", {
      get: () => writes.at(-1) ?? "",
      set: (v) => writes.push(v),
    });
    assert.equal(ctrl._applyCursor(), penCursor());
    assert.deepEqual(writes, [penCursor()]);
    ctrl._panning = true;
    assert.equal(ctrl._applyCursor(), "grabbing");
    assert.deepEqual(writes, [penCursor(), "grabbing"]);
    ctrl._panning = false;
    ctrl._spaceDown = true;
    assert.equal(ctrl._applyCursor(), "grab");
    ctrl._spaceDown = false;
    ctrl._inside = false;
    assert.equal(ctrl._applyCursor(), DEFAULT_CURSOR);
    // no host at all must not throw
    ctrl.host = null;
    assert.equal(ctrl._applyCursor(), DEFAULT_CURSOR);
    mgr.dispose();
  });

  it("_markInside sets the flag and re-applies the cursor", () => {
    const session = makeSession();
    const mgr = new ToolManager({ session });
    mgr.register(new PenTool(envFor(session), { mode: "draw" }));
    const writes = [];
    const ctrl = new InputController({ host: { style: {} }, session, toolManager: mgr });
    Object.defineProperty(ctrl.host.style, "cursor", {
      get: () => writes.at(-1) ?? "",
      set: (v) => writes.push(v),
    });
    ctrl._applyCursor();
    assert.equal(ctrl._markInside(false), false);
    assert.deepEqual(writes.at(-1), DEFAULT_CURSOR);
    assert.equal(ctrl._markInside(), true);
    assert.deepEqual(writes.at(-1), penCursor());
    mgr.dispose();
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