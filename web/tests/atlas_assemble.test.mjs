// <META - FILE SUMMARY - Atlas assembly fixtures: PIL paste-law goldens and assembly checks>

import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";

import { assembleSheet, pasteChannel, pastePixel } from "../src/core/raster/atlas.js";
import { DrawToolError } from "../src/core/errors.js";

// The legacy baker's paste law, derived by brute force over all 256 alpha
// values in the Python oracle and cross-checked on 2000 random RGBA inputs.
// <META - ROLE : paste law reproduces PIL channel blending exactly | L18-52>
test("pasteChannel follows PIL's (c*a+127)/255 blend", () => {
  assert.equal(pasteChannel(0, 200, 0), 0);
  assert.equal(pasteChannel(0, 200, 255), 200);
  assert.equal(pasteChannel(0, 200, 128), 100);
  assert.equal(pasteChannel(0, 200, 64), 50);
  assert.equal(pasteChannel(0, 255, 90), 90);
  // Rounding is +127/255 (round-half-up), not truncation. Verified against PIL:
  // src (200,100,50) a=1 -> (1,0,0,0) and a=200 -> (157,78,39,157).
  assert.equal(pasteChannel(0, 200, 1), 1);
  assert.equal(pasteChannel(0, 200, 200), 157);
  assert.equal(pasteChannel(0, 1, 127), 0);
  assert.equal(pasteChannel(0, 1, 128), 1);
});

// Alpha is squared because the tile is used as its own mask.
// <META - ROLE : self-masked paste squares alpha and premultiplies colour | L54-67>
test("pastePixel squares alpha on a transparent destination", () => {
  const cases = [
    [[200, 100, 50, 255], [200, 100, 50, 255]],
    [[200, 100, 50, 128], [100, 50, 25, 64]],
    [[200, 100, 50, 64], [50, 25, 13, 16]],
    [[200, 100, 50, 90], [71, 35, 18, 32]],
    [[200, 100, 50, 240], [188, 94, 47, 226]],
    [[200, 100, 50, 45], [35, 18, 9, 8]],
    [[0, 0, 0, 0], [0, 0, 0, 0]],
  ];
  for (const [src, want] of cases) {
    assert.deepEqual(pastePixel([0, 0, 0, 0], src), want, `src=${src}`);
  }
});

// Every alpha value must match the closed form, which is how the oracle derived it.
// <META - ROLE : closed form holds across the whole 0..255 alpha range | L69-83>
test("closed form holds for all 256 alpha values", () => {
  for (let a = 0; a < 256; a++) {
    const got = pastePixel([0, 0, 0, 0], [200, 100, 50, a]);
    const want = [
      Math.floor((200 * a + 127) / 255),
      Math.floor((100 * a + 127) / 255),
      Math.floor((50 * a + 127) / 255),
      Math.floor((a * a + 127) / 255),
    ];
    assert.deepEqual(got, want, `alpha=${a}`);
  }
});

// <META - ROLE : single tile is premultiplied, not copied through | L86-102>
test("assembleSheet applies the paste law to a lone tile", () => {
  // Packed RGBA little-endian words: 0xAARRGGBB as read by >>> 24 for alpha.
  // 0xff808080 -> a=255 r=128 g=128 b=128, alpha 255 means no premultiply.
  // 0x80640000 -> a=128 r=0   g=0   b=100 -> (0,0,50,64) after the paste law.
  const tile = { w: 2, h: 1, data: new Uint32Array([0xff808080, 0x80640000]) };
  const out = assembleSheet([tile]);
  assert.equal(out.w, 2);
  assert.equal(out.h, 1);
  assert.deepEqual([...out.data], [128, 128, 128, 255, 0, 0, 50, 64]);
});

// <META - ROLE : tiles are laid out left to right in the given order | L104-118>
test("assembleSheet places tiles left to right without overlap", () => {
  // Alpha 255 so the paste law is the identity and the assertion is about layout.
  const opaque = (r) => ({ w: 1, h: 1, data: new Uint32Array([(255 << 24) | r]) });
  const out = assembleSheet([opaque(0x10), opaque(0x20), opaque(0x30)]);
  assert.equal(out.w, 3);
  assert.equal(out.h, 1);
  assert.deepEqual([...out.data], [
    0x10, 0, 0, 255,
    0x20, 0, 0, 255,
    0x30, 0, 0, 255,
  ]);
});

