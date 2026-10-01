// <META - FILE SUMMARY - io tests: png, validation, roundtrip, flatten, import>
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { DrawToolError } from "../src/core/errors.js";
import { over } from "../src/core/blend.js";
import { packRGBA } from "../src/core/pixel.js";
import { PixelWriter, isAllZero } from "../src/core/chunkstore.js";
import { newDocument } from "../src/model/document.js";
import { base64ToBytes, bytesToBase64 } from "../src/io/base64.js";
import { decodePng, encodePng } from "../src/io/png.js";
import { validateDocument } from "../src/io/validate.js";
import { documentToJson, jsonToDocument, importLayerJson } from "../src/io/serialize.js";
import { flattenToRgba, exportPngBytes } from "../src/io/export_png.js";
import { sanitizeFileName } from "../src/io/file_io.js";
import { buildMetaRecord, buildChunkRecords, chunkRecordKey } from "../src/io/idb_record_builder.js";
import { compositeChunk, samplePixel } from "../src/render/composite.js";

const pngCases = JSON.parse(readFileSync(new URL("./fixtures/png_cases.json", import.meta.url)));
const validationCases = JSON.parse(readFileSync(new URL("./fixtures/validation_cases.json", import.meta.url)));

// <META - ROLE : Fill whole layer with one packed color | L21-29>
function fillLayer(doc, layerId, packed) {
  const layer = doc.getLayer(layerId);
  const w = new PixelWriter(layer.store, layerId);
  for (let y = 0; y < doc.canvas.heightPx; y++) {
    for (let x = 0; x < doc.canvas.widthPx; x++) w.set(x, y, packed);
  }
  w.finish();
}

// <META - ROLE : Snapshot all chunk bytes of a doc | L31-43>
function snapshotChunks(doc) {
  const out = [];
  for (const layer of doc.layers) {
    const chunks = [];
    layer.store.forEachChunk((cx, cy, data) => chunks.push({ cx, cy, bytes: Array.from(data) }));
    out.push({ id: layer.id, chunks });
  }
  return out;
}

describe("C8.1 png_cases + roundtrip", () => {
  it("has 5 normative cases", () => {
    assert.equal(pngCases.cases.length, 5);
  });
  for (const c of pngCases.cases) {
    it(`png case ${c.name}`, async () => {
      const bytes = base64ToBytes(c.png);
      if (c.expect_error === null) {
        const dec = await decodePng(bytes);
        assert.equal(dec.width, c.width);
        assert.equal(dec.height, c.height);
        const expect = new Uint8Array(Buffer.from(c.rgba_hex, "hex"));
        assert.deepEqual(new Uint8Array(dec.rgba), expect);
      } else {
        await assert.rejects(decodePng(bytes), (e) => e instanceof DrawToolError && e.code === c.expect_error);
      }
    });
  }
  it("encode->decode roundtrip is byte-identical", async () => {
    const w = 32;
    const h = 32;
    const rgba = new Uint8ClampedArray(w * h * 4);
    for (let i = 0; i < w * h; i++) {
      rgba[i * 4] = (i * 7) % 256;
      rgba[i * 4 + 1] = (i * 13) % 256;
      rgba[i * 4 + 2] = (i * 29) % 256;
      rgba[i * 4 + 3] = i % 2 === 0 ? 255 : 0;
      if (rgba[i * 4 + 3] === 0) {
        rgba[i * 4] = 0;
        rgba[i * 4 + 1] = 0;
        rgba[i * 4 + 2] = 0;
      }
    }
    const png = await encodePng(rgba, w, h);
    assert.equal(bytesToBase64(png).startsWith("iVBORw0KGgo"), true);
    const dec = await decodePng(png);
    assert.equal(dec.width, w);
    assert.equal(dec.height, h);
    assert.deepEqual(Array.from(dec.rgba), Array.from(rgba));
  });
});

describe("C8.2 validation 21 cases", () => {
  it("has 21 cases", () => {
    assert.equal(validationCases.cases.length, 21);
  });
  for (const c of validationCases.cases) {
    it(`validate ${c.name}`, async () => {
      const res = await validateDocument(c.document);
      assert.equal(res.ok, c.expect_valid);
      if (!c.expect_valid) {
        assert.ok(res.errors.length > 0);
        assert.equal(res.errors[0].code, c.expect_code);
      } else if (c.expect_import_error !== null && c.expect_import_error !== undefined) {
        await assert.rejects(jsonToDocument(c.document), (e) => e instanceof DrawToolError && e.code === c.expect_import_error);
      }
    });
  }
});

