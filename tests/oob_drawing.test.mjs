// Drawing outside the canvas must not leave chunk storage behind.
//
// Found by driving the real UI in a browser: a rect dragged so it fell entirely
// off the top-left left 104 all-transparent chunks in the layer. They are
// invisible, but every composite walks them and autosave writes them back.
//
// PixelWriter.set() is documented to silently ignore out-of-viewport writes, so
// the question this pins is whether an ignored write still allocates a chunk.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { newDocument } from "../src/features/document/document.js";
import { PixelWriter } from "../src/core/chunkstore.js";
import { applyShape } from "../src/core/raster/shape_raster.js";
import { MAX_CHUNKS_X } from "../src/core/constants.js";

const CANVAS = 512;

// <META - ROLE : paint one shape through the REAL writer into the layer | L18-24>
function paint(doc, layer, spec) {
  const w = new PixelWriter(layer.store, layer.id);
  const n = applyShape(w, spec, 0xff0000ff, 0xff0000ff);
  const cs = w.finish();
  return { painted: n, changeSet: cs };
}

// <META - ROLE : every stored chunk coordinate, via the public iterator | L18-22>
function chunkKeys(store) {
  const keys = [];
  store.forEachChunk((cx, cy) => keys.push(cy * MAX_CHUNKS_X + cx));
  return keys;
}

// <META - ROLE : true when a chunk holds at least one non-zero byte | L24-27>
function hasPixels(store, cx, cy) {
  const d = store.getChunk(cx, cy);
  return !!d && d.some((b) => b !== 0);
}

describe("out-of-canvas drawing leaves no storage behind", () => {
  it("a fully out-of-bounds shape stores nothing even though the mask is built", () => {
    // applyShape reports the mask size, not the painted-on-canvas count: the
    // contract (docs/contract.md §2 rule 3) says PixelWriter.set() silently
    // ignores out-of-viewport writes, so the painter does not clip. What matters
    // is that no storage is allocated.
    const doc = newDocument({ widthPx: CANVAS, heightPx: CANVAS, name: "oob" });
    const layer = doc.layers[0];
    assert.equal(layer.store.chunkCount(), 0, "a new document stores nothing");

    for (const bbox of [
      { x: -400, y: -400, w: 32, h: 32 },
      { x: CANVAS + 100, y: CANVAS + 100, w: 32, h: 32 },
      { x: -100, y: 200, w: 32, h: 32 },
      { x: 200, y: -100, w: 32, h: 32 },
    ]) {
      const { changeSet } = paint(doc, layer, { kind: "rect", bbox, strokeWidth: 1, fillMode: "fill" });
      assert.equal(changeSet, null, `${JSON.stringify(bbox)} must produce no change set`);
    }
    assert.equal(layer.store.chunkCount(), 0, "and allocate no chunk");
    assert.equal(layer.store.pruneEmpty(), 0, "nothing to prune either");
  });

  it("a shape straddling the edge stores only the in-canvas part", () => {
    const doc = newDocument({ widthPx: CANVAS, heightPx: CANVAS, name: "edge" });
    const layer = doc.layers[0];
    const { painted } = paint(doc, layer, { kind: "rect", bbox: { x: -16, y: -16, w: 48, h: 48 }, strokeWidth: 1, fillMode: "fill" });
    assert.ok(painted > 0, "the visible half must paint");
    assert.deepEqual(chunkKeys(layer.store), [0], "only chunk (0,0) is stored");
    assert.equal(hasPixels(layer.store, 0, 0), true, "and it holds the visible pixels");
  });

  it("a line running off both ends stores exactly its in-canvas run", () => {
    const doc = newDocument({ widthPx: CANVAS, heightPx: CANVAS, name: "line-oob" });
    const layer = doc.layers[0];
    paint(doc, layer, { kind: "line", p0: { x: -200, y: 256 }, p1: { x: 700, y: 256 }, strokeWidth: 2 });
    const keys = chunkKeys(layer.store);
    assert.ok(keys.length > 0, "the visible run is stored");
    const wrongRow = keys.filter((k) => Math.floor(k / MAX_CHUNKS_X) !== 8 || k < 0);
    assert.deepEqual(wrongRow, [], "every stored chunk is inside the canvas, all in row 8");
    for (const k of keys) {
      assert.equal(hasPixels(layer.store, k % MAX_CHUNKS_X, Math.floor(k / MAX_CHUNKS_X)), true,
        `chunk ${k} holds real pixels`);
    }
  });

  it("a huge rect covering everything allocates no chunk outside the canvas", () => {
    // The mirror image of the bug: a shape far larger than the canvas must not
    // spill into chunk slots beyond the canvas bounds. Bounds are derived from
    // the canvas, not from the key arithmetic (chunk keys are MAX_CHUNKS_X wide).
    const doc = newDocument({ widthPx: CANVAS, heightPx: CANVAS, name: "big" });
    const layer = doc.layers[0];
    paint(doc, layer, { kind: "rect", bbox: { x: -5000, y: -5000, w: 20000, h: 20000 }, strokeWidth: 1, fillMode: "fill" });
    const maxCx = CANVAS / 32;
    const keys = [];
    layer.store.forEachChunk((cx, cy) => {
      if (cx < 0 || cy < 0 || cx >= maxCx || cy >= maxCx) keys.push([cx, cy]);
    });
    assert.deepEqual(keys, [], `no chunk outside the ${maxCx}x${maxCx} canvas grid`);
    assert.equal(layer.store.chunkCount(), maxCx * maxCx, "exactly the cells of a 512x512 canvas");
  });
});
