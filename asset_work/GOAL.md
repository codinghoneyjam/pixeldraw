# ASSET_TIDY 목표 — numbered 44종 전부 구현

- 기준일: 2026-10-06
- 타이디 폴더: `asset_work/tidy/` (SSOT 인벤토리는 `asset_work/tidy/ACTIVE_ASSET_LIST.md`, 재생성 인덱스는 `asset_work/tidy/RECIPE_MAP.md`)
- 목표 md 본 파일이 규범 (`asset_work/GOAL.md`). 기계가 읽는 체크 파일: `asset_work/progress.json` (본 파일과 동시 갱신)
- 범위 확정: `asset_work/tidy/numbered/` 44개 (`01_`~`44_`)만 구현 대상. `asset_work/tidy/target/`(85)・`mapping/`(34)・`tools/`(22)・`unused/`(17)・`gallery.html`은 증거/참조용, 구현 대상 아님. 게이트 입력은 `asset_work/target/` (별도).

## 완료 정의 (Done)

1. `tools/recipe/transpile.mjs` `transpileAndRender()` 로 해당 PNG를 렌더링한다.
2. legacy PNG 대비 RGBA `match_fraction >= 0.99` (`tools/recipe_parity_check.mjs`와 동일 비교, 양쪽 투명 픽셀은 동점으로 간주).
3. 산출 문서 JSON은 32px 청크 정렬을 유지한다 (viewport/crop 경로는 `tests/recipe_sword.test.mjs` 규약 준수).
4. 상태는 `asset_work/progress.json`의 `assets[]`에 `done/partial/todo` + `evidence`로 기록한다.

`partial` = 레이어 단위 또는 구형 fallback 시트까지만 parity 증명됨 (풀시트 합성 미증명).
`todo` = parity 케이스 없음 / 레시피 없음 / 소스 삭제 / 미배선.

## 범위 인벤토리 (2026-10-06 실측)

| 위치 | 개수 | 내용 |
|:---|:---:|:---|
| `asset_work/tidy/numbered/` | 44 | 01–10 JSON spec 10종 + 11–44 PNG 34종 |
| `asset_work/tidy/target/` | 85 | U PNG 34 + recipe 42 + note 9 (평면 페어) |
| `asset_work/tidy/mapping/` | 34 | U11–U44 카드 (`U<NN>.txt`) |
| `asset_work/tidy/tools/` | 22 | 생성기 + v2/engine + migrate + core lib verbatim 복사 |
| `asset_work/tidy/unused/` | 17 | A5+B4+C6+D2, 삭제 후보 (구현 대상 아님) |
| `asset_work/target/` | 53 | 게이트 입력: assetdb 33 + oracle + tests (parity/tests가 읽는 실경로) |
| `asset_work/pygate/` | 6 | Python parity 게이트 + 진단 (JS 게이트의 독립 구현) |
| `asset_work/scratch/` | 4 | 일회성 진단 프로브 (게이트 아님, 고장 시 삭제) |

JSON spec 10종: 01 enemy / 02 player / 03 weapon / 04 manifest / 05 layouts / 06 encounter_pools / 07 room_profiles / 08 room_archetypes / 09 ui_panels / 10 pantograph_keycap_profile.
이 10종은 런타임 스펙이지 transpiler 입력 레시피가 아니므로 parity 게이트 대상이 아니다. 상태는 `inventoried`로 별도 집계한다.

## PNG 34종 상태표

