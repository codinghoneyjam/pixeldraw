# ACTIVE DRAWING ASSET INVENTORY - Your wor(l)d

Scope: runtime drawing PNG + JSON only, 1:1 matched.
Method: static AssetDB call graph from features/, presentation/, core/.
Excluded: pure data JSON, generator recipes, unreachable PNG, shaders, fonts, audio.
Numbered dir: asset_tidy/numbered/ (44 files, 01_ to 44_ prefix).
Source root: assetdb/ (canonical SSOT, domain isolated).

## NUMBERING RULE

01-10: drawing JSON specs.
11-44: drawing PNG textures.
Prefix preserves sort order. Original basename kept after prefix.
Example: 01_enemy.json <- assetdb/entity/enemy/enemy.json.

## ENTITY - JSON (4)

| No | Numbered file | Source path | Loader | Renderer | Pair |
|----|---------------|-------------|--------|----------|------|
| 01 | 01_enemy.json | assetdb/entity/enemy/enemy.json | EntityAssetDB.get_enemy_spec:188, get_all_enemy_specs:207, get_bestiary_entries:217, get_visual_registry:108 | KeybotPuppet.load_visual_profile:143, PuppetFactory.create:7 | 11,12,13,14 |
| 02 | 02_player.json | assetdb/entity/player/player.json | EntityAssetDB.get_player_spec:236, get_player_visual_data:104, get_visual_registry:108 | PlayerPuppet.apply_visual_profile:46, PuppetFactory.create:7 | 16,17,18 |
| 03 | 03_weapon.json | assetdb/entity/weapon/weapon.json | EntityAssetDB.get_weapon_data:56, get_weapon_spec:253, get_all_weapon_data:64 | WeaponPuppet.setup_weapon_profile:10, ProceduralWeaponAssembler.assemble:11, CraftWeaponPreviewCard:145 | 20-25 |
| 04 | 04_modular_weapons_manifest.json | assetdb/entity/weapon/modular_weapons_manifest.json | EntityAssetDB.get_weapon_manifest:83 | ProceduralWeaponAssembler._get_weapon_metadata:80 | 20-25 |

## WORLD - JSON (4)

| No | Numbered file | Source path | Loader | Renderer | Pair |
|----|---------------|-------------|--------|----------|------|
| 05 | 05_layouts.json | assetdb/world/stage/layouts.json | WorldAssetDB.get_stage_layout:47 | RoomProfileCatalog:158, RoomTilemapPuppet.render_sparse_layout:36 | 26 |
| 06 | 06_encounter_pools.json | assetdb/world/stage/encounter_pools.json | WorldAssetDB.get_encounter_pools:58 | EncounterPoolCatalog:69 | 26 |
| 07 | 07_room_profiles.json | assetdb/world/stage/room_profiles.json | WorldAssetDB.get_room_profiles:64 | RoomProfileCatalog:67, RoomTilemapPuppet.render_floor:22 | 26,27 |
| 08 | 08_room_archetypes.json | assetdb/world/stage/room_archetypes.json | WorldAssetDB.get_room_archetypes:70 | RoomProfileCatalog:69 | 26,27 |

## UI - JSON (2)

| No | Numbered file | Source path | Loader | Renderer | Pair |
|----|---------------|-------------|--------|----------|------|
| 09 | 09_ui_panels.json | assetdb/ui/hud/ui_panels.json | UiAssetDB.get_panel_data:40 | UiLayoutCatalog:13 | 36,38,39 |
| 10 | 10_pantograph_keycap_profile.json | assetdb/ui/data/pantograph_keycap_profile.json | UiAssetDB.get_pantograph_profile:98 | KeycapSlotView atlas slicing:198, TitleKeycapSurface | 34 |

## ENTITY - PNG (15)

