// JSON spec consistency: the 10 runtime specs under tidy/numbered are not
// parity-gated, but they must agree with the reproduced assets. This gate pins
// the enemy/player/weapon cross-references and the two known divergences.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..");
const numbered = (n) => path.join(REPO_ROOT, "asset_work", "tidy", "numbered", n);
const target = (rel) => path.join(REPO_ROOT, "asset_work", "target", rel);
const spec = (n) => JSON.parse(fs.readFileSync(numbered(n), "utf-8"));
const ENEMY_IDS = ["kb_z_amber_t1", "kb_f_crimson_t2", "kb_q_cobalt_t3", "kb_b_obsidian_t4"];

describe("enemy specs agree with baked sheets", () => {
  it("encounter pools reference exactly the defined enemy ids", () => {
    const enemy = spec("01_enemy.json");
    const pools = spec("06_encounter_pools.json");
    const used = new Set();
    for (const pool of Object.values(pools)) {
      for (const entry of pool.enemies) {
        assert.ok(enemy[entry.enemy], `pool references defined enemy ${entry.enemy}`);
        used.add(entry.enemy);
      }
    }
    assert.deepEqual([...used].sort(), [...ENEMY_IDS].sort(), "every enemy is pooled");
  });

  it("each signature color is baked somewhere (or deliberately not)", () => {
    const enemy = spec("01_enemy.json");
    const pal = (t) => JSON.parse(fs.readFileSync(
      target(`assetdb/entity/enemy/enemy_${t}_sheet.json`), "utf-8")).palette;
    assert.equal(pal("C-001").body_color, enemy.kb_z_amber_t1.signature_color_hex);
    assert.equal(pal("T-002").body_color, enemy.kb_f_crimson_t2.signature_color_hex);
    // R-003 bakes deep navy bodies; the cobalt signature lives in the glow.
    assert.equal(pal("R-003").glow_color, enemy.kb_q_cobalt_t3.signature_color_hex);
    // The obsidian boss signature is intentionally NOT baked (gate comment).
    assert.ok(!Object.values(pal("C-001")).includes(enemy.kb_b_obsidian_t4.signature_color_hex));
  });
});

describe("player specs agree with sheet palettes", () => {
  it("mouse/trackball signatures match their sheet eye colors", () => {
    const player = spec("02_player.json");
    const profiles = JSON.parse(fs.readFileSync(
      target("assetdb/entity/player/player_profile_specs.json"), "utf-8"));
    assert.equal(profiles.HERO_CLASSIC_GEM.eye_color, player.mouse_hero.signature_color_hex);
    assert.equal(profiles.HERO_TRACKBALL_CYBER.eye_color, player.hero_trackball_cyber.signature_color_hex);
  });

  it("classic_gem amber spec diverges from its sky-tinted sheet (known)", () => {
    // U17 is byte-identical to the sky U16 sheet, but the spec names amber
    // #F59E0B: the sheet predates the amber signature. Pinned, not hidden.
    const player = spec("02_player.json");
    const profiles = JSON.parse(fs.readFileSync(
      target("assetdb/entity/player/player_profile_specs.json"), "utf-8"));
    assert.equal(player.hero_classic_gem.signature_color_hex, "#F59E0B");
    assert.notEqual(profiles.HERO_CLASSIC_GEM.eye_color, "#F59E0B");
  });
});

describe("weapon specs agree with reproduced files", () => {
  it("weapon ids match the manifest and its files exist", () => {
    const weapons = spec("03_weapon.json");
    const manifest = spec("04_modular_weapons_manifest.json");
    assert.deepEqual(Object.keys(weapons).sort(), Object.keys(manifest).sort());
    for (const [id, entry] of Object.entries(manifest)) {
      assert.ok(fs.existsSync(target(`assetdb/entity/weapon/${entry.albedo}`)), `${id} albedo present`);
      assert.ok(fs.existsSync(target(`assetdb/entity/weapon/${entry.emission_mask}`)), `${id} mask present`);
    }
  });
});

describe("layout specs stay well-formed", () => {
  it("room/layout/panel specs keep their top-level keys", () => {
    assert.ok(Object.keys(spec("05_layouts.json")).length > 0, "layouts non-empty");
    assert.ok(spec("07_room_profiles.json").combat_room_boss_01, "boss room present");
    assert.ok(spec("08_room_archetypes.json").BOSS, "BOSS archetype present");
    assert.ok(spec("09_ui_panels.json").hud_player_status, "player status panel present");
  });
});
