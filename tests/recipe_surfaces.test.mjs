// Composite-surface coverage: U36/U38/U39 + U37 scanline render from the
// hud_composite_surfaces.json manifest SSOT (one case per surface id).
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
const MANIFEST = targetPath("assetdb/ui/hud/hud_composite_surfaces.json");

const manifest = JSON.parse(fs.readFileSync(MANIFEST, "utf-8"));
const byId = Object.fromEntries(manifest.surfaces.map((s) => [s.id, s]));

const CASES = [
  ["keycap_screen_9slice", 128, 64],
  ["keyboard_plate_block", 256, 256],
  ["floating_visor_window", 256, 256],
  ["scanline_tile_fine", 32, 32],
];

async function render(surfaceId) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "surf-"));
  const pngPath = path.join(tmp, "out.png");
  const docPath = path.join(tmp, "out.json");
  await transpileAndRender({ recipePath: MANIFEST, layerKey: surfaceId, outputDocPath: docPath, outputPngPath: pngPath });
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

describe("surface manifest shape", () => {
  it("carries the tidy evidence surfaces plus the baker-added scanline", () => {
    for (const id of ["keyboard_plate_block", "keycap_screen_9slice", "floating_visor_window"]) {
      assert.ok(byId[id], `surface ${id} present`);
    }
    assert.ok(byId.scanline_tile_fine, "scanline entry added for U37");
    assert.match(byId.scanline_tile_fine.role, /U37/, "scanline entry documents its divergence");
  });

  it("uses only layer types both builders support", () => {
    const supported = new Set(["rounded_rect", "rect", "line", "polygon"]);
    for (const [id] of CASES) {
      for (const layer of byId[id].layers) {
        assert.ok(supported.has(layer.type), `${id}: supported layer ${layer.type}`);
      }
    }
  });
});

describe("surface render", () => {
  for (const [id, w, h] of CASES) {
    it(`${id} emits ${w}x${h} chunk-aligned and matches legacy at >= 0.99`, async () => {
      const { tmp, png, doc } = await render(id);
      try {
        assert.equal(png.width, w);
        assert.equal(png.height, h);
        assert.equal(doc.canvas.width_px % 32, 0);
        assert.equal(doc.canvas.height_px % 32, 0);
        const legacy = await decodePng(fs.readFileSync(targetPath(`assetdb/ui/hud/${id}.png`)));
        const m = matchFraction(legacy, png);
        assert.ok(m >= 0.99, `${id} match ${m}`);
      } finally {
        fs.rmSync(tmp, { recursive: true, force: true });
      }
    });
  }

  it("scanline rows are byte-exact at y=3,6,...,30", async () => {
    const { tmp, png } = await render("scanline_tile_fine");
    try {
      const at = (x, y) => {
        const i = (y * png.width + x) * 4;
        return [png.rgba[i], png.rgba[i + 1], png.rgba[i + 2], png.rgba[i + 3]];
      };
      for (let y = 0; y < 32; y++) {
        const want = y % 3 === 0 && y !== 0 ? [34, 211, 238, 40] : [0, 0, 0, 0];
        assert.deepEqual(at(0, y), want, `row ${y}`);
        assert.deepEqual(at(31, y), want, `row ${y} right edge`);
      }
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });
});
