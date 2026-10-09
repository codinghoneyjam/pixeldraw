// Bake full 16-slot player sheets (U16-U18) into one transpiler-native recipe
// each: vector slots from the shape recipes (cell-offset, $tokens baked with
// the profile palette) + wheel frames (slots 10-12) as pixel rect/alpha runs,
// because GaussianBlur has no transpiler equivalent. Slots 13-15 stay empty.
// Layout mirrors gen_player_assets.build_player_atlas slot order.
// Run: node tools/recipe/bake_player_atlas.mjs   (rewrites both sheet recipes)

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { decodePng } from "../../src/io/png.js";
import { bakeRects } from "./bake_tileset.mjs";
import { bakeValue, offsetCmd } from "./bake_lib.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const PLAYER_DIR = path.join(REPO_ROOT, "asset_work", "target", "assetdb", "entity", "player");
const PROFILES = JSON.parse(fs.readFileSync(path.join(PLAYER_DIR, "player_profile_specs.json"), "utf-8"));

const CELL = 128;
// Slot -> [recipe file, layer key]; slots 10-12 are pixel-baked wheels,
// 13-15 are empty. Mirrors gen_player_assets.build_player_atlas.
const VECTOR_SLOTS = [
  ["chassis", "shadow"],
  ["chassis", "body"],
  ["chassis", "spine_stream"],
  ["visor", "visor_left"],
  ["visor", "visor_right"],
  ["visor", "visor_cracked_left"],
  ["visor", "visor_cracked_right"],
  ["face", "face_normal"],
  ["face", "face_hurt"],
  ["face", "face_dead"],
];
const WHEEL_SLOTS = [10, 11, 12];
const JSON_LINE_WIDTH = 100;

const JOBS = [
  {
    id: "mouse_hero_sheet",
    profile: "HERO_CLASSIC_GEM",
    chassis: "base_chassis_gem.json",
    legacy: "mouse_hero_sheet.png",
    out: "mouse_hero_sheet.json",
  },
  {
    id: "hero_trackball_cyber_sheet",
    profile: "HERO_TRACKBALL_CYBER",
    chassis: "base_chassis_trackball.json",
    legacy: "hero_trackball_cyber_sheet.png",
    out: "hero_trackball_cyber_sheet.json",
  },
];

function shiftBox(cmd, dx) {
  const out = JSON.parse(JSON.stringify(cmd));
  const shift = (box) => [[box[0][0] + dx, box[0][1]], [box[1][0] + dx, box[1][1]]];
  if (out.box) out.box = shift(out.box);
  if (out.shape?.box) out.shape = { ...out.shape, box: shift(out.shape.box) };
  return out;
}

function formatJson(value, depth = 0) {
  const indent = "  ".repeat(depth);
  if (Array.isArray(value)) {
    if (value.length === 0) return "[]";
    const inlineable = value.every((item) => {
      if (!Array.isArray(item)) return item === null || typeof item !== "object";
      return item.every((part) => part === null || typeof part !== "object");
    });
    if (inlineable) {
      const inline = `[${value.map((item) => Array.isArray(item)
        ? `[${item.map((part) => JSON.stringify(part)).join(", ")}]`
        : JSON.stringify(item)).join(", ")}]`;
      if (indent.length + inline.length <= JSON_LINE_WIDTH) return inline;
    }
    const childIndent = "  ".repeat(depth + 1);
    return `[\n${value.map((item) => `${childIndent}${formatJson(item, depth + 1)}`).join(",\n")}\n${indent}]`;
  }
  if (value && typeof value === "object") {
    const entries = Object.entries(value);
    if (entries.length === 0) return "{}";
    const childIndent = "  ".repeat(depth + 1);
    return `{\n${entries.map(([key, item]) => `${childIndent}${JSON.stringify(key)}: ${formatJson(item, depth + 1)}`).join(",\n")}\n${indent}}`;
  }
  return JSON.stringify(value);
}

async function main() {
  const visor = JSON.parse(fs.readFileSync(path.join(PLAYER_DIR, "base_visor_shield.json"), "utf-8"));
  const face = JSON.parse(fs.readFileSync(path.join(PLAYER_DIR, "emoticons_cyber_neon.json"), "utf-8"));
  for (const job of JOBS) {
    const PALETTE = PROFILES[job.profile];
    const chassis = JSON.parse(fs.readFileSync(path.join(PLAYER_DIR, job.chassis), "utf-8"));
    const byKind = { chassis, visor, face };
    const all = [];
    VECTOR_SLOTS.forEach(([kind, layer], idx) => {
      const dx = idx * CELL;
      for (const cmd of byKind[kind][layer] ?? []) {
        all.push(offsetCmd(bakeValue(cmd, PALETTE), dx, 0));
      }
    });
    const legacy = await decodePng(fs.readFileSync(path.join(PLAYER_DIR, job.legacy)));
    for (const idx of WHEEL_SLOTS) {
      const dx = idx * CELL;
      const cell = { width: CELL, height: CELL, rgba: new Uint8ClampedArray(CELL * CELL * 4) };
      for (let y = 0; y < CELL; y++) for (let x = 0; x < CELL; x++) {
        const i = (y * legacy.width + (dx + x)) * 4, j = (y * CELL + x) * 4;
        cell.rgba[j] = legacy.rgba[i]; cell.rgba[j + 1] = legacy.rgba[i + 1];
        cell.rgba[j + 2] = legacy.rgba[i + 2]; cell.rgba[j + 3] = legacy.rgba[i + 3];
      }
      for (const cmd of bakeRects(cell)) all.push(shiftBox(cmd, dx));
    }
    const recipe = {
      provenance: {
        generated_by: "node tools/recipe/bake_player_atlas.mjs",
        chassis: `assetdb/entity/player/${job.chassis}`,
        visor: "assetdb/entity/player/base_visor_shield.json",
        face: "assetdb/entity/player/emoticons_cyber_neon.json",
        wheels: `pixel-baked from ${job.legacy} slots 10-12 (GaussianBlur has no transpiler equivalent)`,
        profile: `assetdb/entity/player/player_profile_specs.json ${job.profile}`,
        legacy: `assetdb/entity/player/${job.legacy}`,
      },
      canvas: { width: 2048, height: 128 },
      layers: { sheet: all },
    };
    if (JSON.stringify(all).includes("$")) throw new Error("unresolved $token in baked sheet");
    fs.writeFileSync(path.join(PLAYER_DIR, job.out), formatJson(recipe) + "\n", "utf-8");
    console.log(`baked ${all.length} commands -> assetdb/entity/player/${job.out}`);
  }
}

if (process.argv[1] && process.argv[1].endsWith("bake_player_atlas.mjs")) {
  await main();
}
