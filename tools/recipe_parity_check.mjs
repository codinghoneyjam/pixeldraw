// <META - FILE SUMMARY - CI gate: render 20 legacy assets via the committed transpiler, assert RGBA parity>
// Compares decoded RGBA only (never PNG bytes). Gate: match_fraction >= 0.99 per asset.
// Pixels transparent on BOTH sides count as equal (RGB is undefined there); the
// excluded count is printed as transparent_residue and strict_match alongside.
// Classification mirrors classify.py's amplitude floor: max channel delta >= 128 -> ELEMENT_DELTA.
// Run: node tools/recipe_parity_check.mjs   (or: npm run parity:assets)

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { transpileAndRender } from "./recipe/transpile.mjs";
import { decodePng } from "../src/io/png.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// Working assets live under asset_work/ (tidy/ evidence, target/ gate inputs).
const REPO_ROOT = path.resolve(__dirname, "..");
const TARGET_ROOT = path.join(REPO_ROOT, "asset_work", "target");
const resolveCasePath = (rel) =>
  path.join(TARGET_ROOT, rel.startsWith("draw_tool_v2/") ? rel.slice("draw_tool_v2/".length) : rel);

const MATCH_THRESHOLD = 0.99;
const ELEMENT_DELTA_CHANNEL_THRESHOLD = 128;

// The 13 single-invocation assets. Portal socket/sky_gate are excluded (no PNG).
// Layer keys match the scratch all_assets_verify.py baseline exactly (no --slot).
const CASES = [
  { id: "sword_albedo", recipe: "assetdb/entity/weapon/weapon_sword.json", layer: "albedo", legacy: "assetdb/entity/weapon/sword_albedo.png" },
  { id: "sword_mask", recipe: "assetdb/entity/weapon/weapon_sword.json", layer: "mask", legacy: "assetdb/entity/weapon/sword_mask.png" },
  { id: "bow_albedo", recipe: "assetdb/entity/weapon/weapon_bow.json", layer: "albedo", legacy: "assetdb/entity/weapon/bow_albedo.png" },
  { id: "bow_mask", recipe: "assetdb/entity/weapon/weapon_bow.json", layer: "mask", legacy: "assetdb/entity/weapon/bow_mask.png" },
  { id: "spear_albedo", recipe: "assetdb/entity/weapon/weapon_spear.json", layer: "albedo", legacy: "assetdb/entity/weapon/spear_albedo.png" },
  { id: "spear_mask", recipe: "assetdb/entity/weapon/weapon_spear.json", layer: "mask", legacy: "assetdb/entity/weapon/spear_mask.png" },
  { id: "portal_keycap_unpressed", recipe: "assetdb/world/object/portal_keycap_master.json", layer: "keycap_unpressed", legacy: "assetdb/world/img/portal/portal_keycap_unpressed.png" },
  { id: "portal_keycap_pressed", recipe: "assetdb/world/object/portal_keycap_master.json", layer: "keycap_pressed", legacy: "assetdb/world/img/portal/portal_keycap_pressed.png" },
  // Non 32-multiple asset (280x560). The recipe declares export.canvas 288x576
  // plus a viewport so the stored document stays chunk-aligned; the transpiler
  // emits the cropped 280x560 PNG.
  { id: "title_diorama_sword_body", recipe: "assetdb/ui/data/title_diorama_sword_body.json", layer: "body", legacy: "assetdb/ui/title/title_diorama_sword_body.png" },
  // Title logo type. Recipes live under draw_tool_v2/tests/fixtures/ because the
  // committed PNGs were baked from these exact specs, not from assetdb recipes.
  { id: "title_logo_text_our", recipe: "draw_tool_v2/tests/fixtures/title_logo_text_our_recipe.json", layer: null, legacy: "assetdb/ui/title/title_logo_text_our.png" },
  { id: "title_logo_text_or", recipe: "draw_tool_v2/tests/fixtures/title_logo_text_or_recipe.json", layer: null, legacy: "assetdb/ui/title/title_logo_text_or.png" },
  { id: "title_logo_text_d", recipe: "draw_tool_v2/tests/fixtures/title_logo_text_d_recipe.json", layer: null, legacy: "assetdb/ui/title/title_logo_text_d.png" },
  { id: "shockwave_ring", recipe: "draw_tool_v2/tests/fixtures/shockwave_ring_recipe.json", layer: null, legacy: "assetdb/entity/player/shockwave_ring.png" },
  // 16-slot enemy atlas (2048x128). The recipes flatten the legacy shape spec
  // into per-slot command lists; residual is the sheen stamp model and the
  // visor_50/25 cutout path. See docs/40_work/task/active/DRAWTOOL_REPRO.md.
  { id: "enemy_keybot_c001_sheet", recipe: "assetdb/entity/enemy/enemy_C-001_sheet.json", layer: null, legacy: "assetdb/entity/enemy/enemy_keybot_c001_sheet.png" },
  { id: "enemy_keybot_t002_sheet", recipe: "assetdb/entity/enemy/enemy_T-002_sheet.json", layer: null, legacy: "assetdb/entity/enemy/enemy_keybot_t002_sheet.png" },
  { id: "enemy_keybot_r003_sheet", recipe: "assetdb/entity/enemy/enemy_R-003_sheet.json", layer: null, legacy: "assetdb/entity/enemy/enemy_keybot_r003_sheet.png" },
  // B-001 (boss) is byte-identical to C-001 in the committed PNGs, so its recipe
  // is C-001's palette. The boss signature #FF3B30 is intentionally NOT baked
  // yet -- re-baking the boss is a separate, deliberate change.
  { id: "enemy_keybot_b001_sheet", recipe: "assetdb/entity/enemy/enemy_B-001_sheet.json", layer: null, legacy: "assetdb/entity/enemy/enemy_keybot_b001_sheet.png" },
];

