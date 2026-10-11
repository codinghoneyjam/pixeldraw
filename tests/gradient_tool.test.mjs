import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { unpackRGBA } from "../src/core/pixel.js";
import { Session } from "../src/features/document/session.js";
import { GradientTool } from "../src/features/gradient/gradient_tool.js";
import { TOOL_IDS } from "../src/features/document/settings_validator.js";
import { withPreviewCanvas } from "./helpers/preview_canvas.js";

function setup() {
  const session = new Session();
  session.newDocument({ widthPx: 64, heightPx: 64 });
  session.setSetting("primaryColor", "#ff0000");
  session.setSetting("secondaryColor", "#0000ff");
  return { session, tool: new GradientTool({ session }) };
}

function drag(tool) {
  tool.pointerDown({ x: 32, y: 32, button: 0 });
  tool.pointerUp({ x: 48, y: 32, button: 0 });
}

function paintCount(session) {
  const store = session.doc.getLayer(session.doc.activeLayerId).store;
  let count = 0;
  for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) if (store.getPixel(x, y) !== 0) count++;
  return count;
}

describe("radial gradient tool", () => {
  it("is selectable and fades to transparent by default", () => {
    const { session, tool } = setup();
    assert.ok(TOOL_IDS.includes("radial_gradient"));
    drag(tool);
    const store = session.doc.getLayer(session.doc.activeLayerId).store;
    assert.ok(unpackRGBA(store.getPixel(32, 32))[3] > 0);
    assert.equal(store.getPixel(60, 32), 0);
    session.undo();
    assert.equal(store.getPixel(32, 32), 0);
  });

  it("can extend the secondary color outside the drag radius", () => {
    const { session, tool } = setup();
    tool.edgeMode = "solid";
    drag(tool);
    const store = session.doc.getLayer(session.doc.activeLayerId).store;
    assert.deepEqual(unpackRGBA(store.getPixel(60, 32)), [0, 0, 255, 255]);
  });

  it("tracks a preview drag without changing pixels before release", () => {
    const { session, tool } = setup();
    withPreviewCanvas(({ ctx, drawCount }) => {
      tool.pointerDown({ x: 16, y: 16, button: 0 });
      tool.pointerMove({ x: 32, y: 16, button: 0 });
      tool.overlay(ctx, { view: { zoom: 1, offsetX: 0, offsetY: 0 }, dpr: 1 });
      assert.ok(drawCount() > 0, "gradient is drawn on the canvas overlay");
      assert.equal(tool._center.x1, 32);
      assert.equal(paintCount(session), 0, "preview does not modify document pixels");
      tool.pointerUp({ x: 32, y: 16, button: 0 });
      assert.ok(paintCount(session) > 0);
    });
  });
});