describe("C8.3 serialize roundtrip determinism", () => {
  it("doc->json->doc preserves bytes; double serialize identical; skips empty", async () => {
    const doc = newDocument({ widthPx: 64, heightPx: 64, background: "transparent", name: "rt" });
    fillLayer(doc, doc.layers[0].id, packRGBA(10, 20, 30, 255));
    const w = new PixelWriter(doc.layers[0].store, doc.layers[0].id);
    w.set(0, 0, packRGBA(255, 0, 0, 255));
    w.finish();
    doc.layers[0].store.putChunk(1, 1, new Uint8ClampedArray(4096));
    const j1 = await documentToJson(doc);
    const j2 = await documentToJson(doc);
    assert.equal(JSON.stringify(j1), JSON.stringify(j2));
    for (const lj of j1.layers) {
      for (const c of lj.raster.chunks) {
        const dec = await decodePng(base64ToBytes(c.png));
        assert.equal(isAllZero(dec.rgba), false);
      }
    }
    const before = snapshotChunks(doc).map((l) => ({ id: l.id, chunks: l.chunks.filter((c) => !isAllZero(new Uint8ClampedArray(c.bytes))) }));
    const { document: back, warnings } = await jsonToDocument(JSON.parse(JSON.stringify(j1)));
    assert.deepEqual(warnings, []);
    assert.equal(back.id, doc.id);
    assert.equal(back.name, doc.name);
    assert.equal(back.canvas.background, doc.canvas.background);
    assert.equal(back.activeLayerId, doc.activeLayerId);
    assert.deepEqual(snapshotChunks(back), before.map((l) => ({ ...l })));
    assert.deepEqual(back.layers.map((l) => l.id), doc.layers.map((l) => l.id));
  });
});

describe("C8.4 alpha normalization", () => {
  it("transparent pixels with RGB residue load as zero", async () => {
    const rgba = new Uint8ClampedArray(32 * 32 * 4);
    rgba[0] = 200;
    rgba[1] = 100;
    rgba[2] = 50;
    rgba[3] = 0;
    rgba[4] = 9;
    rgba[5] = 9;
    rgba[6] = 9;
    rgba[7] = 255;
    const png = await encodePng(rgba, 32, 32);
    const obj = {
      schema_version: "2.0.0",
      format: "draw_tool.document",
      document_id: "alpha-test",
      canvas: { tile_px: 64, unit_px: 32, width_px: 32, height_px: 32, background: "transparent" },
      layers: [{
        layer_id: "layer-0001", name: "a", type: "raster", visible: true, locked: false, opacity: 1, blend: "normal",
        raster: { chunk_px: 32, encoding: "png_base64", chunks: [{ cx: 0, cy: 0, png: bytesToBase64(png) }] },
      }],
    };
    const { document: back } = await jsonToDocument(obj);
    const data = back.layers[0].store.getChunk(0, 0);
    assert.ok(data);
    assert.deepEqual([data[0], data[1], data[2], data[3]], [0, 0, 0, 0]);
    assert.deepEqual([data[4], data[5], data[6], data[7]], [9, 9, 9, 255]);
  });
});

describe("C8.5 importLayerJson", () => {
  it("drops OOB, reissues id, warns on size mismatch", async () => {
    const doc = newDocument({ widthPx: 64, heightPx: 64 });
    const mk = async (fill) => {
      const rgba = new Uint8ClampedArray(32 * 32 * 4);
      rgba.fill(fill);
      if (fill !== 0) rgba[3] = 255;
      return bytesToBase64(await encodePng(rgba, 32, 32));
    };
    const inside = await mk(7);
    const outside = await mk(9);
    const obj = {
      schema_version: "2.0.0",
      format: "draw_tool.layer",
      source_canvas: { tile_px: 64, unit_px: 32, width_px: 96, height_px: 96, background: "transparent" },
      layer: {
        layer_id: "layer-9999", name: "imp", type: "raster", visible: true, locked: false, opacity: 1, blend: "normal",
        raster: { chunk_px: 32, encoding: "png_base64", chunks: [{ cx: 0, cy: 0, png: inside }, { cx: 2, cy: 2, png: outside }] },
      },
    };
    const { layer, dropped, warnings } = await importLayerJson(doc, obj);
    assert.equal(dropped, 1);
    assert.ok(warnings.length >= 1);
    assert.equal(layer.name, "imp");
    assert.notEqual(layer.id, "layer-9999");
    assert.ok(layer.store.getChunk(0, 0));
    assert.equal(doc.findLayer(layer.id), null);
  });
});

