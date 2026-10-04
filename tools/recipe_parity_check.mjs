// <META - FILE SUMMARY - CI gate: render 8 legacy assets via the committed transpiler and assert decoded-RGBA parity>
// Compares decoded RGBA only (never PNG bytes). Gate: match_fraction >= 0.99 per asset.
// Classification mirrors classify.py's amplitude floor: max channel delta >= 128 -> ELEMENT_DELTA.
// Run: node tools/recipe_parity_check.mjs   (or: npm run parity:assets)

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { transpileAndRender } from "./recipe/transpile.mjs";
import { decodePng } from "../src/io/png.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");

const MATCH_THRESHOLD = 0.99;
const ELEMENT_DELTA_CHANNEL_THRESHOLD = 128;

// The 8 gate-able assets. Portal socket/sky_gate are excluded (no committed PNG).
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
];

// <META - ROLE : Compare two decoded RGBA buffers pixel-by-pixel and measure divergence | L46-78>
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
    } else {
      if (pixelMax > maxDelta) maxDelta = pixelMax;
      if (pixelMax >= ELEMENT_DELTA_CHANNEL_THRESHOLD) elementDelta = true;
      if (pixelMax <= 31) histogram["1-31"]++;
      else if (pixelMax <= 63) histogram["32-63"]++;
      else if (pixelMax <= 127) histogram["64-127"]++;
      else histogram["128-255"]++;
    }
  }
  return {
    sizeMismatch: false,
    total,
    matches,
    differing: total - matches,
    matchFraction: matches / total,
    maxDelta,
    classification: elementDelta ? "ELEMENT_DELTA" : "RENDER_NOISE",
    histogram,
  };
}

// <META - ROLE : Render one case via the transpiler and compare against its legacy PNG | L80-100>
async function runCase(caseInfo, tmpDir) {
  const recipePath = path.join(REPO_ROOT, caseInfo.recipe);
  const legacyPath = path.join(REPO_ROOT, caseInfo.legacy);
  const outPng = path.join(tmpDir, `${caseInfo.id}.png`);
  const outDoc = path.join(tmpDir, `${caseInfo.id}.json`);

  const { pngBytes } = await transpileAndRender({
    recipePath,
    layerKey: caseInfo.layer,
    outputDocPath: outDoc,
    outputPngPath: outPng,
  });

  const legacyDecoded = await decodePng(fs.readFileSync(legacyPath));
  const candidateDecoded = await decodePng(pngBytes);
  const result = compareRgba(legacyDecoded, candidateDecoded);
  return { id: caseInfo.id, result };
}

// <META - ROLE : Print one asset row and its failure detail (delta histogram) | L102-118>
function printRow(id, result) {
  const status = result.matchFraction >= MATCH_THRESHOLD ? "PASS" : "FAIL";
  const note = result.sizeMismatch ? " SIZE-MISMATCH" : "";
  console.log(
    `[${status}] ${id.padEnd(26)} match=${result.matchFraction.toFixed(6)}  ` +
      `${result.matches}/${result.total}  class=${result.classification}  max_delta=${result.maxDelta}${note}`,
  );
  if (status === "FAIL" && result.histogram) {
    const h = result.histogram;
    console.log(
      `       delta histogram: 1-31:${h["1-31"]}  32-63:${h["32-63"]}  ` +
        `64-127:${h["64-127"]}  128-255:${h["128-255"]}`,
    );
  }
}

// <META - ROLE : Run all 8 cases, print the per-asset report and summary, set exit code | L120-140>
async function main() {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "recipe-parity-"));
  console.log(`recipe_parity_check: ${CASES.length} assets, threshold ${MATCH_THRESHOLD}`);
  let passed = 0;
  try {
    for (const caseInfo of CASES) {
      const { id, result } = await runCase(caseInfo, tmpDir);
      printRow(id, result);
      if (result.matchFraction >= MATCH_THRESHOLD) passed++;
    }
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
  const total = CASES.length;
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
