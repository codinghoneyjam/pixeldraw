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
import { CASES, LAYER_CASES, PALETTE_SPEC, resolveCasePath } from "./recipe_cases.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// Working assets live under asset_work/ (tidy/ evidence, target/ gate inputs).
const REPO_ROOT = path.resolve(__dirname, "..");
const TARGET_ROOT = path.join(REPO_ROOT, "asset_work", "target");

const MATCH_THRESHOLD = 0.99;
const ELEMENT_DELTA_CHANNEL_THRESHOLD = 128;

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
