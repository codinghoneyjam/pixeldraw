# Pixeldraw 개선 실행 계획

> 작성일: 2026-10-24
> 기준: `docs/port_status_report.md` (2026-10-18) + `docs/progress/log.md` 최신 항목
> 목적: 개선 과제를 **실행 가능한 단위**로 쪼개고, 완료 기준과 검증 명령을 고정한다.
> 원칙: 코드가 문서보다 우선. 충돌 시 코드를 읽고 문서를 고친 뒤 진행.

---

## 0. 현재 상태 (2026-10-27 기준)

| 항목 | 값 |
|---|---|
| `cargo test --workspace` | **651 pass / 0 fail** (atlas +45) |
| clippy / fmt | 그린 (2026-10-27) |
| npm test | 410 / 410 |
| golden | 201 / 201 · recipe 39 / 39 · png 5 / 5 |
| Phase 0~4 | ✅ 완료 |
| Phase 5(성능) | ⚠️ 부분 — 달성 6 / 미달 4 / 미측정 0 |
| Phase 7.5 | 🔄 수동 스모크 진행 중 |
| Phase 6(패키징) | ⬜ 미착수 |
| feature_expansion | M0 대부분 ✅ · **F01 🟡** (남은 작업: `docs/atlas_plan.md`) |

**남은 큰 덩어리 4개:**
1. (P0) Phase 7.5 수동 스모크 + Phase 2·3 체크리스트 (사람 확인)
2. (P1) 성능 미달 4항목 — composite / flattenToRgba / pen=8 / documentToJson
3. (P1/P2) 파일 UX 잔손질 + 패널 폭 영속화
4. (P2) 문서 정합성 정리 → Phase 6 착수
5. (별도 트랙) feature_expansion M1 — F01 🟡 (남은 작업 `docs/atlas_plan.md`), F02~F04 ⏳

---

## 1. 실행 단위 (Wave 0 — 즉시, 저위험)

코드 변경 최소, 회귀 위험 없음. 먼저 끝낸다.

### W0-1. 캔버스 뷰 dead code 제거
- **파일**: `crates/app/src/canvas_view.rs`
- **변경**: `dispatch_frame_input` 내 `let cursor = state.tools.cursor(); let _ = cursor;` 2줄 삭제 (결과 미사용).
- **완료 기준**: clippy `-D warnings` 그린, app 테스트 그린.
- **상태**: ✅ 완료 (2026-10-24)

### W0-2. `drain_events` 무제한 drain 캡 (V3)
- **파일**: `crates/app/src/session_glue.rs`
- **변경**: 한 프레임 최대 1024개 이벤트만 drain, 초과 시 `eprintln!` 경고 + 잔여는 다음 프레임으로 이월.
- **완료 기준**: 기존 테스트(`drain_events` 사용처) 전부 그린, 캡 동작 단위 테스트 1건 추가.
- **상태**: ✅ 완료 (2026-10-24)

### W0-3. 문서 보드 stale 갱신 (D1)
- **파일**: `docs/progress/README.md`
- **변경**: 누적 게이트 321 → 606, Phase 2/3 체크박스를 코드 완료 상태로 갱신, 커밋 이력에 10-18 항목 추가.
- **완료 기준**: 보드 수치가 실측과 일치.
- **상태**: ✅ 완료 (2026-10-24)

### W0-4. `native_plan.md` 상태줄 갱신 (D2)
- **파일**: `docs/native_plan.md`
- **변경**: "Phase 1 완료" 고정 상태줄을 현재 Phase 진행도로 갱신.
- **상태**: ⏳ (Phase 7.5 종료 시 일괄)

---

## 2. Wave 1 — P0 게이트 닫기 (사람 확인)

### W1-1. Phase 7.5 수동 스모크 (UI-6 / V2)
- **대상**: `docs/native_smoke.md` 체크리스트 전 항목.
- **절차**: release exe(`target/release/pixeldraw.exe`)로 1280×820 / 900×560에서 1회 실행, 실패 항목만 픽스, 실행 기록 채움.
- **완료 기준**: 체크리스트 전 항목 `[x]`, 실패 항목 0 또는 이슈화.
- **차단**: 이 작업이 끝나야 Phase 2·3·7 공식 게이트가 닫힌다.
- **상태**: ⏳ (사람 확인 필요 — 에이전트는 실행/픽스만, 최종 확인은 사용자)

---

## 3. Wave 2 — 성능 잔여 (P1)

