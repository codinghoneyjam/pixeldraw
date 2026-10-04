import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import {
  TILE_PX,
  UNIT_PX,
  CHUNK_PX,
  CHUNK_LEN,
  MIN_SIZE_PX,
  MAX_W_PX,
  MAX_H_PX,
  MAX_CHUNKS_X,
  MAX_CHUNKS_Y,
  PEN_MIN,
  PEN_MAX,
  PEN_DEFAULT,
  MAX_LAYERS,
  UNDO_MAX_STEPS,
  UNDO_MAX_BYTES,
  ZOOM_LEVELS,
  SCHEMA_VERSION,
  isValidCanvasSize,
  chunkKey,
  chunkCoords,
} from "../src/core/constants.js";
import { packRGBA, unpackRGBA, parseHex, toHex, TRANSPARENT } from "../src/core/pixel.js";
import { over } from "../src/core/blend.js";
import { ChunkStore, PixelWriter, isAllZero, changeSetBytes } from "../src/core/chunkstore.js";
import { brushFootprint, forEachBresenham, strokeSegment, strokePoint } from "../src/core/brush.js";

const G = JSON.parse(
  readFileSync(new URL("./fixtures/raster_golden.json", import.meta.url))
);

describe("constants", () => {
  it("table values", () => {
    assert.equal(TILE_PX, 64);
    assert.equal(UNIT_PX, 32);
    assert.equal(CHUNK_PX, 32);
    assert.equal(CHUNK_LEN, 4096);
    assert.equal(MIN_SIZE_PX, 32);
    assert.equal(MAX_W_PX, 2048);
    assert.equal(MAX_H_PX, 1088);
    assert.equal(MAX_CHUNKS_X, 64);
    assert.equal(MAX_CHUNKS_Y, 34);
    assert.equal(PEN_MIN, 1);
    assert.equal(PEN_MAX, 64);
    assert.equal(PEN_DEFAULT, 1);
    assert.equal(MAX_LAYERS, 64);
    assert.equal(UNDO_MAX_STEPS, 200);
    assert.equal(UNDO_MAX_BYTES, 96 * 1024 * 1024);
    assert.deepEqual([...ZOOM_LEVELS], [0.25, 0.5, 1, 2, 3, 4, 6, 8, 12, 16, 24, 32]);
    assert.equal(SCHEMA_VERSION, "2.0.0");
  });
  it("isValidCanvasSize", () => {
    assert.equal(isValidCanvasSize(31, 32), false);
    assert.equal(isValidCanvasSize(33, 32), false);
    assert.equal(isValidCanvasSize(2080, 32), false);
    assert.equal(isValidCanvasSize(32, 1120), false);
    assert.equal(isValidCanvasSize(32, 32), true);
    assert.equal(isValidCanvasSize(1920, 1088), true);
    assert.equal(isValidCanvasSize(512, 512), true);
  });
  it("isValidCanvasSize 2048 boundary", () => {
    assert.equal(isValidCanvasSize(2048, 128), true);
    assert.equal(isValidCanvasSize(2048 + 32, 128), false);
  });
});

describe("chunkKey/chunkCoords", () => {
  it("boundary values with MAX_CHUNKS_X = 64 stride", () => {
    assert.equal(chunkKey(63, 0), 63);
    assert.deepEqual(chunkCoords(64), { cx: 0, cy: 1 });
    assert.deepEqual(chunkCoords(chunkKey(63, 33)), { cx: 63, cy: 33 });
  });
});

describe("pixel", () => {
  it("alpha 0 normalizes to 0", () => {
    assert.equal(packRGBA(255, 128, 64, 0), 0);
    assert.equal(packRGBA(9, 9, 9, 0), 0);
    assert.equal(TRANSPARENT, 0);
  });
  it("round trip with byte order [r,g,b,a]", () => {
    const p = packRGBA(10, 20, 30, 255);
    assert.deepEqual(unpackRGBA(p), [10, 20, 30, 255]);
    const bytes = new Uint8ClampedArray(4);
    new Uint32Array(bytes.buffer)[0] = p;
    assert.deepEqual([...bytes], [10, 20, 30, 255]);
  });
  it("parseHex / toHex", () => {
    assert.deepEqual(parseHex("#f0a"), { r: 255, g: 0, b: 170, a: 255 });
    assert.deepEqual(parseHex("#FF00AA"), { r: 255, g: 0, b: 170, a: 255 });
    assert.deepEqual(parseHex("#11223344"), { r: 17, g: 34, b: 51, a: 68 });
    assert.equal(parseHex("#12"), null);
    assert.equal(parseHex("red"), null);
    assert.equal(parseHex("#gggggg"), null);
    assert.equal(toHex(255, 0, 170), "#ff00aa");
  });
});

