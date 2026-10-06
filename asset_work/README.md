# asset_work — 에셋 작업 폴더 (단일 진입점)

pixeldraw repo의 에셋 작업물을 한 폴더에 모은다. 규범 문서는 `GOAL.md`,
기계 체크용은 `progress.json`이다 (두 파일 동시 갱신, 불일치 금지).

| 경로 | 내용 | 추적 |
|:---|:---|:---|
| `tidy/` | 원본 증거 (구 `asset_tidy/`): numbered 44 + target 85 + mapping 34 + tools 22 + unused 17 + 인벤토리 md + gallery | 읽기전용 취급 (이력 보존, 경로만 변경) |
| `target/` | 게이트 입력 (구 루트 `target/`): assetdb + oracle + tests. `tools/recipe_parity_check.mjs`·`tests/*.test.mjs`·`pygate/`가 읽는 실경로 | 이동 시 TARGET_ROOT 동반 수정 필수 |
| `pygate/` | Python parity 게이트 + 진단 (구 `tools/pygate/`). 실행: `python asset_work/pygate/legacy_vs_drawtool.py` | JS 게이트의 독립 구현 (8/24 케이스) |
| `scratch/` | 일회성 진단 프로브 (구 `tools/diag*.mjs/json`). 게이트 아님. 고장 시 삭제, 수리 금지 | 소유자 없음 |
| `GOAL.md` | 목표·완료정의·34종 상태표·달성률·다음작업 | 규범 |
| `progress.json` | per-asset status + gates + check_commands | agent 체크용 |

게임 repo 문서는 링크만 유지한다 (파일 이동 없음):
`../your-wor(l)d/docs/50_reports/unreproducible_assets_resolution_plan.md`
(R-1~R-8 / T-1~T-7 / S-1~S-8 실행계획 원본),
`../your-wor(l)d/docs/50_reports/parity_sheets/` (baseline·step_t4b 무기/포탈 시트).