| No | Numbered file | Source path | Loader | Renderer | JSON |
|----|---------------|-------------|--------|----------|------|
| 11 | 11_kb_z_amber_t1.png | assetdb/entity/enemy/kb_z_amber_t1.png | EntityAssetDB.get_enemy_sheet:168 | KeybotBodyLayer/Visor/Emoticon/Accessory via setup_from_sheet | 01 |
| 12 | 12_kb_f_crimson_t2.png | assetdb/entity/enemy/kb_f_crimson_t2.png | EntityAssetDB.get_enemy_sheet:168 | KeybotPuppet layers, same path | 01 |
| 13 | 13_kb_q_cobalt_t3.png | assetdb/entity/enemy/kb_q_cobalt_t3.png | EntityAssetDB.get_enemy_sheet:168 | KeybotPuppet layers, same path | 01 |
| 14 | 14_kb_b_obsidian_t4.png | assetdb/entity/enemy/kb_b_obsidian_t4.png | EntityAssetDB.get_enemy_sheet:168 | KeybotPuppet layers, boss scale 1.8 | 01 |
| 15 | 15_keycap_debris.png | assetdb/entity/enemy/keycap_debris.png | EntityAssetDB.get_enemy_debris_texture:184 | KeybotFxPuppet.emit_debris:41 | 01 |
| 16 | 16_mouse_hero_sheet.png | assetdb/entity/player/mouse_hero_sheet.png | EntityAssetDB.get_player_sheet:148 | PlayerPuppet 4 layers: body/visor/emoticon/accessory | 02 |
| 17 | 17_hero_classic_gem_sheet.png | assetdb/entity/player/hero_classic_gem_sheet.png | EntityAssetDB.get_player_sheet:148 | PlayerPuppet, same 4 layers | 02 |
| 18 | 18_hero_trackball_cyber_sheet.png | assetdb/entity/player/hero_trackball_cyber_sheet.png | EntityAssetDB.get_player_sheet:148 | PlayerPuppet, same 4 layers | 02 |
| 19 | 19_shockwave_ring.png | assetdb/entity/player/shockwave_ring.png | EntityAssetDB.get_shockwave_texture:140 | PlayerTrailPuppet:86 | 02 |
| 20 | 20_sword_albedo.png | assetdb/entity/weapon/sword_albedo.png | EntityAssetDB.get_weapon_texture:68 | ProceduralWeaponAssembler._build_modular_weapon:26 + WeaponPuppet | 03,04 |
| 21 | 21_sword_mask.png | assetdb/entity/weapon/sword_mask.png | EntityAssetDB.get_weapon_mask_texture:72 | Same, mask_texture shader param | 04 |
| 22 | 22_bow_albedo.png | assetdb/entity/weapon/bow_albedo.png | EntityAssetDB.get_weapon_texture:68 | Same + bow wire Line2D | 03,04 |
| 23 | 23_bow_mask.png | assetdb/entity/weapon/bow_mask.png | EntityAssetDB.get_weapon_mask_texture:72 | Same, mask_texture shader param | 04 |
| 24 | 24_spear_albedo.png | assetdb/entity/weapon/spear_albedo.png | EntityAssetDB.get_weapon_texture:68 | Same modular path | 03,04 |
| 25 | 25_spear_mask.png | assetdb/entity/weapon/spear_mask.png | EntityAssetDB.get_weapon_mask_texture:72 | Same, mask_texture shader param | 04 |

## WORLD - PNG (4)

| No | Numbered file | Source path | Loader | Renderer | JSON |
|----|---------------|-------------|--------|----------|------|
| 26 | 26_room_tileset.png | assetdb/world/stage/room_tileset.png | WorldAssetDB.get_room_tileset:23 via TilesetBuilder 4 cols | RoomTilemapPuppet.setup_tileset:9 + render_floor/render_sparse_layout | 05,06,07,08 |
| 27 | 27_lobby_tileset.png | assetdb/world/lobby/lobby_tileset.png | WorldAssetDB.get_lobby_tileset:35 via TilesetBuilder 3 cols | LobbyWorld:40 tile_map_layer | 07,08 |
| 28 | 28_portal_keycap_unpressed.png | assetdb/world/img/portal/portal_keycap_unpressed.png | WorldAssetDB.get_portal_texture:76 | PortalPuppet._apply_cap_texture:53, CapSprite | - |
| 29 | 29_portal_keycap_pressed.png | assetdb/world/img/portal/portal_keycap_pressed.png | WorldAssetDB.get_portal_texture:76 | PortalPuppet.set_activation_state:30 + burst tween | - |

## GLOBAL KEYCAP - PNG (4)

| No | Numbered file | Source path | Loader | Renderer | JSON |
|----|---------------|-------------|--------|----------|------|
| 30 | 30_keycap_sheet.png | assetdb/global/img/keycap/keycap_sheet.png | GlobalAssetDB.get_keycap_texture:53 key=sheet | TitleKeycapSurface atlas, KeycapSlotView sub-texture | 10 |
| 31 | 31_fused_keycap_9slice.png | assetdb/global/img/keycap/fused_keycap_9slice.png | GlobalAssetDB.get_keycap_texture:53 key=normal/default | WordFusionAnimator NinePatchRect:89, TitleKeycapSurface menu | 10 |
| 32 | 32_fused_keycap_hover_9slice.png | assetdb/global/img/keycap/fused_keycap_hover_9slice.png | GlobalAssetDB.get_keycap_texture:53 key=hover | TitleKeycapSurface hover state | 10 |
| 33 | 33_fused_keycap_pressed_9slice.png | assetdb/global/img/keycap/fused_keycap_pressed_9slice.png | GlobalAssetDB.get_keycap_texture:53 key=pressed | TitleKeycapSurface pressed, fusion plaque | 10 |

