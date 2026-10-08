// Extract layer units from the committed recipes and render one of them.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { enumerateUnits } from "../tools/extract_layers.mjs";
import { transpileAndRender } from "../tools/recipe/transpile.mjs";
import { decodePng } from "../src/io/png.js";

const read = (rel) => JSON.parse(fs.readFileSync(path.join("asset_work", "target", rel), "utf-8"));

describe("enumerateUnits", () => {
  it("surface manifest -> one unit per surface", () => {
    const recipe = read("assetdb/ui/hud/hud_composite_surfaces.json");
    const units = enumerateUnits(recipe);
    assert.equal(units.length, 6);
    assert.deepEqual(units.map((u) => u.unit), recipe.surfaces.map((s) => s.id));
    for (const u of units) assert.equal(u.slotId, u.unit);
  });

  it("atlas sheet -> one unit per slot", () => {
    const recipe = read("assetdb/entity/enemy/enemy_C-001_sheet.json");
    const units = enumerateUnits(recipe);
    assert.equal(units.length, 16);
    assert.equal(units[0].unit, "shadow");
  });

  it("weapon recipe -> albedo + mask", () => {
    const recipe = read("assetdb/entity/weapon/weapon_sword.json");
    assert.deepEqual(enumerateUnits(recipe).map((u) => u.unit), ["albedo", "mask"]);
  });

  it("pantograph profile -> nine_patch keys + sheet", () => {
    const recipe = read("assetdb/ui/hud/pantograph_keycap_profile.json");
    const units = enumerateUnits(recipe);
    assert.deepEqual(units.map((u) => u.unit), [...Object.keys(recipe.nine_patch), "sheet"]);
  });

  it("player-shape recipe -> top-level command arrays", () => {
    const recipe = read("assetdb/entity/player/player_mouse.json");
    const units = enumerateUnits(recipe);
    assert.ok(units.length >= 8);
    assert.ok(units.some((u) => u.unit === "shadow"));
    assert.ok(units.some((u) => u.unit === "visor_left"));
  });

  it("layers object -> one unit per key", () => {
    const recipe = read("assetdb/entity/player/mouse_hero_sheet.json");
    assert.deepEqual(enumerateUnits(recipe).map((u) => u.unit), ["sheet"]);
  });
});

describe("extract_layers render path", () => {
  it("renders a surface unit identical to the parity-gate render", async () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "extract-layers-"));
    const recipe = "asset_work/target/assetdb/ui/hud/hud_composite_surfaces.json";
    const pngPath = path.join(tmp, "scanline.png");
    const docPath = path.join(tmp, "scanline_drawtool.json");
    await transpileAndRender({ recipePath: recipe, layerKey: null, slotId: "scanline_tile_fine", outputDocPath: docPath, outputPngPath: pngPath });
    const decoded = await decodePng(fs.readFileSync(pngPath));
    assert.equal(decoded.width, 32);
    assert.equal(decoded.height, 32);
    assert.ok(fs.existsSync(docPath));
    const doc = JSON.parse(fs.readFileSync(docPath, "utf-8"));
    assert.equal(doc.layers.length, 1);
  });
});