// <META - ROLE : 16 slots of 128px produce the 2048x128 enemy sheet shape | L120-136>
test("assembleSheet produces the 2048x128 enemy sheet geometry", () => {
  const tile = { w: 128, h: 128, data: new Uint32Array(128 * 128) };
  const tiles = Array.from({ length: 16 }, () => tile);
  const out = assembleSheet(tiles);
  assert.equal(out.w, 2048);
  assert.equal(out.h, 128);
  assert.equal(out.data.length, 2048 * 128 * 4);
});

// Tiles carry packed RGBA in a Uint32Array (one word per pixel).
// The guards throw DrawToolError (contract §2-7), not a bare RangeError, so the
// predicate checks the code - the same shape every other raster error test uses.
// <META - ROLE : reject empty, mismatched and short tiles | L138-154>
test("assembleSheet rejects empty, mismatched and short tiles", () => {
  const codeOf = (fn) => {
    try {
      fn();
      return null;
    } catch (e) {
      return e instanceof DrawToolError ? e.code : e.constructor.name;
    }
  };
  assert.equal(codeOf(() => assembleSheet([])), "INVALID_STATE");
  assert.equal(codeOf(() => assembleSheet(null)), "INVALID_STATE");
  assert.equal(
    codeOf(() => assembleSheet([
      { w: 4, h: 4, data: new Uint32Array(16) },
      { w: 8, h: 4, data: new Uint32Array(32) },
    ])),
    "INVALID_STATE",
  );
  // A 1x1 tile is a legal size, so it fails the data-length check instead.
  assert.equal(codeOf(() => assembleSheet([{ w: 1, h: 1, data: new Uint32Array(4) }])), "INVALID_STATE");
  // A 0x4 tile is an illegal size, caught by assertMaskSize before the length check.
  assert.equal(codeOf(() => assembleSheet([{ w: 0, h: 4, data: new Uint32Array(0) }])), "OUT_OF_RANGE");
});

// The PIL oracle lives in tools/gen_fixtures.py (group "paste"); this pins the
// same vectors against the PRODUCTION module, not just the reference port.
// <META - ROLE : production assembler matches the committed PIL paste goldens | L152-176>
test("assembleSheet reproduces the committed paste goldens", () => {
  const golden = JSON.parse(
    readFileSync(new URL("./fixtures/raster_golden.json", import.meta.url), "utf-8"),
  );
  assert.ok(Array.isArray(golden.paste) && golden.paste.length > 0, "paste goldens missing");
  const chan = (v) => v.toString(16).padStart(2, "0").toUpperCase();
  for (const c of golden.paste) {
    // Golden inputs are "%02X%02X%02X%02X" == RRGGBBAA (PIL row order).
    // assembleSheet works in packed Uint32 with R in the LOW byte (the layout
    // mask writers produce), so convert on the way in and on the way out.
    const rgbaToPacked = (r, g, b, a) => ((a << 24) | (b << 16) | (g << 8) | r) >>> 0;
    const tiles = c.inputs.map((hex) => {
      const px = new Uint32Array(c.cell_w * c.cell_h);
      for (let i = 0; i < px.length; i++) {
        const o = i * 8;
        px[i] = rgbaToPacked(
          parseInt(hex.slice(o, o + 2), 16),
          parseInt(hex.slice(o + 2, o + 4), 16),
          parseInt(hex.slice(o + 4, o + 6), 16),
          parseInt(hex.slice(o + 6, o + 8), 16),
        );
      }
      return { w: c.cell_w, h: c.cell_h, data: px };
    });
    const out = assembleSheet(tiles);
    assert.equal(out.w, c.cell_w * c.slots);
    const lines = [];
    for (let y = 0; y < c.cell_h; y++) {
      let line = "";
      for (let x = 0; x < out.w; x++) {
        // assembleSheet returns Uint8ClampedArray RGBA bytes, already in order.
        const o = (y * out.w + x) * 4;
        line += [out.data[o], out.data[o + 1], out.data[o + 2], out.data[o + 3]].map(chan).join("");
      }
      lines.push(line);
    }
    assert.deepEqual(lines, c.expect, `paste ${c.name}`);
  }
});