## UI - PNG (11)

| No | Numbered file | Source path | Loader | Renderer | JSON |
|----|---------------|-------------|--------|----------|------|
| 34 | 34_pantograph_sheet.png | assetdb/ui/hud/pantograph_sheet.png | UiAssetDB.get_ui_texture:68 id=pantograph_sheet | KeycapSlotView AtlasTexture 8 slots:250, KeycapFlightPresenter:96 | 10 |
| 35 | 35_hud_icons_128_atlas.png | assetdb/ui/hud/hud_icons_128_atlas.png | UiAssetDB.get_hud_texture:51 id=hud_icons_128_atlas | SegmentedGaugeBar._get_icon_texture:85, SystemStatusConsts.get_icon_texture:40 | 09 |
| 36 | 36_keycap_screen_9slice.png | assetdb/ui/hud/keycap_screen_9slice.png | UiAssetDB.get_hud_texture:51 | NeonStatusBadge._apply_screen_style:30, SegmentedGaugeBar._apply_bar_style:101 | 09 |
| 37 | 37_scanline_tile_fine.png | assetdb/ui/hud/scanline_tile_fine.png | UiAssetDB.get_hud_texture:51 | HudStyleBuilder.add_scanline_overlay:36 STRETCH_TILE alpha 0.08 | 09 |
| 38 | 38_keyboard_plate_block.png | assetdb/ui/hud/keyboard_plate_block.png | UiAssetDB.get_hud_texture:51 via make_frame_style | WordProgressPanel:36, PlayerStatusPanel:46, SystemStatusPanel:59 | 09 |
| 39 | 39_floating_visor_window.png | assetdb/ui/hud/floating_visor_window.png | UiAssetDB.get_hud_texture:51 via make_frame_style | MessageFeedPanel:49, MinimapPanel:45 | 09 |
| 40 | 40_title_weapons_atlas.png | assetdb/ui/title/title_weapons_atlas.png | UiAssetDB.get_ui_texture:68 id=title_weapons_atlas | TitleWeaponScroller:86 | - |
| 41 | 41_title_diorama_sword_body.png | assetdb/ui/title/title_diorama_sword_body.png | UiAssetDB.get_ui_texture:68 id=title_diorama_sword_body | LBlade:45 | - |
| 42 | 42_title_logo_text_our.png | assetdb/ui/title/title_logo_text_our.png | UiAssetDB.get_ui_texture:68 id=title_logo_text_our | TitleLogo._make_asset:110 RowTop | - |
| 43 | 43_title_logo_text_or.png | assetdb/ui/title/title_logo_text_or.png | UiAssetDB.get_ui_texture:68 id=title_logo_text_or | TitleLogo._make_asset:110 RowBottom | - |
| 44 | 44_title_logo_text_d.png | assetdb/ui/title/title_logo_text_d.png | UiAssetDB.get_ui_texture:68 id=title_logo_text_d | TitleLogo._make_asset:110 RowBottom | - |

## DRAWING PIPELINE

1. AssetDB cache (SSOT):
   assetdb/asset_db.gd:5-8 exposes global/entity/world/ui.
   BaseAssetDB._load_cached_json:7 and _load_cached_texture:15 give O(1) cache.
   Domain fence: GlobalAssetDB res://assetdb/global/, Entity res://assetdb/entity/,
   World res://assetdb/world/, Ui res://assetdb/ui/. No cross-domain paths.

2. Spec sync (JSON -> typed DTO):
   KeybotCompositeSpec.sync_from_single_dict + KeybotVisualSpec.sync_from_dict:14
   resolves signature_color_hex and sheet via EntityAssetDB.get_enemy_sheet.
   PlayerVisualSpec.sync_from_dict:20 and WeaponVisualSpec.sync_from_dict:12
   follow the same pattern. World catalogs parse layouts/pools/profiles
   without extra DTO. UI panels/pantograph stay as Dictionary.

