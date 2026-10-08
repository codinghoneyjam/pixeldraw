// <META - FILE SUMMARY - Shared recipe-case tables for parity gate and layer extraction
//
// CASES / LAYER_CASES enumerate the 39 render cases (34 PNGs, some layers
// share one recipe render). palette overrides live on PALETTE_SPEC.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..");
const TARGET_ROOT = path.join(REPO_ROOT, "asset_work", "target");

export const resolveCasePath = (rel) =>
  path.join(TARGET_ROOT, rel.startsWith("draw_tool_v2/") ? rel.slice("draw_tool_v2/".length) : rel);

// The 13 single-invocation assets. Portal socket/sky_gate are excluded (no PNG).
// Layer keys match the scratch all_assets_verify.py baseline exactly (no --slot).
export const CASES = [
  { id: "sword_albedo", recipe: "assetdb/entity/weapon/weapon_sword.json", layer: "albedo", legacy: "assetdb/entity/weapon/sword_albedo.png" },
  { id: "sword_mask", recipe: "assetdb/entity/weapon/weapon_sword.json", layer: "mask", legacy: "assetdb/entity/weapon/sword_mask.png" },
  { id: "bow_albedo", recipe: "assetdb/entity/weapon/weapon_bow.json", layer: "albedo", legacy: "assetdb/entity/weapon/bow_albedo.png" },
  { id: "bow_mask", recipe: "assetdb/entity/weapon/weapon_bow.json", layer: "mask", legacy: "assetdb/entity/weapon/bow_mask.png" },
  { id: "spear_albedo", recipe: "assetdb/entity/weapon/weapon_spear.json", layer: "albedo", legacy: "assetdb/entity/weapon/spear_albedo.png" },
  { id: "spear_mask", recipe: "assetdb/entity/weapon/weapon_spear.json", layer: "mask", legacy: "assetdb/entity/weapon/spear_mask.png" },
  { id: "portal_keycap_unpressed", recipe: "assetdb/world/object/portal_keycap_master.json", layer: "keycap_unpressed", legacy: "assetdb/world/img/portal/portal_keycap_unpressed.png" },
  { id: "portal_keycap_pressed", recipe: "assetdb/world/object/portal_keycap_master.json", layer: "keycap_pressed", legacy: "assetdb/world/img/portal/portal_keycap_pressed.png" },
  // Non 32-multiple asset (280x560). The recipe declares export.canvas 288x576
  // plus a viewport so the stored document stays chunk-aligned; the transpiler
  // emits the cropped 280x560 PNG.
  { id: "title_diorama_sword_body", recipe: "assetdb/ui/data/title_diorama_sword_body.json", layer: "body", legacy: "assetdb/ui/title/title_diorama_sword_body.png" },
  // Title logo type. Recipes live under draw_tool_v2/tests/fixtures/ because the
  // committed PNGs were baked from these exact specs, not from assetdb recipes.
  { id: "title_logo_text_our", recipe: "draw_tool_v2/tests/fixtures/title_logo_text_our_recipe.json", layer: null, legacy: "assetdb/ui/title/title_logo_text_our.png" },
  { id: "title_logo_text_or", recipe: "draw_tool_v2/tests/fixtures/title_logo_text_or_recipe.json", layer: null, legacy: "assetdb/ui/title/title_logo_text_or.png" },
  { id: "title_logo_text_d", recipe: "draw_tool_v2/tests/fixtures/title_logo_text_d_recipe.json", layer: null, legacy: "assetdb/ui/title/title_logo_text_d.png" },
  { id: "shockwave_ring", recipe: "draw_tool_v2/tests/fixtures/shockwave_ring_recipe.json", layer: null, legacy: "assetdb/entity/player/shockwave_ring.png" },
  // 16-slot enemy atlas (2048x128). The recipes flatten the legacy shape spec
  // into per-slot command lists; residual is the sheen stamp model and the
  // visor_50/25 cutout path. See docs/40_work/task/active/DRAWTOOL_REPRO.md.
  { id: "enemy_keybot_c001_sheet", recipe: "assetdb/entity/enemy/enemy_C-001_sheet.json", layer: null, legacy: "assetdb/entity/enemy/enemy_keybot_c001_sheet.png" },
  { id: "enemy_keybot_t002_sheet", recipe: "assetdb/entity/enemy/enemy_T-002_sheet.json", layer: null, legacy: "assetdb/entity/enemy/enemy_keybot_t002_sheet.png" },
  { id: "enemy_keybot_r003_sheet", recipe: "assetdb/entity/enemy/enemy_R-003_sheet.json", layer: null, legacy: "assetdb/entity/enemy/enemy_keybot_r003_sheet.png" },
  // B-001 (boss) is byte-identical to C-001 in the committed PNGs, so its recipe
  // is C-001's palette. The boss signature #FF3B30 is intentionally NOT baked
  // yet -- re-baking the boss is a separate, deliberate change.
  { id: "enemy_keybot_b001_sheet", recipe: "assetdb/entity/enemy/enemy_B-001_sheet.json", layer: null, legacy: "assetdb/entity/enemy/enemy_keybot_b001_sheet.png" },
  // Pantograph keycap profile (U30-U34 SSOT). One profile renders the 8-slot
  // sheet (U30 keycap_sheet == U34 pantograph_sheet, byte-identical) plus the
  // three fused 9-slice variants. Residual is the rrect corner rasteriser
  // (same 1px class as sword/portal: ~0.995-0.997).
  { id: "pantograph_sheet", recipe: "assetdb/ui/hud/pantograph_keycap_profile.json", layer: null, legacy: "assetdb/ui/hud/pantograph_sheet.png" },
  { id: "fused_keycap_9slice", recipe: "assetdb/ui/hud/pantograph_keycap_profile.json", layer: "fused_keycap", legacy: "assetdb/ui/hud/fused_keycap_9slice.png" },
  { id: "fused_keycap_hover_9slice", recipe: "assetdb/ui/hud/pantograph_keycap_profile.json", layer: "fused_keycap_hover", legacy: "assetdb/ui/hud/fused_keycap_hover_9slice.png" },
  { id: "fused_keycap_pressed_9slice", recipe: "assetdb/ui/hud/pantograph_keycap_profile.json", layer: "fused_keycap_pressed", legacy: "assetdb/ui/hud/fused_keycap_pressed_9slice.png" },
  // HUD icon atlas (U35): 11 per-icon shape recipes baked into one 512x512
  // transpiler-native atlas recipe (tools/recipe/bake_hud_atlas.mjs). Cells
  // 11-15 are empty. Residual is the arc/chord rasteriser (speaker/gear/
  // chest cells) plus the diamond_gem default-42 "A" glyph, which the
  // transpiler skips (monogram-240 only) -- a few hundred px, gate still 0.99+.
  { id: "hud_icons_128_atlas", recipe: "assetdb/ui/hud/hud_icons_128_atlas.json", layer: null, legacy: "assetdb/ui/hud/hud_icons_128_atlas.png" },
  // Composite surfaces (U36/U38/U39) + scanline tile (U37): one manifest SSOT,
  // one case per surface id. The U37 scanline entry is baker-added (mapping
  // card prescription); its rows are verified byte-exact in tests.
  { id: "keycap_screen_9slice", recipe: "assetdb/ui/hud/hud_composite_surfaces.json", layer: "keycap_screen_9slice", legacy: "assetdb/ui/hud/keycap_screen_9slice.png" },
  { id: "keyboard_plate_block", recipe: "assetdb/ui/hud/hud_composite_surfaces.json", layer: "keyboard_plate_block", legacy: "assetdb/ui/hud/keyboard_plate_block.png" },
  { id: "floating_visor_window", recipe: "assetdb/ui/hud/hud_composite_surfaces.json", layer: "floating_visor_window", legacy: "assetdb/ui/hud/floating_visor_window.png" },
  { id: "scanline_tile_fine", recipe: "assetdb/ui/hud/hud_composite_surfaces.json", layer: "scanline_tile_fine", legacy: "assetdb/ui/hud/scanline_tile_fine.png" },
  // TRES-extracted tilesets (U26/U27): source deleted, recipes are
  // reverse-engineered solid-rect decompositions (bake_tileset.mjs) -- exact.
  { id: "room_tileset", recipe: "assetdb/world/stage/room_tileset.json", layer: null, legacy: "assetdb/world/stage/room_tileset.png" },
  { id: "lobby_tileset", recipe: "assetdb/world/lobby/lobby_tileset.json", layer: null, legacy: "assetdb/world/lobby/lobby_tileset.png" },
  // Keycap debris (U15): unwired quad SSOT with tint slots; the committed PNG
  // is gold-tinted with different quads, so the recipe is a pixel-faithful
  // rect bake (bake_tileset.mjs) -- exact. Runtime tint stays in the engine.
  { id: "keycap_debris", recipe: "assetdb/entity/enemy/base_debris.json", layer: null, legacy: "assetdb/entity/enemy/keycap_debris.png" },
  // Title weapons atlas (U40): no committed profiles JSON; pixel-faithful
  // rect+alpha bake (bake_tileset.mjs) -- exact, incl. the alpha-77 edge.
  { id: "title_weapons_atlas", recipe: "assetdb/ui/title/title_weapons_atlas.json", layer: null, legacy: "assetdb/ui/title/title_weapons_atlas.png" },
  // Player full sheets (U16-U18): vector slots 0-9 baked with the profile
  // palette + wheel slots 10-12 pixel-baked (GaussianBlur). U17 is
  // byte-identical to U16, so one render proves both (asserted in tests).
  { id: "mouse_hero_sheet", recipe: "assetdb/entity/player/mouse_hero_sheet.json", layer: null, legacy: "assetdb/entity/player/mouse_hero_sheet.png" },
  { id: "hero_trackball_cyber_sheet", recipe: "assetdb/entity/player/hero_trackball_cyber_sheet.json", layer: null, legacy: "assetdb/entity/player/hero_trackball_cyber_sheet.png" },
];

