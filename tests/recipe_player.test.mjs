// Player full-sheet coverage: U16/U18 render the whole 2048x128 atlas from
// the baked sheet recipes (bake_player_atlas.mjs); U17 is byte-identical to
// U16, so one render proves both.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { transpileAndRender } from "../tools/recipe/transpile.mjs";
import { decodePng } from "../src/io/png.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..");
const TARGET_ROOT = path.join(REPO_ROOT, "asset_work", "target");
const targetPath = (rel) => path.join(TARGET_ROOT, rel.replace(/^draw_tool_v2\//, ""));
const PLAYER = targetPath("assetdb/entity/player");

const SHEETS = [
  ["mouse_hero_sheet", "mouse_hero_sheet.json", "mouse_hero_sheet.png"],
  ["hero_trackball_cyber_sheet", "hero_trackball_cyber_sheet.json", "hero_trackball_cyber_sheet.png"],
];

const profiles = JSON.parse(fs.readFileSync(path.join(PLAYER, "player_profile_specs.json"), "utf-8"));
const manifestSpec = JSON.parse(
  fs.readFileSync(path.join(TARGET_ROOT, "tests/fixtures/player_arc/manifest.json"), "utf-8")).spec;

async function render(recipeFile) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "psheet-"));
  const pngPath = path.join(tmp, "out.png");
  const docPath = path.join(tmp, "out.json");
  await transpileAndRender({
    recipePath: path.join(PLAYER, recipeFile), layerKey: null, outputDocPath: docPath, outputPngPath: pngPath,
  });
  return {
    tmp,
    png: await decodePng(fs.readFileSync(pngPath)),
    doc: JSON.parse(fs.readFileSync(docPath, "utf-8")),
  };
}

function matchFraction(a, b) {
  assert.equal(a.width, b.width);
  assert.equal(a.height, b.height);
  const total = a.width * a.height;
  let matches = 0;
  for (let i = 0; i < total * 4; i += 4) {
    if (a.rgba[i] === b.rgba[i] && a.rgba[i + 1] === b.rgba[i + 1] &&
      a.rgba[i + 2] === b.rgba[i + 2] && a.rgba[i + 3] === b.rgba[i + 3]) { matches++; continue; }
    if (Math.abs(a.rgba[i + 3] - b.rgba[i + 3]) === 0 && a.rgba[i + 3] === 0) { matches++; continue; }
  }
  return matches / total;
}

describe("player sheet recipe shape", () => {
  it("GEM profile is the gate manifest spec; CYBER is the emerald set", () => {
    assert.deepEqual(profiles.HERO_CLASSIC_GEM, manifestSpec);
    assert.deepEqual(
      [profiles.HERO_TRACKBALL_CYBER.eye_color, profiles.HERO_TRACKBALL_CYBER.glow_color,
        profiles.HERO_TRACKBALL_CYBER.wheel_core, profiles.HERO_TRACKBALL_CYBER.wheel_light],
      ["#10B981", "#34D399", "#059669", "#A7F3D0"]);
  });

  it("U17 is byte-identical to U16 (one render proves both)", () => {
    const tidy = path.join(REPO_ROOT, "asset_work", "tidy", "numbered");
    assert.deepEqual(
      fs.readFileSync(path.join(tidy, "17_hero_classic_gem_sheet.png")),
      fs.readFileSync(path.join(tidy, "16_mouse_hero_sheet.png")));
    assert.deepEqual(
      fs.readFileSync(path.join(PLAYER, "mouse_hero_sheet.png")),
      fs.readFileSync(path.join(tidy, "16_mouse_hero_sheet.png")));
  });

  it("sheet recipes hold vector slots plus pixel-baked wheels, no $tokens", () => {
    for (const [, recipeFile] of SHEETS) {
      const recipe = JSON.parse(fs.readFileSync(path.join(PLAYER, recipeFile), "utf-8"));
      assert.equal(recipe.canvas.width, 2048);
      assert.equal(recipe.canvas.height, 128);
      assert.ok(recipe.layers.sheet.length > 1700, `wheel-inclusive command count (${recipe.layers.sheet.length})`);
      assert.ok(!JSON.stringify(recipe.layers.sheet).includes("$"), "no unresolved $tokens");
      assert.ok(recipe.layers.sheet.some((c) => c.cmd === "alpha"), "pixel-baked wheel runs present");
    }
  });
});

describe("player sheet render", () => {
  for (const [id, recipeFile, legacyFile] of SHEETS) {
    it(`${id} emits 2048x128 chunk-aligned and matches legacy at >= 0.99`, async () => {
      const { tmp, png, doc } = await render(recipeFile);
      try {
        assert.equal(png.width, 2048);
        assert.equal(png.height, 128);
        assert.equal(doc.canvas.width_px % 32, 0);
        assert.equal(doc.canvas.height_px % 32, 0);
        const legacy = await decodePng(fs.readFileSync(path.join(PLAYER, legacyFile)));
        const m = matchFraction(legacy, png);
        assert.ok(m >= 0.99, `${id} match ${m}`);
      } finally {
        fs.rmSync(tmp, { recursive: true, force: true });
      }
    });
  }
});