// Player / chassis / emote layers promoted from tests/fixtures/player_arc/.
// Those legacy PNGs are per-layer 128x128 crops baked with the manifest's
// palette spec, so each case carries its own palette override.
const PALETTE_SPEC = JSON.parse(
  fs.readFileSync(path.join(TARGET_ROOT, "tests/fixtures/player_arc/manifest.json"), "utf-8"),
).spec;
const LAYER_CASES = [
  { id: "player_mouse__shadow", recipe: "assetdb/entity/player/player_mouse.json", layer: "shadow" },
  { id: "player_mouse__visor_left", recipe: "assetdb/entity/player/player_mouse.json", layer: "visor_left" },
  { id: "player_mouse__visor_right", recipe: "assetdb/entity/player/player_mouse.json", layer: "visor_right" },
  { id: "base_chassis_gem__shadow", recipe: "assetdb/entity/player/base_chassis_gem.json", layer: "shadow" },
  { id: "base_chassis_trackball__shadow", recipe: "assetdb/entity/player/base_chassis_trackball.json", layer: "shadow" },
  { id: "base_chassis_trackball__body", recipe: "assetdb/entity/player/base_chassis_trackball.json", layer: "body" },
  { id: "emoticons_cyber_neon__face_normal", recipe: "assetdb/entity/player/emoticons_cyber_neon.json", layer: "face_normal" },
].map((c) => ({ ...c, legacy: `tests/fixtures/player_arc/${c.id}_legacy.png`, palette: PALETTE_SPEC }));