// Player / chassis / emote layers promoted from tests/fixtures/player_arc/.
// Those legacy PNGs are per-layer 128x128 crops baked with the manifest's
// palette spec, so each case carries its own palette override.
export const PALETTE_SPEC = JSON.parse(
  fs.readFileSync(path.join(TARGET_ROOT, "tests/fixtures/player_arc/manifest.json"), "utf-8"),
).spec;
export const LAYER_CASES = [
  { id: "player_mouse__shadow", recipe: "assetdb/entity/player/player_mouse.json", layer: "shadow" },
  { id: "player_mouse__visor_left", recipe: "assetdb/entity/player/player_mouse.json", layer: "visor_left" },
  { id: "player_mouse__visor_right", recipe: "assetdb/entity/player/player_mouse.json", layer: "visor_right" },
  { id: "base_chassis_gem__shadow", recipe: "assetdb/entity/player/base_chassis_gem.json", layer: "shadow" },
  { id: "base_chassis_trackball__shadow", recipe: "assetdb/entity/player/base_chassis_trackball.json", layer: "shadow" },
  { id: "base_chassis_trackball__body", recipe: "assetdb/entity/player/base_chassis_trackball.json", layer: "body" },
  { id: "emoticons_cyber_neon__face_normal", recipe: "assetdb/entity/player/emoticons_cyber_neon.json", layer: "face_normal" },
].map((c) => ({ ...c, legacy: `tests/fixtures/player_arc/${c.id}_legacy.png`, palette: PALETTE_SPEC }));
