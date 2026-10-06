# RECIPE MAP - USED PNG REGENERATION INDEX

Scope: 34 USED PNG under assetdb (11-44). One row per PNG.
Dirs: target/ flat pairs, mapping/ per-asset cards, tools/ verbatim tool copies.
Note: target/ recipe copies are byte copies and may contain UTF-8 Korean from source SSOT. This index file is ASCII-only.

## METHOD LEGEND

RECIPE_JSON: committed JSON recipe + committed generator invocation exists.
RECIPE_JSONx3/x11/SHARED: one PNG needs many recipes, or many PNG share one recipe (duplicated under U prefix in target/).
RECIPE_UNWIRED: recipe file exists but no generator references it.
CODE_ONLY/FONT_ONLY: no JSON; pure python (+font) generates it.
TRES_EXTRACTED: extracted from deleted .tres; no JSON now.
NO_MANIFEST: renderer exists but manifest has no entry.
MISSING_PROFILE: tool needs profiles JSON; none committed.
RECIPE_JSON_DUAL: v2 JSON recipe exists and legacy code bake path also exists.

## TABLE (ID PNG RECIPE METHOD TOOL)

| ID | PNG | RECIPE | METHOD | TOOL |
|----|-----|--------|--------|------|
| U11 | kb_z_amber_t1.png | C-001.json | RECIPE_JSON | gen_enemy_assets.py --enemy C-001 |
| U12 | kb_f_crimson_t2.png | T-002.json | RECIPE_JSON | gen_enemy_assets.py --enemy T-002 |
| U13 | kb_q_cobalt_t3.png | R-003.json | RECIPE_JSON | gen_enemy_assets.py --enemy R-003 |
| U14 | kb_b_obsidian_t4.png | C-001.json | RECIPE_JSON+VIA_OVERRIDE | B-001 reuses C-001 geometry, color #FF3B30 from enemy.json |
| U15 | keycap_debris.png | base_debris.json | RECIPE_UNWIRED | no generator caller; runtime tint only |
| U16 | mouse_hero_sheet.png | base_chassis_gem.json + base_visor_shield.json + emoticons_cyber_neon.json | RECIPE_JSONx3 | gen_player_assets.py |
| U17 | hero_classic_gem_sheet.png | base_chassis_gem.json + base_visor_shield.json + emoticons_cyber_neon.json | RECIPE_JSONx3 | gen_player_assets.py |
| U18 | hero_trackball_cyber_sheet.png | base_chassis_trackball.json + base_visor_shield.json + emoticons_cyber_neon.json | RECIPE_JSONx3 | gen_player_assets.py |
| U19 | shockwave_ring.png | none | CODE_ONLY | gen_speedster_assets.py |
| U20 | sword_albedo.png | weapon_sword.json | RECIPE_JSON | gen_modular_weapons.py |
| U21 | sword_mask.png | weapon_sword.json | RECIPE_JSON | gen_modular_weapons.py mask output |
| U22 | bow_albedo.png | weapon_bow.json | RECIPE_JSON | gen_modular_weapons.py |
| U23 | bow_mask.png | weapon_bow.json | RECIPE_JSON | gen_modular_weapons.py mask output |
| U24 | spear_albedo.png | weapon_spear.json | RECIPE_JSON | gen_modular_weapons.py |
| U25 | spear_mask.png | weapon_spear.json | RECIPE_JSON | gen_modular_weapons.py mask output |
| U26 | room_tileset.png | none | TRES_EXTRACTED | v2/migrate_assets.py 256x64, source deleted |
| U27 | lobby_tileset.png | none | TRES_EXTRACTED | v2/migrate_assets.py 192x64, source deleted |
| U28 | portal_keycap_unpressed.png | portal_keycap_master.json | RECIPE_JSON | gen_portal_assets.py unpressed layer |
| U29 | portal_keycap_pressed.png | portal_keycap_master.json | RECIPE_JSON | gen_portal_assets.py pressed layer |
| U30 | keycap_sheet.png | pantograph_keycap_profile.json | RECIPE_JSON_SHARED | gen_pantograph_atlas.py |
| U31 | fused_keycap_9slice.png | pantograph_keycap_profile.json | RECIPE_JSON_SHARED | gen_pantograph_atlas.py nine_patch |
| U32 | fused_keycap_hover_9slice.png | pantograph_keycap_profile.json | RECIPE_JSON_SHARED | gen_pantograph_atlas.py nine_patch |
| U33 | fused_keycap_pressed_9slice.png | pantograph_keycap_profile.json | RECIPE_JSON_SHARED | gen_pantograph_atlas.py nine_patch |
| U34 | pantograph_sheet.png | pantograph_keycap_profile.json | RECIPE_JSON_SHARED | gen_pantograph_atlas.py 8 slots |
| U35 | hud_icons_128_atlas.png | heart+shield+lightning+speaker+book+gear+stopwatch+diamond_gem+cross_mount+kill_skull+chest (11) | RECIPE_JSONx11 | gen_hud_icons_128.py |
| U36 | keycap_screen_9slice.png | hud_composite_surfaces.json | RECIPE_JSON_SHARED | gen_hud_surfaces.py |
| U37 | scanline_tile_fine.png | none | NO_MANIFEST | gen_ui_surfaces.py renderer only, 32x32 |
| U38 | keyboard_plate_block.png | hud_composite_surfaces.json | RECIPE_JSON_SHARED | gen_hud_surfaces.py 256x256 |
| U39 | floating_visor_window.png | hud_composite_surfaces.json | RECIPE_JSON_SHARED | gen_hud_surfaces.py polygon |
| U40 | title_weapons_atlas.png | none | MISSING_PROFILE | gen_weapon_visuals.py needs profiles JSON |
| U41 | title_diorama_sword_body.png | title_diorama_sword_body.json | RECIPE_JSON_DUAL | v2/engine.py or gen_title_assets.py:148 |
| U42 | title_logo_text_our.png | none | FONT_ONLY | gen_title_assets.py + monogram.ttf 240px |
| U43 | title_logo_text_or.png | none | FONT_ONLY | gen_title_assets.py + monogram.ttf 240px |
| U44 | title_logo_text_d.png | none | FONT_ONLY | gen_title_assets.py + monogram.ttf 240px |