describe("chunkstore", () => {
  it("boundary read/write, sparsity, prune, clone, resize", () => {
    const s = new ChunkStore(64, 32);
    const red = packRGBA(255, 0, 0, 255);
    assert.equal(s.getPixel(31, 0), 0);
    assert.equal(s.getPixel(64, 0), 0);
    s._setPixel(31, 0, red);
    assert.equal(s.getPixel(31, 0), red);
    assert.equal(s.getPixel(32, 0), 0);
    s._setPixel(64, 0, red);
    assert.equal(s.getPixel(64, 0), 0);
    const t = new ChunkStore(32, 32);
    t._setPixel(5, 5, 0);
    assert.equal(t.chunkCount(), 0);
    t._setPixel(5, 5, red);
    t._setPixel(5, 5, 0);
    assert.equal(t.chunkCount(), 1);
    assert.equal(t.pruneEmpty(), 1);
    assert.equal(t.chunkCount(), 0);
    const c = s.clone();
    c._setPixel(0, 0, red);
    assert.equal(s.getPixel(0, 0), 0);
    assert.equal(c.getPixel(0, 0), red);
    const r = new ChunkStore(64, 64);
    r._setPixel(40, 40, red);
    r._setPixel(0, 0, red);
    const removed = r.resizeTo(32, 32);
    assert.equal(removed.length, 1);
    assert.deepEqual([removed[0].cx, removed[0].cy], [1, 1]);
    assert.equal(r.getPixel(0, 0), red);
  });
  it("isAllZero", () => {
    assert.equal(isAllZero(new Uint8ClampedArray(4096)), true);
    const d = new Uint8ClampedArray(4096);
    d[100] = 1;
    assert.equal(isAllZero(d), false);
  });
});

describe("pixelwriter", () => {
  it("two-chunk stroke, null on repaint, discard, invalid state, takeDirty", () => {
    const s = new ChunkStore(64, 32);
    const red = packRGBA(255, 0, 0, 255);
    const w = new PixelWriter(s, "l1");
    w.set(31, 0, red);
    w.set(32, 0, red);
    const dirty = w.takeDirty();
    assert.deepEqual(dirty, [
      { cx: 0, cy: 0 },
      { cx: 1, cy: 0 },
    ]);
    assert.deepEqual(w.takeDirty(), []);
    const cs = w.finish();
    assert.notEqual(cs, null);
    assert.equal(cs.layerId, "l1");
    assert.deepEqual(
      cs.chunks.map((c) => [c.cx, c.cy]),
      [
        [0, 0],
        [1, 0],
      ]
    );
    assert.equal(cs.chunks[0].before, null);
    assert.notEqual(cs.chunks[0].after, null);
    assert.equal(changeSetBytes(cs), 8192);
    const w2 = new PixelWriter(s, "l1");
    w2.set(31, 0, red);
    assert.equal(w2.finish(), null);
    const w3 = new PixelWriter(s, "l1");
    const blue = packRGBA(0, 0, 255, 255);
    w3.set(0, 0, blue);
    const touched = w3.discard();
    assert.deepEqual(touched, [{ cx: 0, cy: 0 }]);
    assert.equal(s.getPixel(0, 0), 0);
    assert.throws(() => w3.set(1, 1, blue), (e) => e.code === "INVALID_STATE");
    assert.throws(() => w.finish(), (e) => e.code === "INVALID_STATE");
  });
});

describe("brush vs golden sample", () => {
  it("brush footprints match all golden", () => {
    for (const c of G.brush) {
      const fp = brushFootprint(c.n);
      const rows = [];
      for (let y = 0; y < c.n; y++) {
        let row = "";
        for (let x = 0; x < c.n; x++) row += fp.mask[y * c.n + x] === 1 ? "#" : ".";
        rows.push(row);
      }
      assert.deepEqual(rows, c.rows, `brush ${c.n}`);
    }
  });
  it("hashes brush 64 / brush 33", () => {
    for (const what of ["brush 64", "brush 33"]) {
      const n = what === "brush 64" ? 64 : 33;
      const fp = brushFootprint(n);
      const lines = [];
      for (let y = 0; y < n; y++) {
        let row = "";
        for (let x = 0; x < n; x++) row += fp.mask[y * n + x] === 1 ? "#" : ".";
        lines.push(row);
      }
      const sha = createHash("sha256").update(lines.join("\n")).digest("hex");
      const g = G.hashes.find((h) => h.what === what);
      assert.equal(sha, g.sha256, what);
    }
  });
  it("bresenham matches golden", () => {
    for (const c of G.bresenham) {
      const pts = [];
      forEachBresenham(c.p0[0], c.p0[1], c.p1[0], c.p1[1], (x, y) => pts.push([x, y]));
      assert.deepEqual(pts, c.points, `bresenham ${c.p0}->${c.p1}`);
    }
  });
  it("stroke matches all golden", () => {
    for (const c of G.stroke) {
      const grid = Array.from({ length: c.H }, () => new Array(c.W).fill(0));
      const fake = { set: (x, y) => void (x >= 0 && y >= 0 && x < c.W && y < c.H && (grid[y][x] = 1)) };
      if (c.points.length === 1) {
        strokePoint(fake, c.points[0][0], c.points[0][1], c.n, 1);
      } else {
        for (let i = 1; i < c.points.length; i++) {
          strokeSegment(fake, c.points[i - 1][0], c.points[i - 1][1], c.points[i][0], c.points[i][1], c.n, 1);
        }
      }
      assert.deepEqual(
        grid.map((r) => r.map((v) => (v ? "#" : ".")).join("")),
        c.rows,
        `stroke n=${c.n}`
      );
    }
  });
});

describe("over vs golden sample", () => {
  it("all composite vectors", () => {
    for (const c of G.composite) {
      assert.deepEqual(over(c.dst, c.src, c.opacity), c.expect, JSON.stringify(c));
    }
  });
});
