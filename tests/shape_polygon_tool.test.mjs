// Phase-1 tools: polygon ShapeTool interaction + RGBA color plumbing.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { packRGBA, unpackRGBA } from "../src/core/pixel.js";
import { applyShape } from "../src/core/raster/shape_raster.js";
import { validateSetting } from "../src/model/settings_validator.js";
import { packOf } from "../src/tools/shape_overlay.js";
import { normHex } from "../src/ui/color/panel_color_fields.js";
import { assertDrawError } from "./helpers/model.js";
import { setup, ev, key, drag, pixel } from "./helpers/shape_tool.js";

const TRI = [[8, 8], [24, 8], [16, 20]];

function place(tool, pts) {
  for (const [x, y] of pts) tool.pointerDown(ev(x, y));
}

function activeLayer(session) {
  return session.doc.activeLayerId;
}

describe("polygon placing", () => {
  it("three clicks + Enter => pending; Enter commits pixels equal to applyShape", () => {
    const { session, tool } = setup("polygon");
    place(tool, TRI);
    assert.equal(tool.hasPending(), false);
    assert.equal(tool.keyDown(key("Enter")), true);
    assert.equal(tool.hasPending(), true);
    assert.deepEqual(tool.getPending(), { points: TRI.map(([x, y]) => ({ x, y })) });
    assert.equal(tool.keyDown(key("Enter")), true);
    assert.equal(tool.hasPending(), false);
    // Reference render of the same spec must match the committed pixels.
    const want = new Map();
    const fake = { set(x, y, v) { want.set(`${x},${y}`, v); } };
    const spec = { kind: "polygon", points: TRI.map(([x, y]) => ({ x, y })), strokeWidth: 1, fillMode: "outline" };
    applyShape(fake, spec, packRGBA(255, 0, 0, 255), packRGBA(255, 0, 0, 255));
    const id = activeLayer(session);
    for (const [k, v] of want) {
      const [x, y] = k.split(",").map(Number);
      assert.equal(pixel(session, id, x, y), v, `pixel ${k}`);
    }
    assert.equal(session.history.isDirty(), true);
  });

  it("clicking the first vertex closes to pending", () => {
    const { tool } = setup("polygon");
    place(tool, TRI);
    tool.pointerDown(ev(8, 8));
    assert.equal(tool.hasPending(), true);
  });

  it("Enter with < 3 vertices does nothing; Esc pops then cancels", () => {
    const { tool } = setup("polygon");
    tool.pointerDown(ev(4, 4));
    tool.pointerDown(ev(10, 4));
    assert.equal(tool.keyDown(key("Enter")), false);
    assert.equal(tool.hasPending(), false);
    assert.equal(tool.keyDown(key("Escape")), true);
    assert.equal(tool.keyDown(key("Escape")), true);
    assert.equal(tool.hasPending(), false);
    assert.equal(tool.keyDown(key("Delete")), false);
  });

  it("Delete discards placing outright", () => {
    const { tool } = setup("polygon");
    place(tool, TRI);
    assert.equal(tool.keyDown(key("Delete")), true);
    assert.equal(tool.hasPending(), false);
  });

  it("vertex handle resize moves one vertex; inside-drag moves all; arrows nudge", () => {
    const { session, tool } = setup("polygon");
    place(tool, TRI);
    tool.keyDown(key("Enter"));
    // Resize vertex v1 (24,8) to (28,8).
    tool.pointerDown(ev(24, 8));
    tool.pointerMove(ev(28, 8));
    tool.pointerUp(ev(28, 8));
    assert.deepEqual(tool.getPending().points[1], { x: 28, y: 8 });
    assert.deepEqual(tool.getPending().points[0], { x: 8, y: 8 });
    // Move by dragging inside the triangle.
    tool.pointerDown(ev(16, 12));
    tool.pointerMove(ev(20, 12));
    tool.pointerUp(ev(20, 12));
    assert.deepEqual(tool.getPending().points[0], { x: 12, y: 8 });
    // Arrow nudge shifts every vertex.
    tool.keyDown(key("ArrowRight"));
    assert.deepEqual(tool.getPending().points[0], { x: 13, y: 8 });
    assert.deepEqual(tool.getPending().points[2], { x: 21, y: 20 });
    void session;
  });

  it("switching away with >= 3 placed vertices commits; < 3 discards", () => {
    const { tool } = setup("polygon");
    place(tool, TRI);
    tool.deactivate();
    assert.equal(tool.hasPending(), false);
    const t2 = setup("polygon").tool;
    t2.pointerDown(ev(1, 1));
    t2.deactivate();
    assert.equal(t2.hasPending(), false);
  });

  it("setPending validates points; fill commit matches applyShape", () => {
    const { session, tool } = setup("polygon");
    assertDrawError(() => tool.setPending({ points: [{ x: 1, y: 1 }] }), "OUT_OF_RANGE");
    session.setSetting("shapeFill", "fill");
    tool.setPending({ points: TRI.map(([x, y]) => ({ x, y })) });
    assert.equal(tool.commit(), true);
    const id = activeLayer(session);
    // Interior point of the filled triangle must be painted.
    assert.notEqual(pixel(session, id, 16, 12), 0);
  });

  it("placing preview renders without throwing", () => {
    const { tool } = setup("polygon");
    place(tool, [[8, 8], [24, 8]]);
    tool.pointerMove(ev(16, 20));
    const seen = [];
    const n = tool.paintPreview({ set(x, y, v) { seen.push([x, y, v]); } });
    assert.ok(n > 0);
    assert.ok(seen.length > 0);
  });
});

