// Golden + layer-parity tests for the arc/chord raster branch.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { arcMask, chordFillMask, chordOutlineMask } from "../src/core/raster/arc.js";
import { transpileAndRender } from "../tools/recipe/transpile.mjs";
import { decodePng } from "../src/io/png.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// Working assets live under asset_work/target/ (recipes + legacy PNGs + oracle ground truth).
const REPO_ROOT = path.resolve(__dirname, "..");
const TARGET_ROOT = path.join(REPO_ROOT, "asset_work", "target");
const targetPath = (rel) => path.join(TARGET_ROOT, rel.replace(/^draw_tool_v2\//, ""));

const G = JSON.parse(
  fs.readFileSync(new URL("./fixtures/raster_golden.json", import.meta.url), "utf-8"),
);

const rowsOfMask = (mask) => {
  const rows = [];
  for (let y = 0; y < mask.h; y++) {
    let s = "";
    for (let x = 0; x < mask.w; x++) s += mask.data[y * mask.w + x] === 1 ? "#" : ".";
    rows.push(s);
  }
  return rows;
};

const wantOf = (c) => c.rows.map((r) => [...r].map((ch) => ch === "#"));

function nearParity(gotRows, want, maxMissing, maxExtras, msg) {
  let missing = 0;
  let extras = 0;
  for (let y = 0; y < want.length; y++) {
    for (let x = 0; x < want[0].length; x++) {
      if (want[y][x] && !gotRows[y][x]) missing++;
      if (gotRows[y][x] && !want[y][x]) extras++;
    }
  }
  assert.ok(missing <= maxMissing, `${msg} drops ${missing} > ${maxMissing}`);
  assert.ok(extras <= maxExtras, `${msg} overshoot ${extras} > ${maxExtras}`);
}

const boolRowsOf = (mask) => {
  const rows = [];
  for (let y = 0; y < mask.h; y++) {
    const row = [];
    for (let x = 0; x < mask.w; x++) row.push(mask.data[y * mask.w + x] === 1);
    rows.push(row);
  }
  return rows;
};

describe("arc golden near-parity", () => {
  it("arcMask covers every PIL pixel with bounded overshoot", () => {
    for (const c of G.arc) {
      const m = arcMask((c.box[0][0] + c.box[1][0]) / 2, (c.box[0][1] + c.box[1][1]) / 2,
        (c.box[1][0] - c.box[0][0]) / 2, (c.box[1][1] - c.box[0][1]) / 2,
        c.start, c.end, c.width, c.w, c.h);
      assert.deepEqual(rowsOfMask(m).length, c.rows.length, `arc ${c.name} height`);
      nearParity(boolRowsOf(m), wantOf(c), 0, 16, `arc ${c.name}`);
    }
  });

  it("chordFillMask covers every PIL pixel with bounded overshoot", () => {
    for (const c of G.chord.filter((g) => !("width" in g))) {
      const m = chordFillMask((c.box[0][0] + c.box[1][0]) / 2, (c.box[0][1] + c.box[1][1]) / 2,
        (c.box[1][0] - c.box[0][0]) / 2, (c.box[1][1] - c.box[0][1]) / 2,
        c.start, c.end, c.w, c.h);
      nearParity(boolRowsOf(m), wantOf(c), 0, 4, `chord_fill ${c.name}`);
    }
  });

  it("chordOutlineMask paints the arc plus the closing line", () => {
    const c = G.chord.find((g) => "width" in g);
    const cx = (c.box[0][0] + c.box[1][0]) / 2;
    const cy = (c.box[0][1] + c.box[1][1]) / 2;
    const rx = (c.box[1][0] - c.box[0][0]) / 2;
    const ry = (c.box[1][1] - c.box[0][1]) / 2;
    const m = chordOutlineMask(cx, cy, rx, ry, c.start, c.end, c.width, c.w, c.h);
    const painted = m.data.reduce((a, v) => a + v, 0);
    assert.ok(painted > 0, "outline paints pixels");
    nearParity(boolRowsOf(m), wantOf(c), 16, 40, `chord_outline ${c.name}`);
  });
});

describe("player arc/chord layer parity", () => {
  const manifest = JSON.parse(
    fs.readFileSync(new URL("./fixtures/player_arc/manifest.json", import.meta.url), "utf-8"),
  );
  for (const c of manifest.cases) {
    it(`${c.id} transpiles to >= 0.99 of the legacy layer`, async () => {
      const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "arc-layer-"));
      const png = path.join(tmp, `${c.id}.png`);
      await transpileAndRender({
        recipePath: targetPath(c.recipe),
        layerKey: c.layer,
        paletteOverrides: manifest.spec,
        outputDocPath: path.join(tmp, `${c.id}.json`),
        outputPngPath: png,
      });
      const legacy = await decodePng(fs.readFileSync(
        path.join(TARGET_ROOT, "draw_tool_v2/tests/fixtures/player_arc".replace(/^draw_tool_v2\//, ""), `${c.id}_legacy.png`)));
      const cand = await decodePng(fs.readFileSync(png));
      assert.equal(legacy.width, cand.width, "width");
      assert.equal(legacy.height, cand.height, "height");
      const total = legacy.width * legacy.height;
      let matches = 0;
      for (let i = 0; i < total * 4; i += 4) {
        if (legacy.rgba[i] === cand.rgba[i] && legacy.rgba[i + 1] === cand.rgba[i + 1] &&
          legacy.rgba[i + 2] === cand.rgba[i + 2] && legacy.rgba[i + 3] === cand.rgba[i + 3]) matches++;
      }
      assert.ok(matches / total >= 0.99, `${c.id} match ${matches}/${total}`);
    });
  }
});
