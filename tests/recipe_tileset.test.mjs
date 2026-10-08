// Tileset coverage: U26/U27 render byte-exact from reverse-engineered
// solid-rect recipes (tools/recipe/bake_tileset.mjs output).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { transpileAndRender } from "../tools/recipe/transpile.mjs";
import { bakeRects } from "../tools/recipe/bake_tileset.mjs";
import { decodePng } from "../src/io/png.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..");
const TARGET_ROOT = path.join(REPO_ROOT, "asset_work", "target");
const targetPath = (rel) => path.join(TARGET_ROOT, rel.replace(/^draw_tool_v2\//, ""));

const CASES = [
  ["room_tileset", "assetdb/world/stage/room_tileset.json", "assetdb/world/stage/room_tileset.png", 256, 64],
  ["lobby_tileset", "assetdb/world/lobby/lobby_tileset.json", "assetdb/world/lobby/lobby_tileset.png", 192, 64],
];

async function render(recipeRel) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tiles-"));
  const pngPath = path.join(tmp, "out.png");
  const docPath = path.join(tmp, "out.json");
  await transpileAndRender({
    recipePath: targetPath(recipeRel), layerKey: null, outputDocPath: docPath, outputPngPath: pngPath,
  });
  return {
    tmp,
    png: await decodePng(fs.readFileSync(pngPath)),
    doc: JSON.parse(fs.readFileSync(docPath, "utf-8")),
  };
}

describe("tileset recipe shape", () => {
  for (const [id, recipeRel, , w, h] of CASES) {
    it(`${id} is a solid-rect decomposition of its legacy PNG`, async () => {
      const recipe = JSON.parse(fs.readFileSync(targetPath(recipeRel), "utf-8"));
      assert.equal(recipe.canvas.width, w);
      assert.equal(recipe.canvas.height, h);
      assert.ok(recipe.provenance?.generated_by?.includes("bake_tileset"), "baker provenance");
      const cmds = recipe.layers.tiles;
      assert.ok(cmds.length > 0 && cmds.length < 200, `compact rect count (${cmds.length})`);
      for (const c of cmds) {
        assert.equal(c.cmd, "rect");
        assert.ok(/^#[0-9A-F]{6}$/.test(c.fill), `opaque hex fill ${c.fill}`);
        const [[x0, y0], [x1, y1]] = c.box;
        assert.ok(0 <= x0 && x0 <= x1 && x1 < w && 0 <= y0 && y0 <= y1 && y1 < h, "rect inside canvas");
      }
      // Baker determinism: re-baking the legacy PNG yields the same commands.
      const legacy = await decodePng(fs.readFileSync(targetPath(CASES.find((c) => c[0] === id)[2])));
      assert.deepEqual(bakeRects(legacy), cmds, "baker reproduces the committed recipe");
    });
  }
});

describe("tileset render", () => {
  for (const [id, recipeRel, legacyRel, w, h] of CASES) {
    it(`${id} emits ${w}x${h} chunk-aligned and matches legacy byte-exact`, async () => {
      const { tmp, png, doc } = await render(recipeRel);
      try {
        assert.equal(png.width, w);
        assert.equal(png.height, h);
        assert.equal(doc.canvas.width_px % 32, 0);
        assert.equal(doc.canvas.height_px % 32, 0);
        const legacy = await decodePng(fs.readFileSync(targetPath(legacyRel)));
        assert.equal(legacy.width, w);
        assert.equal(legacy.height, h);
        assert.deepEqual([...png.rgba], [...legacy.rgba], "pixel-exact");
      } finally {
        fs.rmSync(tmp, { recursive: true, force: true });
      }
    });
  }

  it("keycap_debris emits 64x64 chunk-aligned and matches legacy byte-exact", async () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tiles-"));
    try {
      const recipeRel = "assetdb/entity/enemy/base_debris.json";
      const pngPath = path.join(tmp, "out.png");
      const docPath = path.join(tmp, "out.json");
      await transpileAndRender({
        recipePath: targetPath(recipeRel), layerKey: null, outputDocPath: docPath, outputPngPath: pngPath,
      });
      const png = await decodePng(fs.readFileSync(pngPath));
      const doc = JSON.parse(fs.readFileSync(docPath, "utf-8"));
      assert.equal(png.width, 64);
      assert.equal(png.height, 64);
      assert.equal(doc.canvas.width_px % 32, 0);
      assert.equal(doc.canvas.height_px % 32, 0);
      const recipe = JSON.parse(fs.readFileSync(targetPath(recipeRel), "utf-8"));
      assert.ok(recipe.provenance?.generated_by?.includes("bake_tileset"), "baker provenance");
      const legacy = await decodePng(fs.readFileSync(targetPath("assetdb/entity/enemy/keycap_debris.png")));
      assert.deepEqual(bakeRects(legacy), recipe.layers.debris, "baker reproduces the committed recipe");
      assert.deepEqual([...png.rgba], [...legacy.rgba], "pixel-exact");
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("title_weapons_atlas emits 288x96 chunk-aligned and matches legacy byte-exact", async () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "tiles-"));
    try {
      const recipeRel = "assetdb/ui/title/title_weapons_atlas.json";
      const pngPath = path.join(tmp, "out.png");
      const docPath = path.join(tmp, "out.json");
      await transpileAndRender({
        recipePath: targetPath(recipeRel), layerKey: null, outputDocPath: docPath, outputPngPath: pngPath,
      });
      const png = await decodePng(fs.readFileSync(pngPath));
      const doc = JSON.parse(fs.readFileSync(docPath, "utf-8"));
      assert.equal(png.width, 288);
      assert.equal(png.height, 96);
      assert.equal(doc.canvas.width_px % 32, 0);
      assert.equal(doc.canvas.height_px % 32, 0);
      const recipe = JSON.parse(fs.readFileSync(targetPath(recipeRel), "utf-8"));
      assert.ok(recipe.provenance?.generated_by?.includes("bake_tileset"), "baker provenance");
      const legacy = await decodePng(fs.readFileSync(targetPath("assetdb/ui/title/title_weapons_atlas.png")));
      assert.deepEqual(bakeRects(legacy), recipe.layers.atlas, "baker reproduces the committed recipe");
      assert.deepEqual([...png.rgba], [...legacy.rgba], "pixel-exact");
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });
});