## JSON-LESS 8 + UNWIRED 1

No JSON: U19, U26, U27, U37, U40, U42, U43, U44.
Unwired recipe: U15 base_debris.json (fills '' x3 + #333333 x1, zero generator refs).
Regenerable without JSON today: U19, U41-via-code, U42-44 (code+font).
Not regenerable today: U26, U27 (source gone), U37 (no manifest entry), U40 (no profiles file), U15 (no wiring).

## DEBRIS COLOR PRINCIPLE

Shared texture is gray: keycap_debris.png == debris_1.png (SHA e2fbdbf4, 64x64, 7 RGBA).
Per-enemy color is runtime tint, not file variant:
enemy.json signature_color_hex (4 values) -> KeybotVisualSpec.sync_from_dict:14
-> KeybotPuppet.apply_visual_spec:133 set_color -> KeybotFxPuppet.emit_debris:41 particles.color=color + texture=_debris_tex.
base_debris.json empty fills ('') are tint slots; only switch_stem #333333 is fixed.
debris_2/3.png are unused variants (different SHA, zero callers).

## DIRS

target/: U<NN>_png_<base> + U<NN>_recipe_<base> + U<NN>_note_*.txt, flat, 85 files.
mapping/: U<NN>.txt, 34 cards with PNG/RECIPE/METHOD/TOOL/COMMAND.
tools/: 22 verbatim .py copies (generators + v2/engine + migrate + core libs).
numbered/: 44 canonical USED copies. unused/: 17 orphan copies.
gallery.html: visual USED vs UNUSED browser.
