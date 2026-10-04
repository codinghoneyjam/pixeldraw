// Sword recipe coverage: the 280x560 non-aligned asset, its glyph dependency,
// and the viewport contract that lets it be stored chunk-aligned.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { transpileAndRender } from "../tools/recipe/transpile.mjs";
import { loadGlyphSet } from "../tools/recipe/text_glyphs.mjs";
import { decodePng } from "../src/io/png.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const RECIPE = path.join(REPO_ROOT, "assetdb/ui/data/title_diorama_sword_body.json");
const LEGACY = path.join(REPO_ROOT, "assetdb/ui/title/title_diorama_sword_body.png");

const recipe = JSON.parse(fs.readFileSync(RECIPE, "utf-8"));
const GLYPHS = loadGlyphSet(path.join(__dirname, "..", "tools", "recipe", "text_glyphs.json"));

// <META - ROLE : Transpile the sword recipe once and decode the result | helper>
async function render() {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "sword-"));
  const png = path.join(tmp, "sword.png");
  const docPath = path.join(tmp, "sword.json");
  await transpileAndRender({ recipePath: RECIPE, layerKey: "body", outputDocPath: docPath, outputPngPath: png });
  return {
    tmp,
    png: await decodePng(fs.readFileSync(png)),
    doc: JSON.parse(fs.readFileSync(docPath, "utf-8")),
  };
}

describe("sword recipe shape", () => {
  it("keeps the stored document 32-aligned and crops via viewport", () => {
    const ex = recipe.export;
    assert.equal(ex.canvas.width % 32, 0, "stored width is a 32 multiple");
    assert.equal(ex.canvas.height % 32, 0, "stored height is a 32 multiple");
    assert.deepEqual(ex.viewport, { x: 0, y: 0, w: 280, h: 560 });
    assert.equal(ex.canvas.width, 288, "padded to the next 32 multiple");
    assert.equal(ex.canvas.height, 576);
  });

  it("uses only command types the transpiler implements", () => {
    const supported = new Set(["rect", "rounded_rect", "circle", "ellipse", "line", "lines",
      "spokes", "polygon", "arc", "chord", "radial_gradient", "text"]);
    for (const c of recipe.body) {
      assert.ok(supported.has(c.cmd), `unsupported command type ${c.cmd} (${c.desc})`);
    }
  });

  it("covers the whole 280x560 legacy canvas with its last row", () => {
    // blade_poly reaches y=546 and the guard starts at y=100; assert the recipe
    // actually exercises the lower half so a truncated bake cannot pass.
    const maxY = Math.max(...recipe.body.flatMap((c) =>
      (c.points ?? c.box ?? []).map((p) => (Array.isArray(p) ? p[1] : p.y))));
    assert.ok(maxY >= 540, `recipe reaches y=${maxY}`);
    assert.ok(minYOf(recipe.body) <= 20, "recipe starts near the top");
  });

  it("declares the three engraving glyphs the set carries", () => {
    const needed = recipe.body.filter((c) => c.cmd === "text").map((c) => c.text);
    assert.deepEqual(needed, ["[", "l", "]"]);
    for (const ch of needed) {
      assert.ok(GLYPHS["monogram-240"].glyphs[ch], `monogram-240 carries ${ch}`);
    }
  });

  it("uses disk-stroked glyph variants matching the legacy outlined text", () => {
    // bake_cybernetic_sword_aligned stamps stroke_width 8 and 9 via a disc
    // (ox^2 + oy^2 <= sw^2), so the baked shaped variants must exist for both.
    for (const c of recipe.body.filter((x) => x.cmd === "text")) {
      const gl = GLYPHS["monogram-240"].glyphs[c.text];
      assert.ok(gl.shaped?.[String(c.stroke_width)],
        `${c.text} has a shaped variant for stroke_width ${c.stroke_width}`);
    }
  });

  it("uses raw pen origins, not centred positions", () => {
    // The legacy draws at a literal pen; pos would be re-centred via textbbox
    // and land the glyphs in the wrong place.
    for (const c of recipe.body.filter((x) => x.cmd === "text")) {
      assert.ok(Array.isArray(c.pen), `${c.desc} uses pen`);
      assert.equal(c.pos, undefined, `${c.desc} must not set pos`);
    }
  });
});

describe("sword recipe render", () => {
  it("emits the viewport size and keeps the document chunk-aligned", async () => {
    const { tmp, png, doc } = await render();
    try {
      assert.equal(png.width, 280, "emitted PNG width is the legacy width");
      assert.equal(png.height, 560, "emitted PNG height is the legacy height");
      assert.equal(doc.canvas.width_px, 288, "stored document is padded");
      assert.equal(doc.canvas.height_px, 576);
      assert.deepEqual(doc.canvas.viewport, { x: 0, y: 0, w: 280, h: 560 });
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("reproduces the legacy PNG at >= 0.99", async () => {
    const { tmp, png } = await render();
    try {
      const legacy = await decodePng(fs.readFileSync(LEGACY));
      assert.equal(legacy.width, png.width);
      assert.equal(legacy.height, png.height);
      const total = legacy.width * legacy.height;
      let matches = 0;
      for (let i = 0; i < total * 4; i += 4) {
        if (legacy.rgba[i] === png.rgba[i] && legacy.rgba[i + 1] === png.rgba[i + 1] &&
          legacy.rgba[i + 2] === png.rgba[i + 2] && legacy.rgba[i + 3] === png.rgba[i + 3]) matches++;
      }
      // Measured 156772/156800 = 0.999821. The residual 28 px are the four
      // rounded-rect corner arcs of the visor housing/lens pair, where our
      // corner rasteriser rounds one pixel differently from PIL.
      assert.ok(matches / total >= 0.99, `sword match ${matches}/${total}`);
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("paints both engraving glyphs and the blade glints", async () => {
    const { tmp, png } = await render();
    try {
      const at = (x, y) => {
        const i = (y * png.width + x) * 4;
        return [png.rgba[i], png.rgba[i + 1], png.rgba[i + 2], png.rgba[i + 3]];
      };
      // Engraving bracket '[' ink sits around x 34..80, y 219..340.
      let cyan = 0;
      for (let y = 210; y < 350; y++) for (let x = 20; x < 240; x++) {
        const [r, g, b, a] = at(x, y);
        if (a !== 0 && b === 0xFF && g === 0xE5 && r === 0x00) cyan++;
      }
      assert.ok(cyan > 500, `engraving cyan ink present (got ${cyan})`);
      // Blade glints are semi-transparent over the blade.
      let translucent = 0;
      for (let y = 200; y < 500; y++) for (let x = 100; x < 180; x++) {
        if (at(x, y)[3] === 0x96) translucent++;
      }
      assert.ok(translucent > 500, `blade glint alpha preserved (got ${translucent})`);
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });
});

// <META - ROLE : Smallest y across all recipe coordinates | helper>
function minYOf(cmds) {
  return Math.min(...cmds.flatMap((c) =>
    (c.points ?? c.box ?? []).map((p) => (Array.isArray(p) ? p[1] : p.y))));
}