describe("RGBA color plumbing", () => {
  it("validateSetting canonicalizes opaque and keeps translucent", () => {
    assert.equal(validateSetting("primaryColor", "#FF0000"), "#ff0000");
    assert.equal(validateSetting("primaryColor", "#ff000080"), "#ff000080");
    assert.equal(validateSetting("secondaryColor", "#11223344"), "#11223344");
    assertDrawError(() => validateSetting("primaryColor", "red"), "OUT_OF_RANGE");
    assertDrawError(() => validateSetting("primaryColor", "#00000000"), "OUT_OF_RANGE");
    assert.equal(validateSetting("primaryColor", "#fff"), "#ffffff");
  });

  it("normHex accepts #rgb/#rrggbb/#rrggbbaa", () => {
    assert.equal(normHex("#abc"), "#aabbcc");
    assert.equal(normHex("#11223344"), "#11223344");
    assert.equal(normHex("#FF000080"), "#ff000080");
    assert.equal(normHex("red"), null);
  });

  it("packOf preserves alpha", () => {
    assert.deepEqual(unpackRGBA(packOf("#ff000080")), [255, 0, 0, 128]);
    assert.deepEqual(unpackRGBA(packOf("#ff0000")), [255, 0, 0, 255]);
  });

  it("rect commit with translucent color stores alpha pixels", () => {
    const { session, tool } = setup("rect");
    session.setSetting("primaryColor", "#00ff0080");
    session.setSetting("shapeFill", "fill");
    drag(tool, 4, 4, 10, 8);
    tool.commit();
    const [r, g, b, a] = unpackRGBA(pixel(session, activeLayer(session), 6, 6));
    assert.deepEqual([r, g, b, a], [0, 255, 0, 128]);
  });

  it("fill with translucent color stores alpha pixels", async () => {
    const { session } = setup("rect");
    session.setSetting("primaryColor", "#0000ff80");
    const { FillTool } = await import("../src/tools/fill.js");
    const fill = new FillTool({ session });
    fill.pointerDown(ev(2, 2, { button: 0 }));
    const [r, g, b, a] = unpackRGBA(pixel(session, activeLayer(session), 2, 2));
    assert.deepEqual([r, g, b, a], [0, 0, 255, 128]);
  });
});
