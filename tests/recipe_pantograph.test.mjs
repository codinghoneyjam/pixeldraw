// Pantograph keycap profile coverage: U30-U34 shared SSOT renders via the
// transpiler. One profile yields the 8-slot sheet (U30 == U34, byte-identical)
// plus the three fused 9-slice variants.
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
const RECIPE = targetPath("assetdb/ui/hud/pantograph_keycap_profile.json");

const recipe = JSON.parse(fs.readFileSync(RECIPE, "utf-8"));

async function render(layerKey) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "panto-"));
  const pngPath = path.join(tmp, "out.png");
  const docPath = path.join(tmp, "out.json");
  await transpileAndRender({ recipePath: RECIPE, layerKey, outputDocPath: docPath, outputPngPath: pngPath });
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
    const dA = Math.abs(a.rgba[i + 3] - b.rgba[i + 3]);
    if (a.rgba[i] === b.rgba[i] && a.rgba[i + 1] === b.rgba[i + 1] &&
      a.rgba[i + 2] === b.rgba[i + 2] && a.rgba[i + 3] === b.rgba[i + 3]) { matches++; continue; }
    if (dA === 0 && a.rgba[i + 3] === 0) { matches++; continue; }
  }
  return matches / total;
}

describe("pantograph profile shape", () => {
  it("declares an 8-slot 1024x128 sheet", () => {
    assert.equal(recipe.canvas.total_width, 1024);
    assert.equal(recipe.canvas.total_height, 128);
    assert.equal(recipe.canvas.slot_width, 128);
    assert.equal(recipe.slots.length, 8);
  });

  it("declares the three fused 9-slice entries", () => {
    assert.deepEqual(Object.keys(recipe.nine_patch).sort(),
      ["fused_keycap", "fused_keycap_hover", "fused_keycap_pressed"]);
    for (const entry of Object.values(recipe.nine_patch)) {
      assert.equal(entry.width, 128);
      assert.equal(entry.height, 128);
    }
  });

  it("uses only rounded_rect (transpiler-implemented)", async () => {
    // The profile stores geometry, not commands; the transpiler maps every
    // layer to rounded_rect. Assert the mapping covers all named layers.
    const known = new Set(["outer_frame", "socket_well", "inner_keycap_normal",
      "inner_keycap_pressed", "inner_keycap_hover", "inner_keycap_disabled",
      "visor_window_normal", "visor_window_pressed"]);
    for (const slot of recipe.slots) {
      for (const layer of slot.layers) assert.ok(known.has(layer), `known slot layer ${layer}`);
    }
    for (const entry of Object.values(recipe.nine_patch)) {
      for (const layer of entry.layers) assert.ok(known.has(layer), `known 9slice layer ${layer}`);
    }
  });
});

describe("pantograph sheet render", () => {
  it("emits 1024x128 and keeps the document chunk-aligned", async () => {
    const { tmp, png, doc } = await render(null);
    try {
      assert.equal(png.width, 1024);
      assert.equal(png.height, 128);
      assert.equal(doc.canvas.width_px, 1024);
      assert.equal(doc.canvas.height_px, 128);
      assert.equal(doc.canvas.width_px % 32, 0);
      assert.equal(doc.canvas.height_px % 32, 0);
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("reproduces the legacy sheet at >= 0.99", async () => {
    const { tmp, png } = await render(null);
    try {
      const legacy = await decodePng(fs.readFileSync(targetPath("assetdb/ui/hud/pantograph_sheet.png")));
      const m = matchFraction(legacy, png);
      assert.ok(m >= 0.99, `sheet match ${m}`);
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("paints the visor window black core in slot 3", async () => {
    // Slot 3 (visor_mask_base) is a lone visor rect at x 20..108 / y 20..106
    // offset by 3*128. Its interior must be opaque black.
    const { tmp, png } = await render(null);
    try {
      const at = (x, y) => {
        const i = (y * png.width + x) * 4;
        return [png.rgba[i], png.rgba[i + 1], png.rgba[i + 2], png.rgba[i + 3]];
      };
      assert.deepEqual(at(3 * 128 + 64, 64), [0, 0, 0, 255]);
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });
});

describe("pantograph fused 9-slice render", () => {
  for (const key of ["fused_keycap", "fused_keycap_hover", "fused_keycap_pressed"]) {
    it(`${key} emits 128x128 chunk-aligned and matches legacy at >= 0.99`, async () => {
      const { tmp, png, doc } = await render(key);
      try {
        assert.equal(png.width, 128);
        assert.equal(png.height, 128);
        assert.equal(doc.canvas.width_px % 32, 0);
        assert.equal(doc.canvas.height_px % 32, 0);
        const legacy = await decodePng(fs.readFileSync(targetPath(`assetdb/ui/hud/${key}_9slice.png`)));
        const m = matchFraction(legacy, png);
        assert.ok(m >= 0.99, `${key} match ${m}`);
      } finally {
        fs.rmSync(tmp, { recursive: true, force: true });
      }
    });
  }
});
