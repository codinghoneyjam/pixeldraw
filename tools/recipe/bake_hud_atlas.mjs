// Bake the combined HUD icon atlas recipe from the 11 per-icon shape JSONs.
//
// Source of truth: asset_work/target/assetdb/ui/hud/icons/*.json (byte copies
// of asset_work/tidy/target/U35_recipe_<name>.json) in ICON_NAMES order on a
// 4x4 grid of 128px cells (gen_hud_icons_128.py layout). The baker resolves
// each icon's $tokens against its own dynamic_colors to literals, then offsets
// every geometric coordinate by the cell origin, so the combined file is a
// plain transpiler-native recipe (no palette, no tokens) rendering the full
// 512x512 atlas in ONE transpileAndRender call.
//
// Token resolution reuses color_tokens.js (the same code the transpiler
// paints with), so the baker cannot drift from the render path.
// Run: node tools/recipe/bake_hud_atlas.mjs   (rewrites the atlas JSON)

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { bakeValue, offsetCmd } from "./bake_lib.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const HUD_DIR = path.join(REPO_ROOT, "asset_work", "target", "assetdb", "ui", "hud");
const ICONS_DIR = path.join(HUD_DIR, "icons");
const OUT_PATH = path.join(HUD_DIR, "hud_icons_128_atlas.json");

const CELL = 128;
const COLUMNS = 4;
const ICON_NAMES = [
  "heart", "shield", "lightning", "speaker",
  "book", "gear", "stopwatch", "diamond_gem",
  "cross_mount", "kill_skull", "chest",
];

function main() {
  const all = [];
  for (let idx = 0; idx < ICON_NAMES.length; idx++) {
    const name = ICON_NAMES[idx];
    const recipe = JSON.parse(fs.readFileSync(path.join(ICONS_DIR, `${name}.json`), "utf-8"));
    const palette = { ...(recipe.dynamic_colors ?? {}) };
    const ox = (idx % COLUMNS) * CELL, oy = Math.floor(idx / COLUMNS) * CELL;
    for (const cmd of recipe.body ?? []) {
      all.push(offsetCmd(bakeValue(cmd, palette), ox, oy));
    }
  }
  const combined = {
    provenance: {
      generated_by: "node tools/recipe/bake_hud_atlas.mjs",
      sources: ICON_NAMES.map((n) => `assetdb/ui/hud/icons/${n}.json`),
      layout: "4x4 grid, 128px cells, row-major in ICON_NAMES order; cells 11-15 empty",
      legacy: "assetdb/ui/hud/hud_icons_128_atlas.png (U35)",
    },
    canvas: { width: 512, height: 512 },
    layers: { atlas: all },
  };
  const text = JSON.stringify(combined, null, 2) + "\n";
  if (text.includes("$")) throw new Error("unresolved $token in baked atlas");
  fs.writeFileSync(OUT_PATH, text, "utf-8");
  console.log(`baked ${all.length} commands -> ${path.relative(REPO_ROOT, OUT_PATH)}`);
}

main();
