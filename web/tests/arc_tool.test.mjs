import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { Session } from "../src/features/document/session.js";
import { ArcTool } from "../src/features/arc/arc_tool.js";
import { TOOL_IDS } from "../src/features/document/settings_validator.js";
import { withPreviewCanvas } from "./helpers/preview_canvas.js";

function setup(kind) {
  const session = new Session();
  session.newDocument({ widthPx: 64, heightPx: 64 });
  session.setSetting("primaryColor", "#ff0000");
  session.setSetting("secondaryColor", "#00ff00");
  return { session, tool: new ArcTool({ session }, kind) };
}

function paintCount(session) {
  const store = session.doc.getLayer(session.doc.activeLayerId).store;
  let count = 0;
  for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) if (store.getPixel(x, y) !== 0) count++;
  return count;
}

describe("arc and chord tools", () => {
  it("registers both recipe operations as tools", () => {
    assert.ok(TOOL_IDS.includes("arc"));
    assert.ok(TOOL_IDS.includes("chord"));
    assert.equal(TOOL_IDS.includes("text"), true);
  });

  it("stamps a dragged arc as one undoable edit", () => {
    const { session, tool } = setup("arc");
    tool.pointerDown({ x: 8, y: 8, button: 0 });
    tool.pointerUp({ x: 36, y: 36, button: 0 });
    assert.ok(paintCount(session) > 0);
    session.undo();
    assert.equal(paintCount(session), 0);
  });

  it("stamps a filled chord", () => {
    const { session, tool } = setup("chord");
    tool.start = 0;
    tool.end = 180;
    tool.outline = false;
    tool.pointerDown({ x: 8, y: 8, button: 0 });
    tool.pointerUp({ x: 36, y: 36, button: 0 });
    assert.ok(paintCount(session) > 20);
  });

  it("previews the selected chord mode before committing", () => {
    const { session, tool } = setup("chord");
    withPreviewCanvas(({ ctx, drawCount }) => {
      tool.pointerDown({ x: 8, y: 8, button: 0 });
      tool.pointerMove({ x: 36, y: 36, button: 0 });
      tool.overlay(ctx, { view: { zoom: 1, offsetX: 0, offsetY: 0 }, dpr: 1 });
      assert.ok(drawCount() > 0, "preview mask is drawn on the canvas overlay");
      assert.equal(paintCount(session), 0, "preview does not modify document pixels");
      assert.equal(tool._draw.x1, 36);
      tool.chordMode = "outline";
      tool.pointerUp({ x: 36, y: 36, button: 0 });
      assert.ok(paintCount(session) > 0);
    });
  });
});