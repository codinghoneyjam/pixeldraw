// SCRATCH diagnostic, one-off probe with no owner and not wired into any gate.
// Kept because it is still the fastest way to see which pixels diverge for a
// given case. If it stops answering a question, delete it rather than repair it.
// Scratch probe: assets are read from asset_work/target/, same root the gates use.
// The bare "../assetdb/..." paths this used to pass the transpiler
// resolved against process.cwd(), so they only worked when it was run from
// the game repo root. They now go through an explicit TARGET_ROOT.
const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const TARGET_ROOT = path.join(REPO_ROOT, "asset_work", "target");
const targetPath = (rel) => path.join(TARGET_ROOT, rel);

import { transpileAndRender } from "../../tools/recipe/transpile.mjs";
import { decodePng } from "../../src/io/png.js";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "diamdiag-"));
const png = path.join(tmp, "o.png");
await transpileAndRender({
  recipePath: targetPath("assetdb/ui/data/diamond_gem.json"),
  layerKey: "body",
  outputDocPath: path.join(tmp, "o.json"),
  outputPngPath: png,
});
const legacy = await decodePng(fs.readFileSync(targetPath("assetdb/ui/hud/hud_icons_128_atlas.png")));
const cand = await decodePng(fs.readFileSync(png));
const hex = (r, g, b) => "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("").toUpperCase();
const pairs = new Map();
for (let y = 0; y < 128; y++) {
  for (let x = 0; x < 128; x++) {
    const i = ((128 + y) * 512 + (384 + x)) * 4, j = (y * 128 + x) * 4;
    const a = legacy.rgba, b = cand.rgba;
    if (a[i] !== b[j] || a[i+1] !== b[j+1] || a[i+2] !== b[j+2] || a[i+3] !== b[j+3]) {
      const k = `L=${hex(a[i],a[i+1],a[i+2])},${a[i+3]} D=${hex(b[j],b[j+1],b[j+2])},${b[j+3]}`;
      if (!pairs.has(k)) pairs.set(k, { n: 0, minX: 999, maxX: -1, minY: 999, maxY: -1 });
      const g = pairs.get(k);
      g.n++;
      if (x < g.minX) g.minX = x; if (x > g.maxX) g.maxX = x;
      if (y < g.minY) g.minY = y; if (y > g.maxY) g.maxY = y;
    }
  }
}
for (const [k, g] of [...pairs.entries()].sort((x, y) => y[1].n - x[1].n).slice(0, 12)) {
  console.log(`${String(g.n).padStart(5)} ${k} x[${g.minX}..${g.maxX}] y[${g.minY}..${g.maxY}]`);
}
