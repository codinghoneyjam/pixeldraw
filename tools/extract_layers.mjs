// <META - FILE SUMMARY - Extract per-layer PNG + Document JSON from the 34-asset recipe set
//
// Each recipe is decomposed into its named units (layers, atlas slots,
// surfaces, nine-patch variants, albedo/mask layers) and every unit is
// rendered through the same transpileAndRender() path the parity gate uses.
// Output: asset_work/layers/<recipeSlug>/<unit>.png + <unit>_drawtool.json
// plus asset_work/layers/index.json mapping recipe -> assets.
//
// Run: node tools/extract_layers.mjs [--out DIR] [--only recipeSlug]

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { transpileAndRender } from "./recipe/transpile.mjs";
import { CASES, LAYER_CASES, resolveCasePath } from "./recipe_cases.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..");
const DEFAULT_OUT = path.join(REPO_ROOT, "asset_work", "layers");

function isPantographProfile(recipe) {
  return Boolean(recipe && recipe.geometry && recipe.colors && Array.isArray(recipe.slots));
}
function isSurfaceManifest(recipe) {
  return Boolean(recipe && Array.isArray(recipe.surfaces));
}

// <META - ROLE : Structural layer units of one recipe, in transpile order | L40-90>
export function enumerateUnits(recipe) {
  if (isSurfaceManifest(recipe)) {
    return recipe.surfaces.map((s) => ({ unit: s.id, layerKey: null, slotId: s.id }));
  }
  if (isPantographProfile(recipe)) {
    const units = [];
    for (const key of Object.keys(recipe.nine_patch ?? {})) {
      units.push({ unit: key, layerKey: key, slotId: key });
    }
    units.push({ unit: "sheet", layerKey: null, slotId: null });
    return units;
  }
  const slots = recipe.export?.atlas?.slots ?? recipe.atlas?.slots;
  if (slots) {
    return slots.map((s) => ({ unit: s.id, layerKey: null, slotId: s.id }));
  }
  if (Array.isArray(recipe.layers)) {
    return recipe.layers.map((l) => ({ unit: l.id, layerKey: l.id, slotId: null }));
  }
  if (recipe.layers && typeof recipe.layers === "object") {
    return Object.keys(recipe.layers).map((k) => ({ unit: k, layerKey: k, slotId: null }));
  }
  if (recipe.albedo_layers || recipe.mask_layers) {
    const units = [];
    if (recipe.albedo_layers) units.push({ unit: "albedo", layerKey: "albedo", slotId: null });
    if (recipe.mask_layers) units.push({ unit: "mask", layerKey: "mask", slotId: null });
    return units;
  }
  // Player-shape schema: layers are top-level command arrays (shadow, body, ...).
  const units = [];
  for (const [key, value] of Object.entries(recipe)) {
    if (Array.isArray(value) && value.length > 0 && value.every((v) => v && typeof v === "object" && (v.cmd || v.type))) {
      units.push({ unit: key, layerKey: key, slotId: null });
    }
  }
  if (units.length > 0) return units;
  return [{ unit: "full", layerKey: null, slotId: null }];
}

function slugFor(recipeRel) {
  return path.basename(recipeRel).replace(/\.json$/i, "");
}

async function main() {
  const args = process.argv.slice(2);
  let outDir = DEFAULT_OUT;
  let only = null;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--out") outDir = path.resolve(args[++i]);
    if (args[i] === "--only") only = args[++i];
  }

  // recipe -> { cases, palette } (LAYER_CASES share recipes and carry a palette)
  const recipes = new Map();
  for (const c of [...CASES, ...LAYER_CASES]) {
    const key = `${c.recipe}|${c.palette ? "pal" : "default"}`;
    if (!recipes.has(key)) recipes.set(key, { recipe: c.recipe, palette: c.palette ?? {}, assets: [] });
    recipes.get(key).assets.push(c.id);
  }

  const index = { schema: "draw_tool.layer_index", version: 1, outDir, recipes: [] };
  let rendered = 0;
  for (const { recipe, palette, assets } of recipes.values()) {
    const slug = slugFor(recipe) + (Object.keys(palette).length ? "__pal" : "");
    if (only && slug !== only) continue;
    const recipePath = resolveCasePath(recipe);
    const parsed = JSON.parse(fs.readFileSync(recipePath, "utf-8"));
    const units = enumerateUnits(parsed);
    const dir = path.join(outDir, slug);
    fs.mkdirSync(dir, { recursive: true });
    const unitRows = [];
    for (const u of units) {
      const pngPath = path.join(dir, `${u.unit}.png`);
      const docPath = path.join(dir, `${u.unit}_drawtool.json`);
      try {
        await transpileAndRender({
          recipePath,
          layerKey: u.layerKey,
          slotId: u.slotId,
          paletteOverrides: palette,
          outputDocPath: docPath,
          outputPngPath: pngPath,
        });
        unitRows.push({ unit: u.unit, png: path.relative(outDir, pngPath), doc: path.relative(outDir, docPath) });
        rendered++;
      } catch (err) {
        unitRows.push({ unit: u.unit, error: String(err.message ?? err) });
      }
    }
    index.recipes.push({ slug, recipe, assets, units: unitRows });
    console.log(`[extract] ${slug}: ${unitRows.length} units`);
  }

  fs.mkdirSync(outDir, { recursive: true });
  fs.writeFileSync(path.join(outDir, "index.json"), JSON.stringify(index, null, 2), "utf-8");
  console.log(`[extract] ${rendered} unit renders -> ${outDir}`);
}

if (process.argv[1] && process.argv[1].endsWith("extract_layers.mjs")) {
  await main();
}
