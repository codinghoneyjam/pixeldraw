// Text-mask oracle for the Rust port (docs/roadmap/phase2.yaml W12).
//
// The JSON is produced by calling the JS originals, never by re-deriving the
// math, so `crates/engine/tests/text_mask.rs` compares Rust against the
// behaviour that shipped rather than against a second implementation of it.
//
//   node tools/text_mask_cases.mjs            # write tools/text_mask_cases.json
//   node tools/text_mask_cases.mjs --stdout   # print the JSON instead
//
// `synthetic` — hand-made 3-glyph set (shaped/no-shaped, disk/fill/clip/empty)
//   with full glyph defs inline so Rust rebuilds identical inputs.
// `real` — committed tools/recipe/text_glyphs.json cases (params + expected
//   data only; Rust re-decodes the glyph pack itself, which also pins decode).

import { readFileSync, writeFileSync } from "node:fs";
import { packRGBA } from "../src/core/pixel.js";
import { textMask } from "../src/core/raster/text_mask.js";
import { decodeGlyphSets } from "../src/features/text/glyph_set.js";

const u32 = (v) => v >>> 0;

const SYNTH = {
  A: { advance: 4, dx: 0, dy: 0, w: 3, h: 2, alpha: [255, 0, 255, 0, 255, 0], shaped: {} },
  B: { advance: 3, dx: 1, dy: -1, w: 2, h: 2, alpha: [128, 128, 128, 128], shaped: {} },
  C: {
    advance: 5, dx: 0, dy: 0, w: 1, h: 1, alpha: [255],
    shaped: { 2: { dx: -1, dy: -1, w: 3, h: 3, alpha: [200, 200, 200, 200, 200, 200, 200, 200, 200] } },
  },
};

function toGlyph(def) {
  return {
    advance: def.advance,
    dx: def.dx,
    dy: def.dy,
    w: def.w,
    h: def.h,
    alpha: Uint8Array.from(def.alpha),
    shaped: Object.fromEntries(
      Object.entries(def.shaped ?? {}).map(([k, s]) => [k, {
        dx: s.dx, dy: s.dy, w: s.w, h: s.h, alpha: Uint8Array.from(s.alpha),
      }]),
    ),
  };
}

const glyphsOf = (defs) => Object.fromEntries(Object.entries(defs).map(([k, d]) => [k, toGlyph(d)]));

const RED = u32(packRGBA(255, 0, 0, 255));
const BLUE = u32(packRGBA(0, 0, 255, 255));
const WHITE = u32(packRGBA(255, 255, 255, 255));
const BLACK = u32(packRGBA(0, 0, 0, 255));

const SYNTH_CASES = [
  { name: "disk-ab", text: "AB", x: 1, y: 1, fill: RED, stroke: BLUE, strokeWidth: 1, strokeMode: "shape", w: 12, h: 8 },
  { name: "shaped-c", text: "C", x: 2, y: 1, fill: RED, stroke: BLUE, strokeWidth: 2, strokeMode: "shape", w: 10, h: 6 },
  { name: "disk-c", text: "C", x: 2, y: 1, fill: RED, stroke: BLUE, strokeWidth: 2, strokeMode: "disk", w: 10, h: 6 },
  { name: "default-mode-c", text: "C", x: 2, y: 1, fill: RED, stroke: BLUE, strokeWidth: 2, w: 10, h: 6 },
  { name: "fill-only-zero-stroke", text: "AB", x: 1, y: 1, fill: RED, stroke: 0, strokeWidth: 5, strokeMode: "shape", w: 12, h: 8 },
  { name: "fill-only-zero-width", text: "AB", x: 1, y: 1, fill: RED, stroke: BLUE, strokeWidth: 0, strokeMode: "shape", w: 12, h: 8 },
  { name: "clip-negative", text: "AB", x: -2, y: -1, fill: WHITE, stroke: BLUE, strokeWidth: 1, strokeMode: "shape", w: 6, h: 4 },
  { name: "empty-text", text: "", x: 0, y: 0, fill: RED, stroke: BLUE, strokeWidth: 2, strokeMode: "shape", w: 6, h: 4 },
];

const synthetic = SYNTH_CASES.map((c) => {
  const mask = textMask({ ...c, glyphs: glyphsOf(SYNTH) });
  return {
    name: c.name, glyphs: SYNTH, text: c.text, x: c.x, y: c.y,
    fill: c.fill, stroke: c.stroke, strokeWidth: c.strokeWidth,
    shapeStroke: (c.strokeMode ?? "shape") === "shape", w: c.w, h: c.h,
    data: Array.from(mask.data),
  };
});

const raw = JSON.parse(readFileSync(new URL("./recipe/text_glyphs.json", import.meta.url)));
const fonts = decodeGlyphSets(raw);

const REAL_CASES = [
  { name: "default42-fill", font: "default-42", text: "A", x: 2, y: 3, fill: WHITE, stroke: BLACK, strokeWidth: 0, strokeMode: "shape", w: 40, h: 48 },
  { name: "default42-shaped3", font: "default-42", text: "A", x: 3, y: 4, fill: WHITE, stroke: RED, strokeWidth: 3, strokeMode: "shape", w: 44, h: 52 },
  { name: "default42-disk2", font: "default-42", text: "A", x: 3, y: 4, fill: WHITE, stroke: RED, strokeWidth: 2, strokeMode: "shape", w: 44, h: 52 },
  { name: "monogram-o", font: "monogram-240", text: "o", x: 4, y: 6, fill: WHITE, stroke: BLUE, strokeWidth: 0, strokeMode: "shape", w: 90, h: 120 },
  { name: "monogram-word-clip", font: "monogram-240", text: "word", x: -40, y: 8, fill: WHITE, stroke: 0, strokeWidth: 4, strokeMode: "shape", w: 200, h: 130 },
];

const real = REAL_CASES.map((c) => {
  const mask = textMask({ ...c, glyphs: fonts[c.font].glyphs });
  return {
    name: c.name, font: c.font, text: c.text, x: c.x, y: c.y,
    fill: c.fill, stroke: c.stroke, strokeWidth: c.strokeWidth,
    shapeStroke: (c.strokeMode ?? "shape") === "shape", w: c.w, h: c.h,
    data: Array.from(mask.data),
  };
});

const payload = {
  synthetic,
  real,
  meta: { synthetic: synthetic.length, real: real.length, fonts: Object.keys(fonts) },
};
const text = JSON.stringify(payload);
if (process.argv.includes("--stdout")) {
  process.stdout.write(text);
} else {
  const out = new URL("./text_mask_cases.json", import.meta.url);
  writeFileSync(out, text + "\n");
  console.log(
    `text_mask_cases.json: ${synthetic.length} synthetic + ${real.length} real cases ` +
      `(fonts ${Object.keys(fonts).join(", ")})`,
  );
}
