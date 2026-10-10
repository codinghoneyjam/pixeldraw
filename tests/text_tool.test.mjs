import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Session } from "../src/features/document/session.js";
import { decodeGlyphSets } from "../src/features/text/glyph_set.js";
import { TextTool } from "../src/features/text/text_tool.js";
import { TOOL_IDS, validateSetting } from "../src/features/document/settings_validator.js";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const GLYPHS = decodeGlyphSets(JSON.parse(fs.readFileSync(path.join(ROOT, "tools/recipe/text_glyphs.json"), "utf8")));

function setup() {
  const session = new Session();
  session.newDocument({ widthPx: 64, heightPx: 64 });
  const tool = new TextTool({ session, requestRender() {} }, GLYPHS);
  return { session, tool };
}

function hasPaint(session) {
  const store = session.doc.getLayer(session.doc.activeLayerId).store;
  for (let y = 0; y < 64; y++) {
    for (let x = 0; x < 64; x++) {
      if (store.getPixel(x, y) !== 0) return true;
    }
  }
  return false;
}

describe("text tool", () => {
  it("is a selectable registered tool", () => {
    assert.ok(TOOL_IDS.includes("text"));
    assert.equal(validateSetting("activeTool", "text"), "text");
  });

  it("places baked bitmap text as one undoable edit", () => {
    const { session, tool } = setup();
    tool.text = "A";
    tool.pointerDown({ x: 8, y: 8, button: 0 });
    assert.equal(hasPaint(session), true);
    assert.equal(session.history.canUndo(), true);
    session.undo();
    assert.equal(hasPaint(session), false);
  });

  it("does nothing for an empty string", () => {
    const { session, tool } = setup();
    tool.pointerDown({ x: 8, y: 8, button: 0 });
    assert.equal(hasPaint(session), false);
    assert.equal(session.history.canUndo(), false);
  });
});