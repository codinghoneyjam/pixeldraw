// Bake pixel-faithful tileset recipes from the legacy TRES-extracted PNGs.
//
// U26/U27 have no vector provenance (source .tres deleted after migration), so
// the recipe is reverse-engineered by decomposing the legacy pixels into
// maximal vertical runs of identical row-spans: every emitted `rect` paints a
// region that is one solid color in the legacy file, making the render exact.
// Provenance is recorded in each recipe's `provenance` block.
// Run: node tools/recipe/bake_tileset.mjs   (rewrites both tile recipes)

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { decodePng } from "../../src/io/png.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const TIDY = path.join(REPO_ROOT, "asset_work", "tidy", "numbered");
const TARGET = path.join(REPO_ROOT, "asset_work", "target", "assetdb");

const JOBS = [
  { numbered: "26_room_tileset.png", recipeRel: "world/stage/room_tileset.json", id: "room_tileset", layer: "tiles" },
  { numbered: "27_lobby_tileset.png", recipeRel: "world/lobby/lobby_tileset.json", id: "lobby_tileset", layer: "tiles" },
  // U15: the committed PNG is gold-tinted and its quads differ from the tidy
  // quad SSOT (tint slots), so the recipe is a pixel-faithful rect bake.
  { numbered: "15_keycap_debris.png", recipeRel: "entity/enemy/base_debris.json", id: "keycap_debris", layer: "debris" },
  // U40: no committed profiles JSON (polys unrecoverable -- fillets baked).
  // Pixel-faithful rect+alpha bake; the accent edge (alpha 77) sits on
  // transparency, so overwrite paints it exactly.
  { numbered: "40_title_weapons_atlas.png", recipeRel: "ui/title/title_weapons_atlas.json", id: "title_weapons_atlas", layer: "atlas" },
];

const hex6 = (r, g, b) => "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0").toUpperCase()).join("");

/**
 * Decompose decoded RGBA into solid `rect` commands via vertical run merging.
 * Fully transparent runs are skipped (the document starts transparent).
 * Partially transparent runs become `alpha` rects; the tileset/debris
 * accents sit on transparency, where overwrite paints them exactly.
 */
export function bakeRects(decoded) {
  const { width: w, height: h, rgba } = decoded;
  const at = (x, y) => {
    const i = (y * w + x) * 4;
    return [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]];
  };
  // Row runs: [[x0, x1, r, g, b, a], ...] per row.
  const rows = [];
  for (let y = 0; y < h; y++) {
    const runs = [];
    let sx = 0;
    let [r, g, b, a] = at(0, y);
    for (let x = 1; x <= w; x++) {
      const c = x < w ? at(x, y) : null;
      if (c === null || c[0] !== r || c[1] !== g || c[2] !== b || c[3] !== a) {
        if (a !== 0) runs.push({ x0: sx, x1: x - 1, fill: hex6(r, g, b), alpha: a });
        if (c !== null) { sx = x; [r, g, b, a] = c; }
      }
    }
    rows.push(runs);
  }
  // Vertical merge: extend a rect while the next row has the identical run.
  const cmds = [];
  const open = new Map();
  const keyOf = (run) => `${run.x0}:${run.x1}:${run.fill}:${run.alpha}`;
  for (let y = 0; y <= h; y++) {
    const runs = y < h ? rows[y] : [];
    const seen = new Set();
    for (const run of runs) {
      const k = keyOf(run);
      seen.add(k);
      if (open.has(k)) {
        open.get(k).y1 = y;
      } else {
        open.set(k, { x0: run.x0, x1: run.x1, fill: run.fill, alpha: run.alpha, y0: y, y1: y });
      }
    }
    for (const [k, rect] of [...open]) {
      if (!seen.has(k)) {
        const box = [[rect.x0, rect.y0], [rect.x1, rect.y1]];
        cmds.push(rect.alpha === 255
          ? { cmd: "rect", box, fill: rect.fill }
          : { cmd: "alpha", shape: { cmd: "rect", box, radius: 0 }, color: rect.fill, alpha: rect.alpha });
        open.delete(k);
      }
    }
  }
  // Deterministic order: top-to-bottom, left-to-right.
  const boxOf = (c) => c.box ?? c.shape.box;
  cmds.sort((p, q) => boxOf(p)[0][1] - boxOf(q)[0][1] || boxOf(p)[0][0] - boxOf(q)[0][0]);
  return cmds;
}

async function main() {
  for (const job of JOBS) {
    const pngBytes = fs.readFileSync(path.join(TIDY, job.numbered));
    const decoded = await decodePng(pngBytes);
    const cmds = bakeRects(decoded);
    const recipe = {
      provenance: {
        generated_by: "node tools/recipe/bake_tileset.mjs",
        legacy: `asset_work/tidy/numbered/${job.numbered} (TRES-extracted, source deleted)`,
        method: "reverse-engineered solid-rect decomposition; every rect is one legacy color",
      },
      canvas: { width: decoded.width, height: decoded.height },
      layers: { [job.layer]: cmds },
    };
    const outPath = path.join(TARGET, job.recipeRel);
    fs.mkdirSync(path.dirname(outPath), { recursive: true });
    fs.writeFileSync(outPath, JSON.stringify(recipe, null, 2) + "\n", "utf-8");
    console.log(`baked ${cmds.length} rects -> ${path.relative(REPO_ROOT, outPath)}`);
  }
}

if (process.argv[1] && process.argv[1].endsWith("bake_tileset.mjs")) {
  await main();
}