| ID | numbered | METHOD (RECIPE_MAP) | 상태 | 근거 |
|:---|:---|:---|:---|:---|
| U11 | 11_kb_z_amber_t1.png | RECIPE_JSON (C-001) | partial | fallback `enemy_keybot_c001_sheet`만 parity 0.995110 통과, 모던 `kb_*` 풀시트 미증명 |
| U12 | 12_kb_f_crimson_t2.png | RECIPE_JSON (T-002) | partial | fallback `enemy_keybot_t002_sheet`만 0.994804 통과 |
| U13 | 13_kb_q_cobalt_t3.png | RECIPE_JSON (R-003) | partial | fallback `enemy_keybot_r003_sheet`만 0.995110 통과 |
| U14 | 14_kb_b_obsidian_t4.png | RECIPE_JSON+VIA_OVERRIDE (C-001 기하 + #FF3B30) | partial | fallback `enemy_keybot_b001_sheet`만 0.995110 통과, 보스 시그니처 미베이크 (의도적) |
| U15 | 15_keycap_debris.png | RECIPE_UNWIRED (base_debris.json) | todo | generator caller 없음, parity 없음 |
| U16 | 16_mouse_hero_sheet.png | RECIPE_JSONx3 | partial | shadow/visor 레이어만 parity (0.996033/0.998657), 풀시트 합성 미증명 |
| U17 | 17_hero_classic_gem_sheet.png | RECIPE_JSONx3 | partial | shadow 0.997192까지만 증명, 풀시트 미증명 |
| U18 | 18_hero_trackball_cyber_sheet.png | RECIPE_JSONx3 | partial | shadow/body/face_normal 레이어만 증명 (0.997192/0.997314/0.999512), 풀시트 미증명 |
| U19 | 19_shockwave_ring.png | CODE_ONLY | done | parity 0.999756 통과 (transparent_residue 2384 별도 표기) |
| U20 | 20_sword_albedo.png | RECIPE_JSON | done | parity 1.000000 통과 |
| U21 | 21_sword_mask.png | RECIPE_JSON | done | parity 1.000000 통과 |
| U22 | 22_bow_albedo.png | RECIPE_JSON | done | parity 1.000000 통과 |
| U23 | 23_bow_mask.png | RECIPE_JSON | done | parity 1.000000 통과 |
| U24 | 24_spear_albedo.png | RECIPE_JSON | done | parity 1.000000 통과 |
| U25 | 25_spear_mask.png | RECIPE_JSON | done | parity 1.000000 통과 |
| U26 | 26_room_tileset.png | TRES_EXTRACTED | todo | 소스 삭제됨, 신규 JSON 레시피 필요, parity 없음 |
| U27 | 27_lobby_tileset.png | TRES_EXTRACTED | todo | 동상 |
| U28 | 28_portal_keycap_unpressed.png | RECIPE_JSON | done | parity 0.998291 통과 |
| U29 | 29_portal_keycap_pressed.png | RECIPE_JSON | done | parity 0.997559 통과 |
| U30 | 30_keycap_sheet.png | RECIPE_JSON_SHARED | todo | parity 케이스 없음 |
| U31 | 31_fused_keycap_9slice.png | RECIPE_JSON_SHARED | todo | 동상 |
| U32 | 32_fused_keycap_hover_9slice.png | RECIPE_JSON_SHARED | todo | 동상 |
| U33 | 33_fused_keycap_pressed_9slice.png | RECIPE_JSON_SHARED | todo | 동상 |
| U34 | 34_pantograph_sheet.png | RECIPE_JSON_SHARED | todo | 동상 |
| U35 | 35_hud_icons_128_atlas.png | RECIPE_JSONx11 | todo | parity 케이스 없음 (`asset_work/target/`에 diamond_gem.json만 존재) |
| U36 | 36_keycap_screen_9slice.png | RECIPE_JSON_SHARED | todo | parity 없음 |
| U37 | 37_scanline_tile_fine.png | NO_MANIFEST | todo | manifest 항목 없음, parity 없음 |
| U38 | 38_keyboard_plate_block.png | RECIPE_JSON_SHARED | todo | parity 없음 |
| U39 | 39_floating_visor_window.png | RECIPE_JSON_SHARED | todo | parity 없음 |
| U40 | 40_title_weapons_atlas.png | MISSING_PROFILE | todo | profiles JSON 없음, parity 없음 |
| U41 | 41_title_diorama_sword_body.png | RECIPE_JSON_DUAL | done | parity 0.999821 통과 (28px 코너 잔차는 PIL 반올림 차, 허용) |
| U42 | 42_title_logo_text_our.png | FONT_ONLY | done | parity 1.000000 통과 |
| U43 | 43_title_logo_text_or.png | FONT_ONLY | done | parity 1.000000 통과 |
| U44 | 44_title_logo_text_d.png | FONT_ONLY | done | parity 1.000000 통과 |

## 현재 달성률 (2026-10-06 실측)

- 인벤토리 확보: **44/44 = 100%** (`numbered/` 실재 확인)
- PNG strict done: **13/34 = 38.2%** (U19–25 7종 + U28–29 2종 + U41–44 4종)
- PNG partial 포함: **20/34 = 58.8%** (done 13 + partial U11–14, U16–18 7종)
- PNG todo: **14/34 = 41.2%** (U15, U26–27, U30–40)
- 전체 44종 기준 strict: **13/44 = 29.5%**, partial 포함 **20/44 = 45.5%**
- JSON spec 10종: `inventoried` 10/10, parity 게이트 대상 아님

게이트 상태 (같은 날 실행, 전부 녹색):

| 게이트 | 결과 |
|:---|:---|
| `node --test "tests/*.test.mjs"` | 255/255 pass |
| `node tools/recipe_parity_check.mjs` (`npm run parity:assets`) | 24/24 pass (threshold 0.99) |
| `node tools/parity_check.mjs` | 201 golden checks pass |
| `node tools/png_cases_check.mjs` | 5/5 |
| `node tools/view_check.mjs` | OK |
| `python schema/schema_selfcheck.py` | OK |

## Agent 체크 절차 (게이트 변경 없이 이 순서로)

```bash
node --test "tests/*.test.mjs"
node tools/recipe_parity_check.mjs
node tools/parity_check.mjs
node tools/png_cases_check.mjs
node tools/view_check.mjs
python schema/schema_selfcheck.py
```

ovu 체크 후 `asset_work/progress.json`의 `last_checked`・`gates`・`summary`를 갱신하고, 상태가 바뀐 asset만 `assets[].status/evidence`를 고친다. 본 md의 상태표와 달성률은 JSON에서 그대로 옮긴다 (두 파일 불일치 금지).

## 다음 작업 (우선순위)

1. U30–U34 pantograph/keycap 5종: `pantograph_keycap_profile.json` 공유 레시피를 transpiler 입력으로 승격 + parity 케이스 추가 (RECIPE_JSON_SHARED 해소).
2. U35 hud atlas: 11종 레시피 확보 후 parity (RECIPE_JSONx11).
3. U36/U38/U39 hud surfaces: `hud_composite_surfaces.json` 경로 parity 추가.
4. U37 scanline: manifest 항목 추가 후 parity (NO_MANIFEST 해소).
5. U26/U27 tileset: 신규 JSON 레시피 작성 (TRES 소스 삭제됨이므로 역설계 필요).
6. U40 weapons atlas: profiles JSON 확보 (MISSING_PROFILE 해소).
7. U15 debris: `base_debris.json` 배선 결정 (UNWIRED 해소, 런타임 틴트 원칙 유지).
8. U11–14/U16–18 partial → done: 모던 `kb_*` 시트 및 플레이어 풀시트 합성 parity로 승격.
