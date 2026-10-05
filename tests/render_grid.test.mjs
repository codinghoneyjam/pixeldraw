// Grid rungs + CanvasRenderer Node-safe surface (docs/render.md).

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { gridLines, paintGrid } from "../src/render/grid.js";
import { CanvasRenderer } from "../src/render/renderer.js";
import { DrawToolError } from "../src/core/errors.js";

describe("D4 grid lines", () => {
  const v = { zoom: 8, offsetX: 0, offsetY: 0 };

  it("pixel mode at zoom 8: 1px + 32px + 64px lines", () => {
    const lines = gridLines(v, 64, 64, 800, 600, "pixel");
    assert.ok(lines);
    assert.equal(lines.xs.length, 65 + 3 + 2);
    assert.equal(lines.ys.length, 65 + 3 + 2);
    assert.deepEqual(lines.xs[0], { g: 0, a: 0.15 });
  });

  it("unit mode hides below 6 CSS px spacing; off returns null", () => {
    assert.equal(gridLines({ zoom: 0.1, offsetX: 0, offsetY: 0 }, 512, 512, 800, 600, "unit"), null);
    assert.equal(gridLines(v, 64, 64, 800, 600, "off"), null);
    const tile = gridLines({ zoom: 1, offsetX: 0, offsetY: 0 }, 64, 64, 800, 600, "tile");
    assert.ok(tile);
    // `tile` is 64px ONLY. It used to also emit the 32px unit rungs, which made the
    // 32px and 64px modes look identical on screen -- exactly the reported bug.
    assert.equal(tile.xs.length, 2);
    // ...and `unit` must be 32px only, so the two modes stay distinguishable.
    // A 64px canvas at zoom 1 yields 32px rungs at 0/32/64 (64 is a valid boundary)
    // while `tile` yields only 0/64 -- the two modes never share a rung.
    const unit = gridLines({ zoom: 1, offsetX: 0, offsetY: 0 }, 64, 64, 800, 600, "unit");
    assert.ok(unit);
    assert.deepEqual(unit.xs.map((l) => l.g), [0, 32, 64]);
    assert.deepEqual(tile.xs.map((l) => l.g), [0, 64]);
  });

  it("paintGrid draws unit/tile as a two-tone bevel, pixel as a difference hairline", () => {
    // Record the blend mode per fill rather than the value left on the context:
    // rungs are drawn thinnest-first, so the FINAL op depends on draw order and
    // asserting on it would make this test fragile.
    const makeCtx = () => {
      const calls = [];
      const ops = new Set();
      return {
        calls,
        ops,
        save() { calls.push("save"); },
        restore() { calls.push("restore"); },
        beginPath() {},
        rect() {},
        clip() {},
        fillRect() { calls.push("fill"); ops.add(this.globalCompositeOperation); },
      };
    };
    // `difference` blending was abandoned for the visible rungs because it nets to
    // ZERO change on mid-tone pixels (the checkerboard seam) -- exactly where the
    // grid lines used to vanish. They are now a two-tone `source-over` bevel.
    const ctx = makeCtx();
    paintGrid(ctx, gridLines(v, 64, 64, 800, 600, "unit"), v, 0, 0, 512, 512, 800, 600, 1);
    assert.deepEqual([...ctx.ops], ["source-over"]);
    assert.ok(ctx.calls.includes("save") && ctx.calls.includes("restore") && ctx.calls.includes("fill"));

    // The 1px inspect rung still uses the faint `difference` hairline, so pixel
    // mode exercises BOTH blend paths.
    const hairCtx = makeCtx();
    const hairView = { zoom: 8, offsetX: 0, offsetY: 0 };
    paintGrid(hairCtx, gridLines(hairView, 64, 64, 800, 600, "pixel"),
      hairView, 0, 0, 512, 512, 800, 600, 1);
    assert.ok(hairCtx.ops.has("difference"), "pixel rung uses the difference hairline");
    assert.ok(hairCtx.ops.has("source-over"), "unit/tile rungs use the source-over bevel");

    // null lines must stay a safe no-op.
    paintGrid(ctx, null, v, 0, 0, 512, 512, 800, 600, 1);
  });
});

describe("D3 renderer Node-safe surface", () => {
  function fakeHost() {
    return { appendChild() {}, clientWidth: 800, clientHeight: 600, style: {} };
  }

  it("constructor validates args with DrawToolError", () => {
    const session = new EventTarget();
    assert.throws(() => new CanvasRenderer({ host: null, session, getView: () => ({}) }), DrawToolError);
    assert.throws(
      () => new CanvasRenderer({ host: fakeHost(), session: {}, getView: () => ({}) }),
      DrawToolError,
    );
  });

  it("pixelToDevice math + invalidate/dispose without DOM", () => {
    const session = new EventTarget();
    const r = new CanvasRenderer({
      host: fakeHost(),
      session,
      getView: () => ({ zoom: 2, offsetX: 10, offsetY: 20 }),
    });
    assert.deepEqual(r.pixelToDevice(5, 5), { x: 20, y: 30 });
    r.invalidateChunks([{ cx: 0, cy: 0 }, { cx: -1, cy: 2 }]);
    r.invalidateAll();
    r.requestRender();
    r.dispose();
  });
});