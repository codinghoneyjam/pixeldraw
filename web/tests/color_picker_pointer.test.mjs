import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { pointerToSv } from "../src/features/color/panel_color_wheel.js";

const rect = { left: 10, top: 20, width: 74, height: 74 };
const border = { left: 1, right: 1, top: 1, bottom: 1 };

describe("SV pointer coordinates", () => {
  it("maps the content corners to full saturation/value range", () => {
    assert.deepEqual(pointerToSv(rect, { clientX: 11, clientY: 21 }, border), { s: 0, v: 1 });
    assert.deepEqual(pointerToSv(rect, { clientX: 83, clientY: 93 }, border), { s: 1, v: 0 });
  });

  it("clamps drags outside the content area", () => {
    assert.deepEqual(pointerToSv(rect, { clientX: -20, clientY: -20 }, border), { s: 0, v: 1 });
    assert.deepEqual(pointerToSv(rect, { clientX: 120, clientY: 120 }, border), { s: 1, v: 0 });
  });

  it("uses the element center as the neutral midpoint", () => {
    assert.deepEqual(pointerToSv(rect, { clientX: 47, clientY: 57 }, border), { s: 0.5, v: 0.5 });
  });
});