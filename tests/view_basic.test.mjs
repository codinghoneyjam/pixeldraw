// View worked examples + composite basics (docs/render.md).
// Run: node --test draw_tool_v2/tests/view_basic.test.mjs

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  createView,
  screenToCanvas,
  canvasToScreen,
  pixelAt,
  zoomAt,
  stepZoom,
  panBy,
  clampView,
  fitView,
  actualSizeView,
  visibleChunkRange,
} from "../src/render/view.js";
import { compositeChunk, samplePixel } from "../src/render/composite.js";
import { gridLines, paintGrid } from "../src/render/grid.js";
import { CanvasRenderer } from "../src/render/renderer.js";
import { over } from "../src/core/blend.js";
import { DrawToolError } from "../src/core/errors.js";

function lcg(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

function solidChunk(r, g, b, a) {
  const d = new Uint8ClampedArray(4096);
  for (let i = 0; i < 4096; i += 4) {
    d[i] = r;
    d[i + 1] = g;
    d[i + 2] = b;
    d[i + 3] = a;
  }
  return d;
}

function storeOf(entries) {
  const map = new Map(entries);
  return { getChunk: (cx, cy) => map.get(`${cx},${cy}`) ?? null };
}

function mkLayer(chunk, { visible = true, opacity = 1 } = {}) {
  return { visible, opacity, store: storeOf(chunk ? [["0,0", chunk]] : []) };
}

function mkDoc(layers, w = 64, h = 64) {
  return { canvas: { widthPx: w, heightPx: h, background: "transparent" }, layers };
}

describe("D1 view worked examples", () => {
  it("zoom 4 offset(100,50) mappings", () => {
    const v = createView({ zoom: 4, offsetX: 100, offsetY: 50 });
    assert.deepEqual(screenToCanvas(v, 140, 90), { x: 10, y: 10 });
    assert.deepEqual(pixelAt(v, 143.9, 93.9), { x: 10, y: 10 });
    assert.deepEqual(pixelAt(v, 99, 49), { x: -1, y: -1 });
    assert.deepEqual(canvasToScreen(v, 10, 10), { x: 140, y: 90 });
  });

  it("zoomAt anchor fixed", () => {
    assert.deepEqual(zoomAt(createView({ zoom: 2 }), 100, 100, 4), { zoom: 4, offsetX: -100, offsetY: -100 });
    const v = createView({ zoom: 4, offsetX: 100, offsetY: 50 });
    const z = zoomAt(v, 300, 200, 8);
    assert.deepEqual(screenToCanvas(z, 300, 200), screenToCanvas(v, 300, 200));
  });

  it("stepZoom edges", () => {
    assert.equal(stepZoom(1, 1), 2);
    assert.equal(stepZoom(32, 1), 32);
    assert.equal(stepZoom(0.25, -1), 0.25);
    assert.equal(stepZoom(5, 1), 6);
    assert.equal(stepZoom(5, -1), 4);
  });

  it("panBy/clampView/fitView/actualSizeView", () => {
    assert.deepEqual(panBy(createView({ zoom: 2 }), 5, -7), { zoom: 2, offsetX: 5, offsetY: -7 });
    assert.deepEqual(
      clampView(createView({ offsetX: 5000, offsetY: -5000 }), 512, 512, 800, 600),
      { zoom: 1, offsetX: 768, offsetY: -480 },
    );
    assert.deepEqual(fitView(512, 512, 800, 600), { zoom: 1, offsetX: 144, offsetY: 44 });
    assert.deepEqual(fitView(1920, 1088, 1200, 700), { zoom: 0.5, offsetX: 120, offsetY: 78 });
    assert.deepEqual(actualSizeView(512, 512, 800, 600), { zoom: 1, offsetX: 144, offsetY: 44 });
  });

  it("visibleChunkRange", () => {
    assert.deepEqual(
      visibleChunkRange(createView({ zoom: 1 }), 512, 512, 800, 600),
      { cx0: 0, cy0: 0, cx1: 15, cy1: 15 },
    );
    assert.deepEqual(
      visibleChunkRange(createView({ zoom: 4, offsetX: -64, offsetY: 0 }), 512, 512, 256, 256),
      { cx0: 0, cy0: 0, cx1: 2, cy1: 1 },
    );
    assert.equal(visibleChunkRange(createView({ zoom: 1, offsetX: 900 }), 512, 512, 800, 600), null);
  });

  it("createView rejects bad zoom", () => {
    assert.throws(() => createView({ zoom: 0 }), DrawToolError);
    assert.throws(() => zoomAt(createView(), 0, 0, -2), DrawToolError);
  });

  it("roundtrip + zoomAt invariance over 100 seeded views", () => {
    const rand = lcg(1234);
    const levels = [0.25, 0.5, 1, 2, 3, 4, 6, 8, 12, 16, 24, 32];
    for (let i = 0; i < 100; i++) {
      const v = createView({
        zoom: levels[Math.floor(rand() * levels.length)],
        offsetX: Math.floor(rand() * 4000) - 2000,
        offsetY: Math.floor(rand() * 4000) - 2000,
      });
      const px = rand() * 512;
      const py = rand() * 512;
      const s = canvasToScreen(v, px, py);
      assert.deepEqual(pixelAt(v, s.x, s.y), { x: Math.floor(px), y: Math.floor(py) });
      const nz = levels[Math.floor(rand() * levels.length)];
      const ax = rand() * 800;
      const ay = rand() * 600;
      const before = screenToCanvas(v, ax, ay);
      const after = screenToCanvas(zoomAt(v, ax, ay, nz), ax, ay);
      assert.ok(Math.abs(before.x - after.x) < 1e-9 && Math.abs(before.y - after.y) < 1e-9);
    }
  });
});

describe("D2 composite basics", () => {
  it("empty doc -> false and zeroed out", () => {
    const out = new Uint8ClampedArray(4096).fill(7);
    assert.equal(compositeChunk(mkDoc([mkLayer(null)]), 0, 0, out), false);
    assert.deepEqual([...out], new Array(4096).fill(0));
  });

  it("opaque top covers bottom", () => {
    const doc = mkDoc([mkLayer(solidChunk(255, 0, 0, 255)), mkLayer(solidChunk(0, 0, 255, 255))]);
    const out = new Uint8ClampedArray(4096);
    assert.equal(compositeChunk(doc, 0, 0, out), true);
    assert.deepEqual([out[0], out[1], out[2], out[3]], [0, 0, 255, 255]);
    assert.deepEqual(samplePixel(doc, 5, 5), [0, 0, 255, 255]);
  });

  it("opacity 0.5 matches over()", () => {
    const doc = mkDoc([mkLayer(solidChunk(255, 0, 0, 255)), mkLayer(solidChunk(0, 0, 255, 255), { opacity: 0.5 })]);
    const want = over([255, 0, 0, 255], [0, 0, 255, 255], 0.5);
    assert.deepEqual(samplePixel(doc, 0, 0), want);
    const out = new Uint8ClampedArray(4096);
    assert.equal(compositeChunk(doc, 0, 0, out), true);
    assert.deepEqual([out[0], out[1], out[2], out[3]], want);
  });

  it("hidden layer excluded", () => {
    const doc = mkDoc([
      mkLayer(solidChunk(255, 0, 0, 255)),
      mkLayer(solidChunk(0, 0, 255, 255), { visible: false }),
    ]);
    assert.deepEqual(samplePixel(doc, 3, 3), [255, 0, 0, 255]);
  });

  it("layer order is bottom-to-top", () => {
    const redTop = mkDoc([mkLayer(solidChunk(0, 0, 255, 255)), mkLayer(solidChunk(255, 0, 0, 255))]);
    assert.deepEqual(samplePixel(redTop, 1, 1), [255, 0, 0, 255]);
  });

  it("transparent pixels normalize RGB to 0", () => {
    const dirty = solidChunk(0, 0, 0, 0);
    dirty[0] = 200;
    dirty[1] = 100;
    dirty[2] = 50;
    dirty[3] = 0;
    const doc = mkDoc([mkLayer(dirty)]);
    const out = new Uint8ClampedArray(4096);
    assert.equal(compositeChunk(doc, 0, 0, out), false);
    assert.deepEqual([out[0], out[1], out[2], out[3]], [0, 0, 0, 0]);
  });

  it("out-of-range samplePixel is transparent", () => {
    const doc = mkDoc([mkLayer(solidChunk(255, 0, 0, 255))]);
    assert.deepEqual(samplePixel(doc, -1, 0), [0, 0, 0, 0]);
    assert.deepEqual(samplePixel(doc, 64, 0), [0, 0, 0, 0]);
    assert.deepEqual(samplePixel(doc, 0, 64), [0, 0, 0, 0]);
  });

  it("fast paths equal over-only result on random chunks", () => {
    const rand = lcg(987);
    const rndByte = () => Math.floor(rand() * 256);
    const randChunk = () => {
      const d = new Uint8ClampedArray(4096);
      for (let i = 0; i < 4096; i++) d[i] = rndByte();
      return d;
    };
    const opChoices = [0, 0.25, 0.5, 0.75, 1];
    for (let t = 0; t < 8; t++) {
      const layers = [];
      const refLayers = [];
      const n = 1 + Math.floor(rand() * 3);
      for (let l = 0; l < n; l++) {
        const data = randChunk();
        const opacity = opChoices[Math.floor(rand() * opChoices.length)];
        const visible = rand() < 0.8;
        layers.push({ visible, opacity, store: storeOf([["0,0", data]]) });
        if (visible && opacity > 0) refLayers.push({ data, opacity });
      }
      const doc = mkDoc(layers);
      const out = new Uint8ClampedArray(4096);
      const got = compositeChunk(doc, 0, 0, out);
      const ref = new Uint8ClampedArray(4096);
      let refAny = false;
      for (let i = 0; i < 1024; i++) {
        let acc = [0, 0, 0, 0];
        for (const { data, opacity } of refLayers) {
          const o = i * 4;
          acc = over(acc, [data[o], data[o + 1], data[o + 2], data[o + 3]], opacity);
        }
        ref[i * 4] = acc[0];
        ref[i * 4 + 1] = acc[1];
        ref[i * 4 + 2] = acc[2];
        ref[i * 4 + 3] = acc[3];
        if (acc[3] !== 0) refAny = true;
      }
      assert.deepEqual([...out], [...ref], `chunk bytes differ (trial ${t})`);
      assert.equal(got, refAny, `return flag differs (trial ${t})`);
    }
  });
});

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
    // 32px and 64px modes look identical on screen — exactly the reported bug.
    assert.equal(tile.xs.length, 2);
    // ...and `unit` must be 32px only, so the two modes stay distinguishable.
    // A 64px canvas at zoom 1 yields 32px rungs at 0/32/64 (64 is a valid boundary)
    // while `tile` yields only 0/64 — the two modes never share a rung.
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
    // ZERO change on mid-tone pixels (the checkerboard seam) — exactly where the
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