### W2-1. composite / flattenToRgba 미달 해소 (PERF-2)
- **파일**: `crates/engine/src/composite.rs`, `crates/engine/src/display/frame.rs`(blit), `blend.rs`
- **변경**: 5.1 u32 워드 SIMD + 로드 단위 스킵 확장. `over_blend` f64 연산 → u32 워드 연산.
- **목표**: composite ≤ 2 ms, flattenToRgba ≤ 3 ms (현재 14.27 / 14.70).
- **완료 기준**: framebench A/B 측정치 기록, golden/hash parity 불변.
- **상태**: ⏳

### W2-2. documentToJson 미달 해소 (PERF-3 잔여)
- **파일**: `crates/engine/src/io/serialize.rs`
- **변경**: 청크당 할당 재사용 (per-thread `Compress` + borrowed chunks 이미 적용, 남은 Vec/base64/Value 할당 제거).
- **목표**: 578.9 ms → 120 ms.
- **완료 기준**: bench.rs 측정치 기록, PNG 바이트 동일성 테스트 그린.
- **상태**: ⏳

### W2-3. stroke pen=8 미달 해소 (PERF-2)
- **파일**: `crates/engine/src/raster/brush.rs`
- **변경**: 스팬 폭 임계, footprint 재사용.
- **목표**: 4.12 ms → 2 ms.
- **상태**: ⏳

### W2-4. 프레임 타이밍 HUD (PERF-4 5.7)
- **파일**: `crates/app/src/canvas_view.rs` 또는 신규 `hud.rs`
- **변경**: 개발 빌드에서 프레임 ms · dirty 청크 수 · 레이어 수 · comp 예산 초과 횟수 오버레이.
- **상태**: ⏳

---

## 4. Wave 3 — UI 잔손질 + 영속화 (P1/P2)

### W3-1. SaveAs / 최근 경로 분리 (UI-7)
- **파일**: `crates/app/src/shell.rs`, `crates/app/src/file_ops.rs`
- **변경**: `Action::SaveAs`가 `Save`와 동일 동작 → 별도 경로 선택 + 최근 디렉터리 기억.
- **상태**: ⏳

### W3-2. 패널 폭 영속화 (UI-4)
- **파일**: `crates/app/src/ui/settings_store.rs`, `crates/app/src/shell.rs`
- **변경**: `settings.json`에 `panelWidths` 병합 저장 (사용자가 명시 조절한 쪽만).
- **상태**: ⏳

### W3-3. strings 중앙화 잔여 (UI-7)
- **파일**: `crates/app/src/ui/shared/strings.rs` + 피처 모듈
- **변경**: 남은 라벨을 중앙화.
- **상태**: ⏳

---

## 5. Wave 4 — 문서 정합성 (P2)

### W4-1. `ui_ux_port_analysis.md` stale 표기 (D3)
- **파일**: `docs/ui_ux_port_analysis.md`
- **변경**: 상단에 "초기 분석·현 아키텍처(eframe/egui)와 불일치" 표기.

### W4-2. known_doc_code_discrepancies 정리 (D4)
- **파일**: `docs/README.md` 등
- **변경**: Phase 6.6 문서 재작성 때 일괄.

### W4-3. Phase 7 로그 기록 (D5)
- **파일**: `docs/progress/log.md`
- **변경**: 10-15 이후 Phase 7 작업 로그 초안 작성 (오케스트레이터 합류).

### W4-4. 미커밋 변경 정리 (D6)
- **파일**: 다수
- **변경**: 7.5 수동 확인 후 사용자 승인 하에 Phase 7 배치 커밋.

---

## 6. Wave 5 — Phase 6 패키징 (P2)

### W5-1. NSIS 설치자 (6.2)
- **결정 필요**: 별도 NSIS vs Tauri 캐시 `makensis.exe`, per-user/per-machine, `%APPDATA%/pixeldraw` 보존, 아이콘·버전 리소스.

### W5-2. 웹 잔재 삭제 (6.3)
- **대상**: `src/`, `index.html`, `style_*.css`, `run.py`, `src-tauri/`, `tools/{stage,bundle,check_stage}_*.mjs`.
- **유지**: `reference/`, `tests/fixtures/*`, `tools/oracle-manifest.json` (골든 원본).

### W5-3. 문서 재작성 (6.4~6.6)
- **대상**: `contract.md` §4~§5, README·desktop.md, core/render/model/io/tools/ui 문서를 크레이트 기준으로.

### W5-4. CI 게이트 (6.7)
- **대상**: `cargo test/clippy/fmt` + 오라클 (Windows 러너).

### W5-5. 폰트 번들 결정 (D3)
- **결정**: 시스템 폰트 의존 vs 번들 (라이선스·크기).

---

