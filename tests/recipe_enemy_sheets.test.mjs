// Enemy full-sheet identity: the modern kb_* PNGs (U11-U14) are byte-identical
// to the fallback sheets the parity gate already covers, so the existing
// enemy_keybot_* cases prove the full sheets. This test pins the identity.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..");
const tidy = (n) => path.join(REPO_ROOT, "asset_work", "tidy", "numbered", n);
const gate = (n) => path.join(REPO_ROOT, "asset_work", "target", "assetdb", "entity", "enemy", n);

const CASES = [
  ["11_kb_z_amber_t1.png", "enemy_keybot_c001_sheet.png", "enemy_keybot_c001_sheet"],
  ["12_kb_f_crimson_t2.png", "enemy_keybot_t002_sheet.png", "enemy_keybot_t002_sheet"],
  ["13_kb_q_cobalt_t3.png", "enemy_keybot_r003_sheet.png", "enemy_keybot_r003_sheet"],
  ["14_kb_b_obsidian_t4.png", "enemy_keybot_b001_sheet.png", "enemy_keybot_b001_sheet"],
];

describe("enemy modern sheets are the gated fallback sheets", () => {
  for (const [numbered, legacy, gateId] of CASES) {
    it(`${numbered} is byte-identical to gated ${gateId}`, () => {
      assert.deepEqual(fs.readFileSync(tidy(numbered)), fs.readFileSync(gate(legacy)));
    });
  }

  it("boss U14 reuses the C-001 geometry (signature intentionally unbaked)", () => {
    assert.deepEqual(fs.readFileSync(tidy("14_kb_b_obsidian_t4.png")), fs.readFileSync(tidy("11_kb_z_amber_t1.png")));
  });
});
