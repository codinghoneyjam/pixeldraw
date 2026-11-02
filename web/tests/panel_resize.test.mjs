import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { clampPanelWidth, panelWidthAtPointer } from "../src/ui/shared/panel_resize.js";

describe("panel resize math", () => {
  it("clamps rounded widths to responsive bounds", () => {
    assert.equal(clampPanelWidth(179.8, 180, 420), 180);
    assert.equal(clampPanelWidth(451, 180, 420), 420);
    assert.equal(clampPanelWidth(256.4, 180, 420), 256);
  });

  it("moves each panel edge in the expected direction", () => {
    assert.equal(panelWidthAtPointer("left", 220, 100, 125), 245);
    assert.equal(panelWidthAtPointer("right", 240, 300, 325), 215);
  });
});