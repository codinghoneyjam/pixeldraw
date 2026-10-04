// Smoke tests for the recipe transpiler: valid PNG output for legacy weapon recipes.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { transpileAndRender } from "../tools/recipe/transpile.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");

const PNG_SIG = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

function assertValidPng(filePath, width, height) {
  const buf = fs.readFileSync(filePath);
  assert.ok(buf.length > 8, "PNG non-empty");
  assert.deepEqual(buf.subarray(0, 8), PNG_SIG, "PNG signature");
  assert.equal(buf.subarray(12, 16).toString("ascii"), "IHDR");
  assert.equal(buf.readUInt32BE(16), width, "PNG width");
  assert.equal(buf.readUInt32BE(20), height, "PNG height");
}

describe("recipe_parity smoke", () => {
  it("weapon_sword albedo transpiles to a 128x128 PNG", async () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "recipe-parity-"));
    const png = path.join(tmp, "weapon_sword.png");
    await transpileAndRender({
      recipePath: path.join(REPO_ROOT, "assetdb/entity/weapon/weapon_sword.json"),
      layerKey: "albedo",
      outputDocPath: path.join(tmp, "weapon_sword.json"),
      outputPngPath: png,
    });
    assertValidPng(png, 128, 128);
  });

  it("weapon_bow albedo transpiles to a 128x128 PNG", async () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "recipe-parity-"));
    const png = path.join(tmp, "weapon_bow.png");
    await transpileAndRender({
      recipePath: path.join(REPO_ROOT, "assetdb/entity/weapon/weapon_bow.json"),
      layerKey: "albedo",
      outputDocPath: path.join(tmp, "weapon_bow.json"),
      outputPngPath: png,
    });
    assertValidPng(png, 128, 128);
  });
});
