// ShapeTool preview/overlay parity (docs/tools.md, shape_render.js).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { packRGBA } from "../src/core/pixel.js";
import { applyShape } from "../src/core/raster/shape_raster.js";
import { PP, drag, fakeWriter, key, pixel, setup } from "./helpers/shape_tool.js";

describe("preview mask == committed pixels (4 kinds x 3 fills x odd/even)", () => {
  const cases = [];
  for (const kind of ["rect", "rrect", "ellipse", "line"]) {
    for (const fill of ["outline", "fill"]) {
      for (const parity of ["even", "odd"]) cases.push({ kind, fill, parity });
    }
  }
  for (const { kind, fill, parity } of cases) {
    it(`${kind}/${fill}/${parity}`, () => {
      const { session, tool } = setup(kind);
      session.setSetting("shapeFill", fill);
      session.setSetting("penSize", 3);
      session.setSetting("shapeRadius", 3);
      const id = session.doc.activeLayerId;
      const [x0, y0, x1, y1] = parity === "even" ? [4, 4, 11, 10] : [20, 20, 26, 26];
      drag(tool, x0, y0, x1, y1);
      assert.equal(tool.hasPending(), true);
      const W = 64;
      const H = 64;
      const prev = fakeWriter(W, H);
      assert.ok(tool.paintPreview(prev) > 0);
      assert.equal(tool.keyDown(key("Enter")), true);
      for (let y = 0; y < H; y++) {
        for (let x = 0; x < W; x++) {
          assert.equal(pixel(session, id, x, y), prev.grid[y][x], `${kind}/${fill}/${parity} @${x},${y}`);
        }
      }
    });
  }

  it("direct applyShape agrees with committed layer for a fill-mode rrect", () => {
    const { session, tool } = setup("rrect");
    session.setSetting("shapeFill", "fill");
    session.setSetting("penSize", 2);
    session.setSetting("shapeRadius", 3);
    const id = session.doc.activeLayerId;
    tool.setPending({ x: 5, y: 5, w: 12, h: 9, radius: 3 });
    const expect = fakeWriter(64, 64);
    // Only the foreground may paint, so the committed layer must match an applyShape
    // call given the foreground colour for BOTH colour arguments.
    applyShape(expect, { kind: "rrect", bbox: { x: 5, y: 5, w: 12, h: 9 }, radius: 3, strokeWidth: 2, fillMode: "fill" }, PP, PP);
    tool.keyDown(key("Enter"));
    for (let y = 0; y < 64; y++) {
      for (let x = 0; x < 64; x++) assert.equal(pixel(session, id, x, y), expect.grid[y][x], `fill @${x},${y}`);
    }
  });

  it("a shape keeps the foreground colour it was drawn with", () => {
    const { session, tool } = setup("rrect");
    session.setSetting("shapeFill", "fill");
    const id = session.doc.activeLayerId;
    tool.setPending({ x: 5, y: 5, w: 12, h: 9, radius: 3 });
    // Change the foreground AFTER the shape exists but BEFORE it is committed: the
    // already-made shape must not follow along.
    session.setSetting("primaryColor", "#00ff00");
    tool.keyDown(key("Enter"));
    let drawn = 0;
    for (let y = 0; y < 64; y++) {
      for (let x = 0; x < 64; x++) {
        const p = pixel(session, id, x, y);
        if (p === PP) drawn += 1;
        assert.notEqual(p, packRGBA(0, 255, 0, 255), `recoloured @${x},${y}`);
      }
    }
    assert.ok(drawn > 0, "shape painted something");
  });
});

describe("overlay draws without DOM", () => {
  it("emits fillRect calls covering the mask plus handles and dashed bbox", () => {
    const { tool } = setup("rect");
    drag(tool, 4, 4, 11, 10);
    const calls = [];
    const fake = {
      fillStyle: "", strokeStyle: "", lineWidth: 1,
      fillRect: (...a) => calls.push(["fillRect", ...a]),
      strokeRect: (...a) => calls.push(["strokeRect", ...a]),
      save: () => calls.push(["save"]), restore: () => calls.push(["restore"]),
      setLineDash: (d) => calls.push(["setLineDash", d]),
    };
    tool.overlay(fake, { view: { zoom: 2, offsetX: 0, offsetY: 0 }, dpr: 1 });
    const fills = calls.filter((c) => c[0] === "fillRect");
    assert.ok(fills.length > 16);
    assert.ok(calls.some((c) => c[0] === "setLineDash"));
    assert.ok(calls.filter((c) => c[0] === "strokeRect").length >= 9);
  });
});