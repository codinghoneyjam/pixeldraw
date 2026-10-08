// HUD icon atlas coverage: U35 renders the full 512x512 atlas from the baked
// combined recipe (tools/recipe/bake_hud_atlas.mjs output) in one call.
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
const HUD = targetPath("assetdb/ui/hud");
const RECIPE = path.join(HUD, "hud_icons_128_atlas.json");
const LEGACY = path.join(HUD, "hud_icons_128_atlas.png");

const ICON_NAMES = [
  "heart", "shield", "lightning", "speaker",
  "book", "gear", "stopwatch", "diamond_gem",
  "cross_mount", "kill_skull", "chest",
];

const recipe = JSON.parse(fs.readFileSync(RECIPE, "utf-8"));

async function render() {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "hud-"));
  const pngPath = path.join(tmp, "out.png");
  const docPath = path.join(tmp, "out.json");
  await transpileAndRender({ recipePath: RECIPE, layerKey: null, outputDocPath: docPath, outputPngPath: pngPath });
  return {
    tmp,
    png: await decodePng(fs.readFileSync(pngPath)),
    doc: JSON.parse(fs.readFileSync(docPath, "utf-8")),
  };
}

function region(png, stride, ox, oy, w, h) {
  const out = [];
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = ((oy + y) * stride + (ox + x)) * 4;
    out.push([png.rgba[i], png.rgba[i + 1], png.rgba[i + 2], png.rgba[i + 3]]);
  }
  return out;
}

describe("hud atlas recipe shape", () => {
  it("covers all 11 icons with baked literals only", () => {
    let bodyTotal = 0;
    for (const name of ICON_NAMES) {
      const icon = JSON.parse(fs.readFileSync(path.join(HUD, "icons", `${name}.json`), "utf-8"));
      assert.ok(Array.isArray(icon.body), `${name} has a body command list`);
      bodyTotal += icon.body.length;
    }
    assert.equal(recipe.layers.atlas.length, bodyTotal, "atlas holds every icon command");
    assert.ok(!JSON.stringify(recipe).includes("$"), "no unresolved $tokens");
  });

  it("declares the 512x512 canvas and provenance", () => {
    assert.equal(recipe.canvas.width, 512);
    assert.equal(recipe.canvas.height, 512);
    assert.deepEqual(recipe.provenance.sources, ICON_NAMES.map((n) => `assetdb/ui/hud/icons/${n}.json`));
  });

  it("uses only command types the transpiler implements", () => {
    const supported = new Set(["rect", "rounded_rect", "circle", "ellipse", "line", "lines",
      "spokes", "polygon", "arc", "chord", "radial_gradient", "text"]);
    for (const c of recipe.layers.atlas) {
      assert.ok(supported.has(c.cmd ?? c.type), `unsupported ${(c.cmd ?? c.type)} (${c.desc})`);
    }
  });
});

describe("hud atlas render", () => {
  it("emits 512x512 and keeps the document chunk-aligned", async () => {
    const { tmp, png, doc } = await render();
    try {
      assert.equal(png.width, 512);
      assert.equal(png.height, 512);
      assert.equal(doc.canvas.width_px, 512);
      assert.equal(doc.canvas.height_px, 512);
      assert.equal(doc.canvas.width_px % 32, 0);
      assert.equal(doc.canvas.height_px % 32, 0);
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("reproduces the legacy atlas at >= 0.99", async () => {
    const { tmp, png } = await render();
    try {
      const legacy = await decodePng(fs.readFileSync(LEGACY));
      const total = 512 * 512;
      let matches = 0;
      for (let i = 0; i < total * 4; i += 4) {
        if (legacy.rgba[i] === png.rgba[i] && legacy.rgba[i + 1] === png.rgba[i + 1] &&
          legacy.rgba[i + 2] === png.rgba[i + 2] && legacy.rgba[i + 3] === png.rgba[i + 3]) { matches++; continue; }
        if (Math.abs(legacy.rgba[i + 3] - png.rgba[i + 3]) === 0 && legacy.rgba[i + 3] === 0) { matches++; continue; }
      }
      assert.ok(matches / total >= 0.99, `hud atlas match ${matches}/${total}`);
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("keeps cells 11-15 empty and pins the exact book/cross cells", async () => {
    const { tmp, png } = await render();
    try {
      const legacy = await decodePng(fs.readFileSync(LEGACY));
      for (let idx = 11; idx < 16; idx++) {
        const ox = (idx % 4) * 128, oy = Math.floor(idx / 4) * 128;
        for (const [r, g, b, a] of region(png, 512, ox, oy, 128, 128)) {
          assert.equal(a, 0, `cell ${idx} stays transparent`);
        }
      }
      // book (idx 4) and cross_mount (idx 8) have zero residual per-cell.
      for (const idx of [4, 8]) {
        const ox = (idx % 4) * 128, oy = Math.floor(idx / 4) * 128;
        const got = region(png, 512, ox, oy, 128, 128);
        const want = region(legacy, 512, ox, oy, 128, 128);
        assert.deepEqual(got, want, `cell ${idx} is pixel-exact`);
      }
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });
});
