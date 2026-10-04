import { textMask } from "../src/core/raster/text_mask.js";
import { loadGlyphSet } from "./recipe/text_glyphs.mjs";
import { decodePng } from "../src/io/png.js";
import { packRGBA, unpackRGBA } from "../src/core/pixel.js";
import fs from "node:fs";

const SETS = loadGlyphSet("./tools/recipe/text_glyphs.json");
const glyphs = SETS["monogram-240"].glyphs;
const W = 318, H = 200;
const m = textMask({ text: "our", glyphs, x: 24, y: -15, fill: packRGBA(255, 255, 255, 255), stroke: packRGBA(0, 0, 0, 255), strokeWidth: 9, w: W, h: H });

const legacy = await decodePng(fs.readFileSync("../assetdb/ui/title/title_logo_text_our.png"));
const a = legacy.rgba;
let diffs = 0;
let first = [];
for (let y = 0; y < H; y++) {
  for (let x = 0; x < W; x++) {
    const i = (y * W + x) * 4;
    const p = m.data[y * W + x];
    const [r, g, b, al] = unpackRGBA(p >>> 0);
    // packRGBA byte order is [r,g,b,a] little-endian u32; unpack to compare
    if (a[i] !== r || a[i + 1] !== g || a[i + 2] !== b || a[i + 3] !== al) {
      diffs++;
      if (first.length < 10) first.push([x, y, [a[i], a[i+1], a[i+2], a[i+3]], [r, g, b, al]]);
    }
  }
}
console.log("textMask-vs-legacy diffs:", diffs, "/", W * H);
console.log(JSON.stringify(first));
