// Glyph + stroke tests for the bitmap text raster branch.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { measureText, placeText, textMask } from "../src/core/raster/text_mask.js";
import { loadGlyphSet } from "../tools/recipe/text_glyphs.mjs";
import { transpileAndRender } from "../tools/recipe/transpile.mjs";
import { decodePng } from "../src/io/png.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");

const GLYPHS = loadGlyphSet(path.join(__dirname, "..", "tools", "recipe", "text_glyphs.json"));

describe("text glyph integrity", () => {
  it("bakes the planned glyphs with integer advances", () => {
    // monogram-240 carries the six logo letters plus the three sword-blade
    // engraving glyphs ([ l ]) added for title_diorama_sword_body.
    assert.deepEqual(
      Object.keys(GLYPHS["monogram-240"].glyphs).sort(),
      ["[", "]", "d", "l", "o", "r", "u", "w", "y"],
    );
    assert.deepEqual(Object.keys(GLYPHS["default-42"].glyphs), ["A"]);
    for (const [name, font] of Object.entries(GLYPHS)) {
      for (const [ch, g] of Object.entries(font.glyphs)) {
        assert.ok(Number.isInteger(g.advance), `${name}/${ch} integer advance`);
        assert.equal(g.alpha.length, g.w * g.h, `${name}/${ch} alpha size`);
        assert.ok(g.alpha.some((v) => v !== 0), `${name}/${ch} non-empty`);
      }
    }
  });

  it("measures the diamond A like PIL textbbox", () => {
    const m = measureText(GLYPHS["default-42"].glyphs, "A");
    assert.equal(m.w, 27, "tw");
    assert.equal(m.h, 29, "th");
  });

  it("places chars on the integer pen grid", () => {
    const placed = placeText(GLYPHS["monogram-240"].glyphs, "our", 24, -15);
    assert.deepEqual(placed.map((p) => [p.x, p.y]), [[24, 70], [114, 70], [204, 70]]);
  });
});

describe("title logo text reproduction", () => {
  for (const t of ["our", "or", "d"]) {
    it(`title_logo_text_${t} transpiles integer-exact`, async () => {
      const recipePath = path.join(REPO_ROOT, "draw_tool_v2/tests/fixtures", `title_logo_text_${t}_recipe.json`);
      const recipe = JSON.parse(fs.readFileSync(recipePath, "utf-8"));
      const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "titletext-"));
      const png = path.join(tmp, `${t}.png`);
      await transpileAndRender({
        recipePath,
        layerKey: null,
        outputDocPath: path.join(tmp, `${t}.json`),
        outputPngPath: png,
      });
      const legacy = await decodePng(fs.readFileSync(path.join(REPO_ROOT, "assetdb/ui/title", `title_logo_text_${t}.png`)));
      const cand = await decodePng(fs.readFileSync(png));
      const [cw, ch] = recipe.crop;
      assert.equal(legacy.width, cw, "legacy width");
      assert.equal(legacy.height, ch, "legacy height");
      const a = legacy.rgba;
      const b = cand.rgba;
      let diffs = 0;
      for (let y = 0; y < ch; y++) {
        for (let x = 0; x < cw; x++) {
          const i = (y * legacy.width + x) * 4;
          const j = (y * cand.width + x) * 4;
          if (a[i] !== b[j] || a[i + 1] !== b[j + 1] || a[i + 2] !== b[j + 2] || a[i + 3] !== b[j + 3]) diffs++;
        }
      }
      assert.equal(diffs, 0, `exact match, ${diffs} diffs`);
    });
  }
});

describe("hud icons atlas text cell", () => {
  it("diamond_gem body transpiles to >= 0.99 of the legacy cell", async () => {
    // 16384/16384 after the T-7b line fix: the three width-2 line commands in
    // diamond_gem body now reproduce PIL exactly.
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "diamond-"));
    const png = path.join(tmp, "diamond.png");
    await transpileAndRender({
      recipePath: path.join(REPO_ROOT, "assetdb/ui/data/diamond_gem.json"),
      layerKey: "body",
      outputDocPath: path.join(tmp, "diamond.json"),
      outputPngPath: png,
    });
    const legacy = await decodePng(fs.readFileSync(path.join(REPO_ROOT, "assetdb/ui/hud", "hud_icons_128_atlas.png")));
    const cand = await decodePng(fs.readFileSync(png));
    // diamond_gem is ICON_NAMES index 7 -> cell (384, 128) in the 512 atlas
    const total = 128 * 128;
    let matches = 0;
    for (let y = 0; y < 128; y++) {
      for (let x = 0; x < 128; x++) {
        const i = ((128 + y) * 512 + (384 + x)) * 4;
        const j = (y * 128 + x) * 4;
        if (legacy.rgba[i] === cand.rgba[j] && legacy.rgba[i + 1] === cand.rgba[j + 1] &&
          legacy.rgba[i + 2] === cand.rgba[j + 2] && legacy.rgba[i + 3] === cand.rgba[j + 3]) matches++;
      }
    }
    assert.ok(matches / total >= 0.99, `diamond cell match ${matches}/${total}`);
  });
});
