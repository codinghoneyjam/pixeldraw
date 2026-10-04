// Diagnostic 2: count colors in legacy vs drawtool; dump diff coords
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { transpileAndRender } from "./recipe/transpile.mjs";
import { decodePng } from "../src/io/png.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");

const CASES = [
  { id: "bow_albedo", recipe: "assetdb/entity/weapon/weapon_bow.json", layer: "albedo", legacy: "assetdb/entity/weapon/bow_albedo.png" },
  { id: "portal_keycap_unpressed", recipe: "assetdb/world/data/portal/portal_keycap_master.json", layer: "keycap_unpressed", legacy: "assetdb/world/img/portal/portal_keycap_unpressed.png" },
  { id: "portal_keycap_pressed", recipe: "assetdb/world/data/portal/portal_keycap_master.json", layer: "keycap_pressed", legacy: "assetdb/world/img/portal/portal_keycap_pressed.png" },
];

const hex = (r, g, b) => "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("").toUpperCase();

for (const c of CASES) {
  const outPng = path.join(__dirname, `diag_${c.id}.png`);
  const outDoc = path.join(__dirname, `diag_${c.id}.json`);
  await transpileAndRender({
    recipePath: path.join(REPO_ROOT, c.recipe),
    layerKey: c.layer,
    outputDocPath: outDoc,
    outputPngPath: outPng,
  });
  const legacy = await decodePng(fs.readFileSync(path.join(REPO_ROOT, c.legacy)));
  const cand = await decodePng(fs.readFileSync(outPng));
  const a = legacy.rgba, b = cand.rgba;
  const total = legacy.width * legacy.height;

  const countColors = (img) => {
    const m = new Map();
    for (let i = 0; i < total * 4; i += 4) {
      const key = `${hex(img[i],img[i+1],img[i+2])},${img[i+3]}`;
      m.set(key, (m.get(key) ?? 0) + 1);
    }
    return m;
  };
  const lc = countColors(a), cc = countColors(b);
  console.log(`=== ${c.id} ===`);
  console.log("  legacy colors:", [...lc.entries()].sort((x,y)=>y[1]-x[1]).map(([k,v])=>`${k}:${v}`).join("  "));
  console.log("  drawtool colors:", [...cc.entries()].sort((x,y)=>y[1]-x[1]).map(([k,v])=>`${k}:${v}`).join("  "));

  // diff coords grouped by (L,D) pair, print bounding boxes
  const groups = new Map();
  for (let y = 0; y < legacy.height; y++) {
    for (let x = 0; x < legacy.width; x++) {
      const i = (y * legacy.width + x) * 4;
      if (a[i]!==b[i]||a[i+1]!==b[i+1]||a[i+2]!==b[i+2]||a[i+3]!==b[i+3]) {
        const key = `L=${hex(a[i],a[i+1],a[i+2])},${a[i+3]} D=${hex(b[i],b[i+1],b[i+2])},${b[i+3]}`;
        if (!groups.has(key)) groups.set(key, { n: 0, minX: 999, maxX: -1, minY: 999, maxY: -1 });
        const g = groups.get(key);
        g.n++;
        if (x<g.minX)g.minX=x; if (x>g.maxX)g.maxX=x; if (y<g.minY)g.minY=y; if (y>g.maxY)g.maxY=y;
      }
    }
  }
  for (const [k, g] of [...groups.entries()].sort((x,y)=>y[1].n-x[1].n)) {
    console.log(`  ${String(g.n).padStart(5)}px  ${k}  bbox x[${g.minX}..${g.maxX}] y[${g.minY}..${g.maxY}]`);
  }
}