describe("C8.6 flattenToRgba", () => {
  it("opaque top wins; opacity blends per over(); background in/out; matches composite", async () => {
    const doc = newDocument({ widthPx: 32, heightPx: 32, background: "#ff0000" });
    fillLayer(doc, doc.layers[0].id, packRGBA(0, 0, 255, 255));
    const { document: doc2 } = await jsonToDocument(await documentToJson(doc));
    void doc2;
    const top = (await import("../src/model/layer.js")).Layer.create({ id: doc.ids.next(), name: "top", widthPx: 32, heightPx: 32 });
    doc._insert(top, 1);
    fillLayer(doc, top.id, packRGBA(0, 255, 0, 255));
    const flat = flattenToRgba(doc, { includeBackground: true });
    assert.deepEqual([flat[0], flat[1], flat[2], flat[3]], [0, 255, 0, 255]);
    top.opacity = 0.5;
    const flat2 = flattenToRgba(doc, { includeBackground: false });
    const expect = over([0, 0, 255, 255], [0, 255, 0, 255], 0.5);
    assert.deepEqual([flat2[0], flat2[1], flat2[2], flat2[3]], expect);
    const bg = flattenToRgba(doc, { includeBackground: true });
    void bg;
    const solo = newDocument({ widthPx: 32, heightPx: 32, background: "#ff0000" });
    const onlyBg = flattenToRgba(solo, { includeBackground: true });
    assert.deepEqual([onlyBg[0], onlyBg[1], onlyBg[2], onlyBg[3]], [255, 0, 0, 255]);
    const noBg = flattenToRgba(solo, { includeBackground: false });
    assert.deepEqual([noBg[0], noBg[1], noBg[2], noBg[3]], [0, 0, 0, 0]);
    for (const [x, y] of [[0, 0], [5, 7], [31, 31]]) {
      const s = samplePixel(doc, x, y);
      const o = (y * 32 + x) * 4;
      assert.deepEqual([flat2[o], flat2[o + 1], flat2[o + 2], flat2[o + 3]], s);
    }
    const chunk = new Uint8ClampedArray(4096);
    compositeChunk({ layers: [{ visible: true, opacity: 1, store: solo.layers[0].store }], canvas: solo.canvas }, 0, 0, chunk);
    const pngBytes = await exportPngBytes(solo, { includeBackground: true });
    const dec = await decodePng(pngBytes);
    assert.equal(dec.width, 32);
    assert.equal(dec.height, 32);
    assert.deepEqual([dec.rgba[0], dec.rgba[1], dec.rgba[2], dec.rgba[3]], [255, 0, 0, 255]);
  });
});

describe("C8.7 store helpers + file name", () => {
  it("buildMetaRecord/buildChunkRecords/chunkRecordKey shapes", () => {
    const doc = newDocument({ widthPx: 64, heightPx: 32 });
    fillLayer(doc, doc.layers[0].id, packRGBA(1, 2, 3, 255));
    const meta = buildMetaRecord(doc);
    assert.equal(meta.key, "current");
    assert.equal(meta.schema, 2);
    assert.equal(meta.documentId, doc.id);
    assert.equal(typeof meta.updatedAt, "number");
    assert.deepEqual(meta.canvas, { widthPx: 64, heightPx: 32, background: "transparent" });
    assert.equal(meta.layers.length, 1);
    assert.equal(chunkRecordKey("layer-0001", 3, 2), "layer-0001|2|3");
    const recs = buildChunkRecords(doc.layers[0], [{ cx: 0, cy: 0 }, { cx: 1, cy: 0 }]);
    assert.equal(recs.length, 2);
    assert.ok(recs[0].data instanceof ArrayBuffer);
    assert.equal(recs[0].data.byteLength, 4096);
  });
  it("sanitizeFileName", () => {
    assert.equal(sanitizeFileName("a/b:c*d"), "a_b_c_d");
    assert.equal(sanitizeFileName("  ..  "), "untitled");
    assert.equal(sanitizeFileName("x".repeat(200)).length <= 80, true);
  });
});
