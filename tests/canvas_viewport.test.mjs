// Logical viewport: express an arbitrary export size without disturbing the
// 32px chunk alignment the stored canvas depends on.
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { newDocument, validateViewport } from "../src/model/document.js";
import { exportPngBytes, flattenToRgba } from "../src/io/export_png.js";
import { documentToJson, jsonToDocument } from "../src/io/serialize.js";
import { decodePng } from "../src/io/png.js";

// <META - ROLE : Paint one opaque marker pixel per axis so crop errors show | L15-21>
function paint(doc, x, y, packed) {
  for (const layer of doc.layers) layer.store._setPixel(x, y, packed);
}

const rgba = (r, g, b, a) => ((a << 24) | (r << 0) | (g << 8) | (b << 16)) >>> 0;

describe("validateViewport", () => {
  it("accepts an in-range integer region and normalizes it", () => {
    assert.deepEqual(validateViewport({ x: 0, y: 0, w: 280, h: 560 }, 288, 576), { x: 0, y: 0, w: 280, h: 560 });
    assert.deepEqual(validateViewport({ x: 4, y: 8, w: 280, h: 560 }, 288, 576), { x: 4, y: 8, w: 280, h: 560 });
  });

  it("treats absent as null (export the whole canvas)", () => {
    assert.equal(validateViewport(undefined, 288, 576), null);
    assert.equal(validateViewport(null, 288, 576), null);
  });

  it("rejects non-integers, non-positive size, and out-of-range regions", () => {
    // DrawToolError carries a code property; assert on that rather than the message.
    const code = (fn) => {
      try { fn(); } catch (e) { return e.code; }
      return null;
    };
    assert.equal(code(() => validateViewport({ x: 0, y: 0, w: 1.5, h: 8 }, 288, 576)), "VIEWPORT_INVALID");
    assert.equal(code(() => validateViewport({ x: 0, y: 0, w: 0, h: 8 }, 288, 576)), "VIEWPORT_INVALID");
    assert.equal(code(() => validateViewport({ x: 0, y: 0, w: 8, h: 8 }, 288, 576)), null, "in-range is accepted");
    assert.equal(code(() => validateViewport({ x: -1, y: 0, w: 8, h: 8 }, 288, 576)), "VIEWPORT_OUT_OF_RANGE");
    // 9 + 560 = 569 <= 576 is still in range; the boundary case is y = 16.
    assert.equal(code(() => validateViewport({ x: 4, y: 9, w: 280, h: 560 }, 288, 576)), null);
    assert.equal(code(() => validateViewport({ x: 4, y: 17, w: 280, h: 560 }, 288, 576)), "VIEWPORT_OUT_OF_RANGE");
    assert.equal(code(() => validateViewport({ x: 9, y: 8, w: 280, h: 560 }, 288, 576)), "VIEWPORT_OUT_OF_RANGE");
  });
});

describe("canvas keeps chunk alignment under a viewport", () => {
  it("stores a non-32 viewport without resizing the canvas", () => {
    const doc = newDocument({ widthPx: 288, heightPx: 576, viewport: { x: 4, y: 8, w: 280, h: 560 } });
    assert.equal(doc.canvas.widthPx, 288, "canvas width untouched");
    assert.equal(doc.canvas.heightPx, 576, "canvas height untouched");
    assert.deepEqual(doc.canvas.viewport, { x: 4, y: 8, w: 280, h: 560 });
    // Layer stores still match the 32-multiple canvas, so chunkKey alignment holds.
    for (const l of doc.layers) {
      assert.equal(l.store.widthPx, 288);
      assert.equal(l.store.heightPx, 576);
    }
  });

  it("_setViewport clears with null and re-validates against current size", () => {
    const doc = newDocument({ widthPx: 288, heightPx: 576 });
    assert.equal(doc.canvas.viewport, null);
    assert.deepEqual(doc._setViewport({ x: 4, y: 8, w: 280, h: 560 }), { x: 4, y: 8, w: 280, h: 560 });
    assert.equal(doc._setViewport(null), null);
    assert.throws(() => doc._setViewport({ x: 0, y: 0, w: 400, h: 600 }), (e) => e.code === "VIEWPORT_OUT_OF_RANGE");
  });

  it("a rejected viewport leaves the previous one intact", () => {
    const doc = newDocument({ widthPx: 288, heightPx: 576 });
    doc._setViewport({ x: 4, y: 8, w: 280, h: 560 });
    assert.throws(() => doc._setViewport({ x: 0, y: 0, w: 9999, h: 1 }), (e) => e.code === "VIEWPORT_OUT_OF_RANGE");
    assert.deepEqual(doc.canvas.viewport, { x: 4, y: 8, w: 280, h: 560 }, "unchanged after a rejected set");
  });
});

