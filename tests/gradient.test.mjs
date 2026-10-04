// Golden + profile tests for the radial multi-stop gradient raster branch.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { radialGradientMask } from "../src/core/raster/gradient.js";
import { unpackRGBA } from "../src/core/pixel.js";
import { transpileAndRender } from "../tools/recipe/transpile.mjs";
import { decodePng } from "../src/io/png.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");

const G = JSON.parse(
  fs.readFileSync(new URL("./fixtures/raster_golden.json", import.meta.url), "utf-8"),
);

const rowsOf = (mask) => {
  const rows = [];
  for (let y = 0; y < mask.h; y++) {
    const row = [];
    for (let x = 0; x < mask.w; x++) row.push([...unpackRGBA(mask.data[y * mask.w + x])]);
    rows.push(row);
  }
  return rows;
};

describe("gradient golden bit-match", () => {
  it("radialGradientMask matches all golden gradient rgba rows", () => {
    for (const c of G.gradient) {
      const m = radialGradientMask({ cx: c.cx, cy: c.cy, r0: c.r0, r1: c.r1, stops: c.stops, w: c.w, h: c.h });
      assert.deepEqual(rowsOf(m), c.rgba, `gradient ${c.name}`);
    }
  });

  it("ring profile is non-monotonic: alpha rises to a peak then falls", () => {
    const c = G.gradient.find((g) => g.name === "ring_nonmonotonic");
    const m = radialGradientMask({ cx: c.cx, cy: c.cy, r0: c.r0, r1: c.r1, stops: c.stops, w: c.w, h: c.h });
    const cy = Math.floor(c.h / 2);
    const alphas = [];
    for (let x = 0; x < c.w; x++) alphas.push(unpackRGBA(m.data[cy * c.w + x])[3]);
    const peak = Math.max(...alphas);
    assert.ok(peak >= 240, `ring peak near 255 (got ${peak})`);
    assert.equal(alphas[0], 0, "far outside band is transparent");
    assert.equal(alphas[c.w - 1], 0, "far outside band is transparent");
    const peakIdx = alphas.indexOf(peak);
    assert.ok(peakIdx > 0 && peakIdx < c.w - 1, "peak sits inside the row");
    assert.ok(alphas.slice(0, peakIdx + 1).every((a, i, s) => i === 0 || s[i - 1] <= a), "alpha rises to the peak");
  });

  it("degenerate span (r1 <= r0) is a hard edge without throwing", () => {
    const m = radialGradientMask({ cx: 4, cy: 4, r0: 3, r1: 3, stops: [[0, 9, 9, 9, 255], [1, 0, 0, 0, 0]], w: 8, h: 8 });
    assert.equal(unpackRGBA(m.data[4 * 8 + 4])[3], 255, "inside the radius is solid");
    assert.equal(m.data[0], 0, "outside the radius is transparent");
  });
});

describe("shockwave_ring reproduction", () => {
  it("transpiled recipe matches legacy PNG on alpha at >= 0.99", async () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "shockwave-"));
    const png = path.join(tmp, "shockwave_ring.png");
    await transpileAndRender({
      recipePath: path.join(REPO_ROOT, "draw_tool_v2/tests/fixtures/shockwave_ring_recipe.json"),
      layerKey: null,
      outputDocPath: path.join(tmp, "shockwave_ring.json"),
      outputPngPath: png,
    });
    const legacy = await decodePng(fs.readFileSync(path.join(REPO_ROOT, "assetdb/entity/player/shockwave_ring.png")));
    const cand = await decodePng(fs.readFileSync(png));
    assert.equal(legacy.width, cand.width, "width");
    assert.equal(legacy.height, cand.height, "height");
    const total = legacy.width * legacy.height;
    let matches = 0;
    for (let i = 0; i < total * 4; i += 4) {
      if (legacy.rgba[i + 3] === cand.rgba[i + 3]) matches++;
    }
    assert.ok(matches / total >= 0.99, `alpha match ${matches}/${total}`);
  });
});