// <META - ROLE : Compare two decoded RGBA buffers pixel-by-pixel and measure divergence | L62-123>
function compareRgba(legacy, candidate) {
  if (legacy.width !== candidate.width || legacy.height !== candidate.height) {
    return {
      sizeMismatch: true,
      total: 0,
      matches: 0,
      differing: 0,
      matchFraction: 0,
      maxDelta: 0,
      classification: "ELEMENT_DELTA",
      histogram: null,
    };
  }
  const total = legacy.width * legacy.height;
  const a = legacy.rgba;
  const b = candidate.rgba;
  let matches = 0;
  let transparentResidue = 0;
  let maxDelta = 0;
  let elementDelta = false;
  const histogram = { "1-31": 0, "32-63": 0, "64-127": 0, "128-255": 0 };
  for (let i = 0; i < total * 4; i += 4) {
    const dR = Math.abs(a[i] - b[i]);
    const dG = Math.abs(a[i + 1] - b[i + 1]);
    const dB = Math.abs(a[i + 2] - b[i + 2]);
    const dA = Math.abs(a[i + 3] - b[i + 3]);
    const pixelMax = Math.max(dR, dG, dB, dA);
    if (pixelMax === 0) {
      matches++;
      continue;
    }
    // A pixel that is fully transparent on BOTH sides carries no colour: PNG
    // leaves RGB undefined there and every consumer ignores it. PIL legacy
    // bakers leak the paint colour into that region (shockwave_ring: 2384 px of
    // #C8C8C8 at alpha 0) because they build RGBA arrays channel-wise, while
    // packRGBA() drops RGB when alpha is 0. Counted separately so the strict
    // number stays visible and the gate judges appearance, not array residue.
    if (dA === 0 && a[i + 3] === 0) {
      transparentResidue++;
      matches++;
      continue;
    }
    if (pixelMax > maxDelta) maxDelta = pixelMax;
    if (pixelMax >= ELEMENT_DELTA_CHANNEL_THRESHOLD) elementDelta = true;
    if (pixelMax <= 31) histogram["1-31"]++;
    else if (pixelMax <= 63) histogram["32-63"]++;
    else if (pixelMax <= 127) histogram["64-127"]++;
    else histogram["128-255"]++;
  }
  return {
    sizeMismatch: false,
    total,
    matches,
    transparentResidue,
    differing: total - matches,
    matchFraction: matches / total,
    strictMatchFraction: (matches - transparentResidue) / total,
    maxDelta,
    classification: elementDelta ? "ELEMENT_DELTA" : "RENDER_NOISE",
    histogram,
  };
}

// <META - ROLE : Render one case via the transpiler and compare against its legacy PNG | L126-144>
async function runCase(caseInfo, tmpDir) {
  const recipePath = resolveCasePath(caseInfo.recipe);
  const legacyPath = resolveCasePath(caseInfo.legacy);
  const outPng = path.join(tmpDir, `${caseInfo.id}.png`);
  const outDoc = path.join(tmpDir, `${caseInfo.id}.json`);

  const { pngBytes } = await transpileAndRender({
    recipePath,
    layerKey: caseInfo.layer,
    paletteOverrides: caseInfo.palette ?? {},
    outputDocPath: outDoc,
    outputPngPath: outPng,
  });

  const legacyDecoded = await decodePng(fs.readFileSync(legacyPath));
  const candidateDecoded = await decodePng(pngBytes);
  const result = compareRgba(legacyDecoded, candidateDecoded);
  return { id: caseInfo.id, result };
}

// <META - ROLE : Print one asset row and its failure detail (delta histogram) | L147-165>
function printRow(id, result) {
  const status = result.matchFraction >= MATCH_THRESHOLD ? "PASS" : "FAIL";
  const note = result.sizeMismatch ? " SIZE-MISMATCH" : "";
  const res = result.transparentResidue
    ? `  transparent_residue=${result.transparentResidue}` +
      `  strict_match=${result.strictMatchFraction.toFixed(6)}`
    : "";
  console.log(
    `[${status}] ${id.padEnd(26)} match=${result.matchFraction.toFixed(6)}  ` +
      `${result.matches}/${result.total}  class=${result.classification}  max_delta=${result.maxDelta}${note}${res}`,
  );
  if (status === "FAIL" && result.histogram) {
    const h = result.histogram;
    console.log(
      `       delta histogram: 1-31:${h["1-31"]}  32-63:${h["32-63"]}  ` +
        `64-127:${h["64-127"]}  128-255:${h["128-255"]}`,
    );
  }
}

// <META - ROLE : Run every case, print the per-asset report and summary, set exit code | L168-191>
async function main() {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "recipe-parity-"));
  const all = [...CASES, ...LAYER_CASES];
  console.log(`recipe_parity_check: ${all.length} assets, threshold ${MATCH_THRESHOLD}`);
  let passed = 0;
  try {
    for (const caseInfo of all) {
      const { id, result } = await runCase(caseInfo, tmpDir);
      printRow(id, result);
      if (result.matchFraction >= MATCH_THRESHOLD) passed++;
    }
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
  const total = all.length;
  console.log("--");
  console.log(`RESULT: ${passed}/${total} passed (threshold ${MATCH_THRESHOLD})`);
  if (passed === total) {
    console.log("parity gate: OK");
  } else {
    console.log("parity gate: FAILED");
  }
  process.exit(passed === total ? 0 : 1);
}

await main();
