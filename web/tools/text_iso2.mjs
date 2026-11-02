import { transpileAndRender } from "./recipe/transpile.mjs";
import { decodePng } from "../src/io/png.js";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tiso-"));
const png = path.join(tmp, "o.png");
await transpileAndRender({
  recipePath: "tests/fixtures/title_logo_text_our_recipe.json",
  layerKey: null,
  outputDocPath: path.join(tmp, "o.json"),
  outputPngPath: png,
});
const legacy = await decodePng(fs.readFileSync("../assetdb/ui/title/title_logo_text_our.png"));
const cand = await decodePng(fs.readFileSync(png));
console.log("sizes:", legacy.width, legacy.height, cand.width, cand.height);
const hex = (r, g, b) => "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("").toUpperCase();
const pairs = new Map();
for (let y = 0; y < 200; y++) {
  for (let x = 0; x < 318; x++) {
    const i = (y * 318 + x) * 4, j = (y * 320 + x) * 4;
    const a = legacy.rgba, b = cand.rgba;
    if (a[i] !== b[j] || a[i+1] !== b[j+1] || a[i+2] !== b[j+2] || a[i+3] !== b[j+3]) {
      const k = `L=${hex(a[i],a[i+1],a[i+2])},${a[i+3]} D=${hex(b[j],b[j+1],b[j+2])},${b[j+3]}`;
      pairs.set(k, (pairs.get(k) ?? 0) + 1);
    }
  }
}
let n = 0;
for (const [k, v] of [...pairs.entries()].sort((x, y) => y[1] - x[1]).slice(0, 8)) { console.log(`${v} ${k}`); n += v; }
console.log("total diffs:", n);