3. Puppet entry (wiring interface, 1 hop, no wrapper):
   PuppetFactory.create:7 validates kind/profile via get_visual_registry/has_visual,
   then calls set_visual_profile once. Actor = Actuator x Puppet.
   Actuator: features/entity/*, features/world/* (stats, FSM, timers).
   Puppet: presentation/* (passive view, tween only).

4. Concrete drawing nodes:
   - Player/Keybot: LayeredPuppet._setup_layer_textures dispatches one sheet
     to 4 layers (body/visor/emoticon/accessory). Each layer slices AtlasTexture
     regions. Shaders: spine_energy_flow, visor_edge_glow on Sprite2D material.
     Motion: feed_motion -> bob/tilt/facing, trigger_action -> IDLE/WALK/ATTACK/
     HURT/DEAD, set_visual_profile -> sheet swap.
   - Weapon: ProceduralWeaponAssembler.assemble:11 loads albedo + mask +
     weapon_simple_neon shader, builds Sprite2D + ShaderMaterial(mask_texture)
     + Line2D bow wire, or fallback Polygon2D + Line2D rim. WeaponPuppet
     trigger_action:26 plays swing tween + CPUParticles2D spark.
   - World: RoomTilemapPuppet.setup_tileset:9 attaches TileMapLayer with
     TileSet from TilesetBuilder (room 4 cols, lobby 3 cols), then
     render_floor/render_sparse_layout set_cell atlas coords. PortalPuppet
     swaps unpressed/pressed Texture2D on one CapSprite + burst scale tween.
   - UI: TextureRect + AtlasTexture sub-region (pantograph 8 slots 128px,
     hud_icons 128px grid), StyleBoxTexture frames (make_frame_style),
     NinePatchRect fused keycap (32px margins), scanline TextureRect TILE mode,
     Label with mono font (UiRenderConsts.apply_mono_font). Shaders:
     ui_pantograph_visor_shine, ui_cyber_chamfer_panel, ui_cyber_visor_panel
     via UiThemeResolver/AssetDB.ui.get_hud_shader.

5. Call order example (enemy spawn):
   Keybot.apply_spec(AssetDB.entity.get_enemy_spec) ->
   PuppetFactory.create(enemy, visual_id) ->
   KeybotPuppet.set_visual_profile -> load_profile -> setup_from_sheet ->
   body/visor/emoticon/accessory.setup_textures(sheet) ->
   feed_motion per frame, trigger_action on combat events.

## EXCLUDED - VERIFIED UNUSED (NOT COPIED)

JSON (no runtime AssetDB getter call found):
assetdb/entity/enemy/C-001.json, R-003.json, T-002.json,
base_debris.json, base_keycap_1u.json, base_visor_wrap.json,
enemy_B-001_sheet.json, enemy_C-001_sheet.json, enemy_R-003_sheet.json,
enemy_T-002_sheet.json, enemy_shadow_recipe.json,
assetdb/entity/player/base_chassis_gem.json, base_chassis_trackball.json,
base_visor_shield.json, emoticons_cyber_neon.json, player_mouse.json,
player_schema.json, wheel_recipe.json,
assetdb/entity/weapon/weapon_sword.json, weapon_spear.json, weapon_bow.json,
assetdb/world/stage/room_recipes.json, portal_profiles.json,
assetdb/world/object/keycap_visual_profiles.json,
assetdb/world/object/portal_keycap_master.json,
assetdb/ui/data/*.json except pantograph (book, chest, cross_mount,
diamond_gem, gear, heart, hud_icons_128, kill_skull, lightning, shield,
speaker, stopwatch, title_diorama_sword_body, ui_panels duplicate),
assetdb/ui/data/manifests/hud_composite_surfaces.json,
assetdb/core/*.json + assetdb/global/data/*.json (data layer, not drawing).

PNG (no runtime texture load found):
assetdb/entity/weapon/sword.png, bow.png, spear.png (getter uses *_albedo.png),
assetdb/entity/enemy/debris_1.png, debris_2.png, debris_3.png
(getter uses keycap_debris.png),
assetdb/entity/enemy/enemy_keybot_*_sheet.png (legacy fallback, clean kb_* hit first),
assetdb/entity/player/mouse_hero.png, hero_classic_gem.png,
hero_trackball_cyber.png (getter uses *_sheet.png),
assetdb/entity/player/hero_*_sheet vs non-sheet duplicates above,
assetdb/ui/hud/hud_icons_128_mask.png, chip_particle_atlas.png,
switch_socket_well.png, chapter_title_well.png (constant defined, no call site).

## VERIFY

Count: 10 JSON + 34 PNG = 44 files in asset_tidy/numbered/.
Check: dir asset_tidy/numbered, file asset_tidy/ACTIVE_ASSET_LIST.md.
All source paths exist under assetdb/. Numbered copies are byte copies.

## RECIPE EXTENSION (see RECIPE_MAP.md)

Regeneration index: asset_tidy/RECIPE_MAP.md (34 rows, ASCII).
Flat pairs: asset_tidy/target/ (85 files: 34 png + 42 recipe + 9 note).
Per-asset cards: asset_tidy/mapping/ (34 txt).
Tool copies: asset_tidy/tools/ (22 py, verbatim).
JSON-less 8: U19 U26 U27 U37 U40 U42 U43 U44. Unwired 1: U15.
