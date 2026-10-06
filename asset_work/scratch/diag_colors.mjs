// SCRATCH diagnostic, one-off probe with no owner and not wired into any gate.
// Kept because it is still the fastest way to see which pixels diverge for a
// given case. If it stops answering a question, delete it rather than repair it.
// Diagnostic: check color resolution + pixel diffs for the 3 failing assets
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { resolveColor, resolveColorToken } from "../../tools/recipe/color_tokens.mjs";
import { transpileAndRender } from "../../tools/recipe/transpile.mjs";
import { decodePng } from "../../src/io/png.js";
import { unpackRGBA } from "../../src/core/pixel.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// Scratch probe: assets are read from asset_work/target/, same root the gates use.
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const TARGET_ROOT = path.join(REPO_ROOT, "asset_work", "target");
const targetPath = (rel) => path.join(TARGET_ROOT, rel);

// --- Part 1: weapon_bow palette resolution ---
const bowPalette = {
  "$slate_dark": "#1E293B",
  "$outline": "#0F172A",
  "$metal_mid": "#94A3B8",
  "$metal_light": "#E2E8F0",
  "$neon_cyan": "#22D3EE",
};
console.log("=== weapon_bow palette resolution ===");
for (const tok of ["$metal_light", "$outline", "$metal_mid", "$slate_dark", "$neon_cyan"]) {
  const raw = resolveColorToken(tok, bowPalette);
  const packed = resolveColor(tok, bowPalette);
  const rgba = unpackRGBA(packed >>> 0);
  console.log(`  ${tok} -> raw=${JSON.stringify(raw)} packed=${packed} rgba=${JSON.stringify(rgba)}`);
}

// --- Part 2: portal dynamic_colors resolution ---
const portalDynamic = {
  "socket_plate": "#0E141C",
  "socket_rim": "#253448",
  "keycap_skirt_top": "#1D382B",
  "keycap_skirt_side": "#13261D",
  "keycap_skirt_shadow": "#0A1610",
  "keycap_face": "#152E22",
  "accent_neon": "#00FF88",
  "accent_glow": "#39FF14",
  "sky_gate_primary": "#00FF88",
  "sky_gate_secondary": "#39FF14",
  "core_black": "#050A07",
};
console.log("=== portal dynamic_colors resolution (unprefixed keys, $-refs) ===");
for (const tok of ["$keycap_skirt_shadow", "$keycap_face", "$accent_neon", "$accent_glow", "$core_black", "$socket_plate"]) {
  const raw = resolveColorToken(tok, portalDynamic);
  const packed = resolveColor(tok, portalDynamic);
  const rgba = unpackRGBA(packed >>> 0);
  console.log(`  ${tok} -> raw=${JSON.stringify(raw)} packed=${packed} rgba=${JSON.stringify(rgba)}`);
}

// --- Part 3: pixel diffs ---
const CASES = [
  { id: "bow_albedo", recipe: "assetdb/entity/weapon/weapon_bow.json", layer: "albedo", legacy: "assetdb/entity/weapon/bow_albedo.png" },
  { id: "portal_keycap_unpressed", recipe: "assetdb/world/object/portal_keycap_master.json", layer: "keycap_unpressed", legacy: "assetdb/world/img/portal/portal_keycap_unpressed.png" },
  { id: "portal_keycap_pressed", recipe: "assetdb/world/object/portal_keycap_master.json", layer: "keycap_pressed", legacy: "assetdb/world/img/portal/portal_keycap_pressed.png" },
];

const hex = (r, g, b) => "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("").toUpperCase();

for (const c of CASES) {
  const outPng = path.join(__dirname, `diag_${c.id}.png`);
  const outDoc = path.join(__dirname, `diag_${c.id}.json`);
  await transpileAndRender({
    recipePath: targetPath(c.recipe),
    layerKey: c.layer,
    outputDocPath: outDoc,
    outputPngPath: outPng,
  });
  const legacy = await decodePng(fs.readFileSync(targetPath(c.legacy)));
  const cand = await decodePng(fs.readFileSync(outPng));
  const a = legacy.rgba, b = cand.rgba;
  const total = legacy.width * legacy.height;
  // histogram of (legacyRGBA -> candRGBA) pairs, top 12
  const pairs = new Map();
  let diffs = 0;
  for (let i = 0; i < total * 4; i += 4) {
    if (a[i] !== b[i] || a[i+1] !== b[i+1] || a[i+2] !== b[i+2] || a[i+3] !== b[i+3]) {
      diffs++;
      const key = `L=${hex(a[i],a[i+1],a[i+2])},${a[i+3]} D=${hex(b[i],b[i+1],b[i+2])},${b[i+3]}`;
      pairs.set(key, (pairs.get(key) ?? 0) + 1);
    }
  }
  console.log(`=== ${c.id}: ${diffs} diffs / ${total} ===`);
  const sorted = [...pairs.entries()].sort((x, y) => y[1] - x[1]).slice(0, 12);
  for (const [k, v] of sorted) console.log(`  ${v.toString().padStart(5)}px  ${k}`);
  // sample specific points
  const pts = c.id === "bow_albedo" ? [[50,31],[46,34]] : [[64,64],[40,40],[64,20]];
  for (const [x, y] of pts) {
    const i = (y * legacy.width + x) * 4;
    console.log(`  sample (${x},${y}): L=${hex(a[i],a[i+1],a[i+2])},${a[i+3]} D=${hex(b[i],b[i+1],b[i+2])},${b[i+3]}`);
  }
}
