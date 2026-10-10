import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { unpackRGBA } from "../src/core/pixel.js";
import { Session } from "../src/features/document/session.js";
import { GradientTool } from "../src/features/gradient/gradient_tool.js";
import { TOOL_IDS } from "../src/features/document/settings_validator.js";

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
});