## 7. 별도 트랙 — feature_expansion (M0 → M1)

`docs/feature_expansion_plan.md` 기준. M0 기반 정리가 먼저.

### M0. 기반 정리 (선행 필수)
1. ActionRouter 단일화 (`crates/app/src/ui/action_router.rs`) — ✅ (2026-10-24, 모듈 생성 + menubar 경유; shell은 자체 복사본 유지 → 후속 통합)
2. DialogOutcome 도입 (취소 버그 수정) — ✅ (2026-10-24, `confirmed` 플래그로 취소/ESC 시 적용 차단)
3. 스키마 v2.1 마이그레이션 하네스 — ✅ (2026-10-24, SCHEMA_VERSION 2.1.0 + READABLE list + migrate_document/validate_supported_schema)
4. Overlay `MaskBlit` 구현 — ✅ (2026-10-24, `crates/app/src/tools/overlay.rs`)
5. CompositeCommand (다중 명령 1스텝) — ✅ (2026-10-24, `crates/engine/src/model/commands/composite.rs`)
6. ToolCx/Session 확장 원칙 — ✅ (2026-10-24, 관례 문서화; 실제 상태는 F03/F05/F10에서 Session에 추가)

**게이트 G0**: 기존 테스트 전부 그린 + v2.0.0 문서 그대로 열림.

### M1. P0 4개 (사용자 요청)
1. **F01 아틀라스 워크벤치** — 🟡 대부분 완료 (2026-10-27). 레이어/합성 시트·`dtAt` v2·GUI/CLI 왕복 동작. 남은 작업: `docs/atlas_plan.md` (레이아웃 영속화, 슬롯 복원, 슬라이스, 패딩/마진, 셀 편집).
2. **F02 시작 레이어 프리셋** — 6종 스타터 스택 + 기존 문서 적용
3. **F03 대칭 드로잉** — 수직/수평/사분면/방사형, 1 undo
4. **F04 도형 프리셋** — 하트, N각 별 (기존 폴리곤 경로)

**게이트 G1**: 4개 수용 기준 통과.

---

## 8. ui-storage-plan 9개 항목 (후속/병행)

`docs/ui-storage-plan.md` 기준. M0/M1과 병행·후속으로 흡수.

1. 좌측 패널 구획·접기·반응형·스크롤
2. 레이어 선택색/호버 맥동
3. 색상환 배치·입력 필드
4. SV 컬러픽커 포인터
5. 40색 팔레트/최근색 8칸
6. 2상태 토글
7. 다각형 정점 입력
8. 도구 전환 깜박임
9. 저장 포맷 3종 (읽기 쉬운 JSON / 명령 레이어 / 순수 청크)

---

## 9. 검증 명령 (모든 Wave 공통)

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
npm.cmd test
npm.cmd run parity
npm.cmd run parity:assets
node tools/png_cases_check.mjs
node tools/view_check.mjs
node tools/oracle_manifest.mjs --check
```

성능 변경 Wave는 추가로:
```powershell
cargo run -p pixel-engine --release --example bench
cargo run -p pixel-engine --release --example framebench
```

---

## 10. 진행 상태 요약

| Wave | 항목 | 상태 |
|---|---|---|
| 0 | W0-1 dead code 제거 | ✅ |
| 0 | W0-2 drain_events 캡 | ✅ |
| 0 | W0-3 보드 stale 갱신 | ✅ |
| 0 | W0-4 native_plan 상태줄 | ⏳ (Phase 7.5 종료 시) |
| 1 | W1-1 수동 스모크 | ⏳ (사람 확인) |
| 2 | W2-1~4 성능 잔여 | ⏳ |
| 3 | W3-1~3 UI 잔손질/영속화 | ⏳ |
| 4 | W4-1~4 문서 정합성 | ⏳ |
| 5 | W5-1~5 Phase 6 패키징 | ⏳ |
| 별도 | M0 기반 정리 | 🟡 (ActionRouter/DialogOutcome 후속 통합) |
| 별도 | M1 P0 4개 | 🟡 (F01 대부분 완료, F02~F04 ⏳) |

---

## 참고

- 계획/로그: `docs/native_plan.md`, `docs/progress/{README.md,log.md}`
- 성능: `docs/perf_js_vs_rust.md`, `docs/perf_optimization_candidates.md`
- UI: `docs/native_ui_layout_audit.md`, `docs/native_smoke.md`, `docs/ui_ux_port_analysis.md`
- 기능 확장: `docs/feature_expansion_plan.md`
- UI/저장 개선: `docs/ui-storage-plan.md`
- 포팅 현황·개선 과제: `docs/port_status_report.md`