describe("export crop", () => {
  it("default export ignores the viewport and emits canvas-sized PNG", async () => {
    const doc = newDocument({ widthPx: 288, heightPx: 576, viewport: { x: 4, y: 8, w: 280, h: 560 } });
    const png = await decodePng(await exportPngBytes(doc));
    assert.equal(png.width, 288, "full canvas width");
    assert.equal(png.height, 576, "full canvas height");
  });

  it("cropToViewport emits viewport-sized PNG", async () => {
    const doc = newDocument({ widthPx: 288, heightPx: 576, viewport: { x: 4, y: 8, w: 280, h: 560 } });
    const png = await decodePng(await exportPngBytes(doc, { cropToViewport: true }));
    assert.equal(png.width, 280, "viewport width");
    assert.equal(png.height, 560, "viewport height");
  });

  it("crop maps viewport pixels 1:1 and excludes everything outside", async () => {
    const doc = newDocument({ widthPx: 288, heightPx: 576, viewport: { x: 4, y: 8, w: 280, h: 560 } });
    // Marker on the first in-viewport pixel, and one just outside each edge.
    paint(doc, 4, 8, rgba(255, 0, 0, 255));      // in-viewport origin
    paint(doc, 3, 8, rgba(0, 255, 0, 255));      // one px left of the viewport
    paint(doc, 4, 7, rgba(0, 0, 255, 255));      // one px above the viewport
    const png = await decodePng(await exportPngBytes(doc, { cropToViewport: true }));
    const at = (x, y) => {
      const i = (y * png.width + x) * 4;
      return [png.rgba[i], png.rgba[i + 1], png.rgba[i + 2], png.rgba[i + 3]];
    };
    assert.deepEqual(at(0, 0), [255, 0, 0, 255], "viewport origin is the painted marker");
    assert.deepEqual(at(1, 0), [0, 0, 0, 0], "neighbour inside the viewport is empty");
    assert.deepEqual(at(0, 1), [0, 0, 0, 0], "row below origin is empty");
    // The outside markers must not appear anywhere in the cropped output.
    let green = 0, blue = 0;
    for (let i = 0; i < png.width * png.height * 4; i += 4) {
      if (png.rgba[i + 1] === 255 && png.rgba[i] === 0) green++;
      if (png.rgba[i + 2] === 255 && png.rgba[i] === 0) blue++;
    }
    assert.equal(green, 0, "left-of-viewport pixel excluded");
    assert.equal(blue, 0, "above-viewport pixel excluded");
  });

  it("crop is exactly the viewport sub-rectangle of the full flatten", async () => {
    const doc = newDocument({ widthPx: 288, heightPx: 576, viewport: { x: 4, y: 8, w: 280, h: 560 } });
    for (let i = 0; i < 40; i++) paint(doc, 10 + i, 20 + i * 7, rgba(200, 100 + i, 40, 255));
    const full = flattenToRgba(doc);
    const cropped = await decodePng(await exportPngBytes(doc, { cropToViewport: true }));
    for (let y = 0; y < 560; y += 37) {
      for (let x = 0; x < 280; x += 41) {
        const s = ((8 + y) * 288 + (4 + x)) * 4;
        const d = (y * cropped.width + x) * 4;
        assert.deepEqual(
          [cropped.rgba[d], cropped.rgba[d + 1], cropped.rgba[d + 2], cropped.rgba[d + 3]],
          [full[s], full[s + 1], full[s + 2], full[s + 3]],
          `crop(${x},${y}) equals full-canvas pixel at (${4 + x},${8 + y})`,
        );
      }
    }
  });
});

describe("viewport round-trips through save/load", () => {
  const VP = { x: 4, y: 8, w: 280, h: 560 };

  it("survives a serialize/deserialize cycle", async () => {
    const doc = newDocument({ widthPx: 288, heightPx: 576, viewport: VP });
    paint(doc, 4, 8, rgba(255, 0, 0, 255));
    const json = await documentToJson(doc);
    assert.deepEqual(json.canvas.viewport, VP, "viewport is written to canvas");
    assert.equal(json.canvas.width_px, 288, "stored size stays a 32 multiple");
    assert.equal(json.canvas.height_px, 576);

    const back = (await jsonToDocument(json)).document;
    assert.deepEqual(back.canvas.viewport, VP, "viewport is restored");
    assert.equal(back.canvas.widthPx, 288);
    assert.equal(back.canvas.heightPx, 576);
  });

  it("omits the key entirely when no viewport is set", async () => {
    const doc = newDocument({ widthPx: 288, heightPx: 576 });
    const json = await documentToJson(doc);
    assert.equal("viewport" in json.canvas, false, "no viewport key when unset");
    const back = (await jsonToDocument(json)).document;
    assert.equal(back.canvas.viewport, null, "defaults to null");
  });

  it("a crop of a reloaded document matches the original crop", async () => {
    const doc = newDocument({ widthPx: 288, heightPx: 576, viewport: VP });
    for (let i = 0; i < 30; i++) paint(doc, 4 + i * 3, 8 + i * 5, rgba(10 + i, 200, 30, 255));
    const before = await decodePng(await exportPngBytes(doc, { cropToViewport: true }));
    const back = (await jsonToDocument(await documentToJson(doc))).document;
    const after = await decodePng(await exportPngBytes(back, { cropToViewport: true }));
    assert.equal(after.width, before.width);
    assert.equal(after.height, before.height);
    assert.deepEqual([...after.rgba], [...before.rgba], "pixels survive the round trip");
  });
});
