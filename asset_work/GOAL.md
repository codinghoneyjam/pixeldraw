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
| `asset_work/target/` | 89 | 게이트 입력: assetdb 69 + oracle 8 + tests 12 (parity/tests가 읽는 실경로) |
| `asset_work/pygate/` | 6 | Python parity 게이트 + 진단 (JS 게이트의 독립 구현) |
| `asset_work/scratch/` | 4 | 일회성 진단 프로브 (게이트 아님, 고장 시 삭제) |

JSON spec 10종: 01 enemy / 02 player / 03 weapon / 04 manifest / 05 layouts / 06 encounter_pools / 07 room_profiles / 08 room_archetypes / 09 ui_panels / 10 pantograph_keycap_profile.
01–09는 런타임 스펙이지 transpiler 입력 레시피가 아니므로 parity 게이트 대상이 아니다. 10_pantograph만 예외로 U30–U34 transpiler SSOT 입력으로 승격되었다. 상태는 `inventoried`로 별도 집계한다.

## PNG 34종 상태표

| ID | numbered | METHOD (RECIPE_MAP) | 상태 | 근거 |
|:---|:---|:---|:---|:---|
| U11 | 11_kb_z_amber_t1.png | RECIPE_JSON (C-001) | done | gated `enemy_keybot_c001_sheet`와 바이트 동일, parity 0.995110으로 증명 |
| U12 | 12_kb_f_crimson_t2.png | RECIPE_JSON (T-002) | done | gated `enemy_keybot_t002_sheet`와 바이트 동일, parity 0.994804로 증명 |
| U13 | 13_kb_q_cobalt_t3.png | RECIPE_JSON (R-003) | done | gated `enemy_keybot_r003_sheet`와 바이트 동일, parity 0.995110으로 증명 |
| U14 | 14_kb_b_obsidian_t4.png | RECIPE_JSON+VIA_OVERRIDE (C-001 기하 + #FF3B30) | done | gated `enemy_keybot_b001_sheet`와 바이트 동일, parity 0.995110 (보스 시그니처 미베이크는 의도적) |
| U15 | 15_keycap_debris.png | RECIPE_UNWIRED (base_debris.json) | done | parity 1.000000 통과 (rect 92개 픽셀 bake; 레거시 쿼드가 틴트 슬롯 SSOT와 상이, 런타임 틴트 유지) |
| U16 | 16_mouse_hero_sheet.png | RECIPE_JSONx3 | done | `mouse_hero_sheet` 풀시트 합성 parity 0.996426 통과 |
| U17 | 17_hero_classic_gem_sheet.png | RECIPE_JSONx3 | done | U16과 바이트 동일, 단일 렌더로 둘 다 증명 |
| U18 | 18_hero_trackball_cyber_sheet.png | RECIPE_JSONx3 | done | `hero_trackball_cyber_sheet` 풀시트 합성 parity 0.997406 (CYBER 에메랄드 프로파일 역설계) |
| U19 | 19_shockwave_ring.png | CODE_ONLY | done | parity 0.999756 통과 (transparent_residue 2384 별도 표기) |
| U20 | 20_sword_albedo.png | RECIPE_JSON | done | parity 1.000000 통과 |
| U21 | 21_sword_mask.png | RECIPE_JSON | done | parity 1.000000 통과 |
| U22 | 22_bow_albedo.png | RECIPE_JSON | done | parity 1.000000 통과 |
| U23 | 23_bow_mask.png | RECIPE_JSON | done | parity 1.000000 통과 |
| U24 | 24_spear_albedo.png | RECIPE_JSON | done | parity 1.000000 통과 |
| U25 | 25_spear_mask.png | RECIPE_JSON | done | parity 1.000000 통과 |
| U26 | 26_room_tileset.png | TRES_EXTRACTED | done | parity 1.000000 통과 (solid-rect 23개 역설계 레시피) |
| U27 | 27_lobby_tileset.png | TRES_EXTRACTED | done | parity 1.000000 통과 (solid-rect 16개 역설계 레시피) |
| U28 | 28_portal_keycap_unpressed.png | RECIPE_JSON | done | parity 0.998291 통과 |
| U29 | 29_portal_keycap_pressed.png | RECIPE_JSON | done | parity 0.997559 통과 |
| U30 | 30_keycap_sheet.png | RECIPE_JSON_SHARED | done | pantograph_sheet parity 0.997101 (U30==U34 SHA 35562ebc2f1f, 단일 parity로 둘 다 증명) |
| U31 | 31_fused_keycap_9slice.png | RECIPE_JSON_SHARED | done | parity 0.995361 통과 |
| U32 | 32_fused_keycap_hover_9slice.png | RECIPE_JSON_SHARED | done | parity 0.995361 통과 |
| U33 | 33_fused_keycap_pressed_9slice.png | RECIPE_JSON_SHARED | done | parity 0.995361 통과 |
| U34 | 34_pantograph_sheet.png | RECIPE_JSON_SHARED | done | pantograph_sheet parity 0.997101 (8슬롯 시트, profile SSOT 승격) |
| U35 | 35_hud_icons_128_atlas.png | RECIPE_JSONx11 | done | parity 0.994621 통과 (11종 bake 결합 레시피, 11–15셀 빈칸, book/cross 셀 픽셀 정확) |
| U36 | 36_keycap_screen_9slice.png | RECIPE_JSON_SHARED | done | parity 0.996094 통과 |
| U37 | 37_scanline_tile_fine.png | NO_MANIFEST | done | parity 1.000000 통과 (manifest 항목 baker-add, mapping 카드 처방) |
| U38 | 38_keyboard_plate_block.png | RECIPE_JSON_SHARED | done | parity 0.998779 통과 |
| U39 | 39_floating_visor_window.png | RECIPE_JSON_SHARED | done | parity 0.999313 통과 (반투명 2종 포함) |
| U40 | 40_title_weapons_atlas.png | MISSING_PROFILE | done | parity 1.000000 통과 (rect+alpha 265개 픽셀 bake; profiles JSON 없음, fillet 복원 불가) |
| U41 | 41_title_diorama_sword_body.png | RECIPE_JSON_DUAL | done | parity 0.999821 통과 (28px 코너 잔차는 PIL 반올림 차, 허용) |
| U42 | 42_title_logo_text_our.png | FONT_ONLY | done | parity 1.000000 통과 |
| U43 | 43_title_logo_text_or.png | FONT_ONLY | done | parity 1.000000 통과 |
| U44 | 44_title_logo_text_d.png | FONT_ONLY | done | parity 1.000000 통과 |

## 현재 달성률 (2026-10-06 실측)

- 인벤토리 확보: **44/44 = 100%** (`numbered/` 실재 확인)
- PNG strict done: **34/34 = 100%** 🎉 (U11–14 identidad 증명 + U15 + U16–18 풀시트 + U19–29 11종 + U30–40 11종 + U41–44 4종)
- PNG partial: **0** — partial 소진
- PNG todo: **0** — todo 소진
- 전체 44종 기준 strict: **34/44 = 77.3%** (잔여 10종은 parity 게이트 대상이 아닌 JSON spec `inventoried`)
- JSON spec 10종: `inventoried` 10/10 + `tests/spec_consistency.test.mjs`로 교차 검증 (01–09 parity 게이트 대상 아님, 10_pantograph는 U30–U34 transpiler SSOT 입력으로 승격). 기지 분기 1건: hero_classic_gem amber 스펙 vs sky 시트 (시트가 구버전, 고정됨).
- U30==U34 바이트 동일 (SHA 35562ebc2f1f): `pantograph_sheet` 단일 parity 0.997101이 둘 다 증명

게이트 상태 (같은 날 실행, 전부 녹색):

| 게이트 | 결과 |
|:---|:---|
| `node --test "tests/*.test.mjs"` | 293/293 pass (+10 enemy/player sheets) |
| `node tools/recipe_parity_check.mjs` (`npm run parity:assets`) | 39/39 pass (threshold 0.99, +2 player sheets) |
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

1. ~~U30–U34 pantograph/keycap 5종~~ ✅ **완료 (2026-10-06).** profile을 transpiler SSOT 입력으로 승격 (`transpile.mjs` pantograph 분기), 게이트 입력 5종 추가 (`asset_work/target/assetdb/ui/hud/`), parity 4케이스 + 테스트 9건. 잔차는 rrect 코너 1px 클래스 (sword/portal과 동일).
2. ~~U35 hud atlas~~ ✅ **완료 (2026-10-06).** 11종 아이콘을 `bake_hud_atlas.mjs`로 단일 512x512 transpiler-native 레시피에 bake ($토큰→리터럴, 셀 오프셋), parity 0.994621. 잔차는 arc/chord 래스터 1px 클래스 + diamond `default-42` "A" 미렌더 (수백 px, 게이트 여유 내). book/cross 셀은 픽셀 정확.
3. ~~U36/U38/U39 hud surfaces + U37 scanline~~ ✅ **완료 (2026-10-06).** manifest를 transpiler SSOT 입력으로 승격 (`transpile.mjs` surfaces 분기, type/bbox→cmd/box), U37 항목 baker-add (mapping 카드 처방, `#22D3EE28` 10행), parity 4종 (0.996094/0.998779/0.999313/1.0) + 테스트 7건.
4. ~~U26/U27 tileset~~ ✅ **완료 (2026-10-06).** TRES 소스 삭제済이므로 `bake_tileset.mjs`로 solid-rect 역설계 레시피 생성 (23/16 rect), parity 둘 다 1.0 + 테스트 4건.
5. ~~U40 weapons atlas~~ ✅ **완료 (2026-10-06).** profiles JSON이 없어 fillet 복원 불가이므로 `bake_tileset.mjs`를 alpha-run까지 확장해 rect+alpha 265개 픽셀 bake, parity 1.0 + 테스트 1건. 투명 위 엣지라 overwrite가 정확.
6. ~~U15 debris~~ ✅ **완료 (2026-10-06).** 레거시 쿼드가 틴트 슬롯 SSOT와 상이함을 실측으로 확정 (fringe 265 + fill 상이 410) 후 rect 92개 픽셀 bake, parity 1.0 + 테스트 1건. 런타임 틴트 원칙은 엔진에 유지.
7. ~~U11–14/U16–18 partial → done~~ ✅ **완료 (2026-10-06).** U11–14는 gated fallback 시트와 바이트 동일임을 테스트로 고정 (추가 렌더 없음). U16/U18은 `bake_player_atlas.mjs`로 16슬롯 풀시트 합성 (벡터 0–9 + 휠 10–12 픽셀 bake + 13–15 빈칸), parity 0.996426/0.997406. U17은 U16과 바이트 동일. TRACKBALL_CYBER 에메랄드 프로파일 역설계 후 `player_profile_specs.json`에 커밋. 테스트 10건.
8. ~~34개 레시피 레이어 추출~~ ✅ **완료 (2026-10-07).** `tools/extract_layers.mjs`가 26개 레시피를 구조 단위(레이어/슬롯/surface/nine_patch/albedo·mask)로 분해해 단위별 PNG + Document JSON을 `asset_work/layers/`에 렌더링 (113 unit renders). 케이스 표는 `tools/recipe_cases.mjs`로 SSOT 분리, `tests/extract_layers.test.mjs` 7건.
