// Browser-side recipe import path: recipeJsonToDocument must rasterise the
// same pixels as the Node transpile path.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

import { recipeJsonToDocument } from "../src/io/serialize.js";
import { transpileAndRender } from "../tools/recipe/transpile.mjs";
import { exportPngBytes } from "../src/io/export_png.js";
import { decodePng } from "../src/io/png.js";
import path from "node:path";
import os from "node:os";

async function rgbaOfDoc(doc) {
  const bytes = await exportPngBytes(doc, {});
  return decodePng(bytes);
}

describe("recipeJsonToDocument", () => {
  it("base_debris (rect bake) matches the transpile render exactly", async () => {
    const recipePath = path.join("asset_work", "target", "assetdb/entity/enemy/base_debris.json");
    const obj = JSON.parse(fs.readFileSync(recipePath, "utf-8"));
    const doc = recipeJsonToDocument(obj);
    assert.equal(doc.layers.length, 1);

    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "recipe-import-"));
    const { pngBytes } = await transpileAndRender({ recipePath, outputDocPath: path.join(tmp, "a.json"), outputPngPath: path.join(tmp, "a.png") });
    const viaBrowser = await rgbaOfDoc(doc);
    const viaNode = await decodePng(pngBytes);
    assert.equal(viaBrowser.width, viaNode.width);
    assert.equal(viaBrowser.height, viaNode.height);
    assert.deepEqual(Buffer.from(viaBrowser.rgba).equals(Buffer.from(viaNode.rgba)), true);
  });

  it("weapon_sword splits albedo/mask into two layers", () => {
    const obj = JSON.parse(fs.readFileSync(path.join("asset_work", "target", "assetdb/entity/weapon/weapon_sword.json"), "utf-8"));
    const doc = recipeJsonToDocument(obj);
    assert.deepEqual(doc.layers.map((l) => l.id), ["albedo", "mask"]);
  });

  it("enemy sheet exposes one visible sheet layer + 16 hidden slot layers", () => {
    const obj = JSON.parse(fs.readFileSync(path.join("asset_work", "target", "assetdb/entity/enemy/enemy_C-001_sheet.json"), "utf-8"));
    const doc = recipeJsonToDocument(obj);
    assert.equal(doc.canvas.widthPx, 2048);
    assert.equal(doc.layers[0].id, "sheet");
    assert.equal(doc.layers[0].visible, true);
    assert.equal(doc.layers.length, 17);
    assert.equal(doc.layers.filter((l) => l.visible).length, 1);
  });

  it("pantograph profile renders the full sheet", () => {
    const obj = JSON.parse(fs.readFileSync(path.join("asset_work", "target", "assetdb/ui/hud/pantograph_keycap_profile.json"), "utf-8"));
    const doc = recipeJsonToDocument(obj);
    assert.equal(doc.canvas.widthPx, obj.canvas.total_width);
    assert.equal(doc.layers.length, 1);
  });

  it("surface manifest imports the first surface by default", () => {
    const obj = JSON.parse(fs.readFileSync(path.join("asset_work", "target", "assetdb/ui/hud/hud_composite_surfaces.json"), "utf-8"));
    const doc = recipeJsonToDocument(obj);
    assert.equal(doc.layers.length, 1);
    assert.equal(doc.layers[0].id, obj.surfaces[0].id);
  });
});
