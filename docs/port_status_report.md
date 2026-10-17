# Rust 포팅 현황과 개선 과제

> 조사일: **2026-10-17** · 기준: HEAD `72160fc` + Phase 7 미커밋 워킹트리
> 검증: 이 문서의 모든 게이트 수치는 2026-10-17에 저장소에서 직접 실행한 결과다(부록 A).
> 관련 문서: `docs/native_plan.md`(계획) · `docs/progress/log.md`(로그) ·
> `docs/perf_js_vs_rust.md`(성능 실측) · `docs/native_ui_layout_audit.md`(레이아웃) ·
> `docs/roadmap/`(작업 명세) · `docs/perf_optimization_candidates.md`(최적화 후보)

---

## 0. 요약

- **코드 이전은 사실상 완료.** Phase 0·1·2·3·4 완료, Phase 5(성능)는 부분 적용
  (목표 9항목 중 6 달성), Phase 7(UI 정리)은 7.1~7.4 완료·7.5(수동 확인) 진행 중,
  Phase 6(절삭·패키징) 미착수.
- **오늘 게이트: 사실상 그린.** `cargo test --workspace` **595** · clippy/fmt 그린 ·
  `npm test` 410/410 · golden 201/201 · recipe 39/39 · png 5/5 · view/schema OK.
  **유일한 레드 = 오라클 매니페스트 드리프트 4건**(→ V1 — 같은 날 재동결로 해소).
- **큰 덩어리 4개 — 1·2는 2026-10-17 해소**
  1. ~~설정 영속화(3.9)·자동저장(3.10) 미구현~~ → `settings_store.rs`·`autosave.rs` + 셸 배선 완료.
  2. ~~단축키/포커스/busy 가드 미구현~~ → `find_matching_guarded` + Esc 블러 + busy 비활성 완료.
  3. ~~프레임 경로 병렬화(5.2 확장)·텍스처 부분 업로드(5.6)~~ → `perf_js_vs_rust.md`
     §14로 해소(2026-10-17). 남은 것: 성능 목표 미달 4항목(composite/flatten/pen=8/
     documentToJson) + HUD(5.7).
  4. Phase 6 패키징·삭제·문서 재작성(NSIS 결정 포함).

### Phase 진행도

| Phase | 계획 | 상태 | 근거 |
|---|---|---|---|
| 0 기반 | 오라클 동결·워크스페이스 | ✅ | `tools/oracle-manifest.json` (단 V1 드리프트) |
| 1 엔진 | 1.1~1.13 | ✅ | recipe 39/39 · golden 201/201 · png 5/5 |
| 2 네이티브 캔버스 | 2.1~2.7 | ✅ 구현 완료 (공식 수동 스모크만 미실행) | `crates/app/{canvas_view,canvas_texture,session_glue,input*,overlay_raster}.rs` |
| 3 UI 전면 이식 | 3.1~3.10 | ✅ 구현 완료 (3.9·3.10 포함; 수동 스모크만 UI-6) | `ui/{settings_store,autosave}.rs` + 셸 배선 · 웍스 585 |
| 4 자산 파이프라인 | A 선택 | ✅ | viewer 삭제, 개발 자산 유지 |
| 5 성능 | 5.1~5.7 | ⚠️ 부분 | `perf_js_vs_rust.md` §11+§13+§14: 달성 6 · 미달 4 · 미측정 0. 5.6 완료(§14) · 5.7 미착수 |
| 7 UI 레이아웃 정리 | 7.1~7.5 | 🔄 7.5 진행 중 | `docs/roadmap/phase7.yaml` (7.1~7.4 completed, 7.5 in_progress) |
| 6 절삭·패키징 | 6.1~6.7 | ⬜ 미착수 | `src/`·`run.py`·`src-tauri/` 잔존(의도된 상태) |

규모(테스트 포함, 오늘 측정): `engine/src` 19,948줄 + `engine/tests` 1,888 ·
`app/src` 14,384 · `platform/src` 816 — 합계 약 **37k줄**.

### 게이트 실측 (2026-10-17)

| 게이트 | 결과 | 비고 |
|---|---|---|
| `cargo test --workspace` | **595 pass / 0 fail** (engine lib 342+1 ignored, comp_cache 7, e2e 5, fixtures 5, model_basic 14, model_history 11, recipe 1, text_mask 6, platform 24, app 180) | 그린 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | 그린 |
| `cargo fmt --all --check` | exit 0 | 그린 |
| `npm test` | **410 / 410** | 그린 (사용 셸 주의: 부록 A) |
| `npm run parity` | 201 golden checks passed | 그린 |
| `npm run parity:assets` | 39 / 39 | 그린 |
| `node tools/png_cases_check.mjs` | 5 / 5 | 그린 |
| `node tools/view_check.mjs` | OK | 그린 |
| `python schema/schema_selfcheck.py` | OK (14+5 negative, flatten parity, v1 migration) | 그린 |
| `node tools/oracle_manifest.mjs --check` | **OK — 193 files, digest `7b8fbd4a…`** | 재동결 (V1 종결) |
| `cargo build --release -p pixeldraw` | `target/release/pixeldraw.exe` 7.0 MB (3.9/3.10 배선 후 재빌드) | 단일 exe 동작 |

---

## 1. 개선 과제 — UI/UX

| # | 항목 | 현재 상태 | 우선순위 |
|---|---|---|---|
| UI-1 | 설정 영속화(Phase 3.9) | ✅ 구현 완료 (2026-10-17) | ~~P0~~ |
| UI-2 | 자동저장 + 복원(Phase 3.10) | ✅ 구현 완료 (2026-10-17) | ~~P0~~ |
| UI-3 | 단축키 입력 가드 · busy 정책 | ✅ 구현 완료 (2026-10-17) | ~~P0~~ |
| UI-4 | 패널 폭 영속화 | 미구현(인메모리) | P1 |
| UI-5 | 캔버스 텍스처 매 프레임 전체 업로드 | ✅ 구현 완료 (2026-10-17) | ~~P1~~ |
| UI-6 | Phase 7.5 / Phase 2~3 수동 스모크 | 미실행 | **P0(사람)** |
| UI-7 | 잔손질(파일 UX·dead code 등) | 복수 | P2 |

### UI-1. 설정 영속화 미구현 (P0)

- **현재**: `penSize`·주/보조색·`activeTool`·`gridMode`·최근색(8칸)이 **앱 재시작 시 초기화**된다.
  디스크에 저장되는 UI 상태는 `collapsedPanels` 하나뿐
  (`crates/platform/src/ui_prefs.rs` ↔ `platform::settings_file()`의 유일한 사용처).
- **근거**: `crates/app/src/shell.rs`·`ui/` 어디에도 load/save 호출이 없다.
  `Session::set_setting`은 엔진 메모리 상태만 바꾼다. 최근색은 `ui_state.panels.recent`
  인메모리 벡터(`crates/app/src/ui/panels/chrome.rs`).
- **제안**: `docs/roadmap/phase3.yaml` 3.9 명세대로 `ui/settings_store.rs`를 배선한다.
  부팅 시 읽기 → `Session::set_setting`, 변경 시 기존 `settings.json`에 **병합 저장**
  (접기 저장과 같은 패턴, 250ms 디바운스). JS `SETTING_KEYS` 스키마를 재사용한다.

### UI-2. 자동저장·복원 미구현 (P0)

- **현재**: `pixel_platform::autosave_file()`이 정의만 되어 있고 **호출자가 0건**.
  JS 앱의 IndexedDB 자동저장(지연 저장 + 충돌 복원 프롬프트)이 통째로 없다.
  크래시·강제 종료 시 작업이 전부 유실된다.
- **제안**: phase3.yaml **3.10a**(`AutosaveRecord` 빌드/로드, 800ms·5000ms 디바운스,
  `write_bytes_atomic`) → **3.10b**(부팅 복원/버리기 다이얼로그). Esc=버리기 정책은
  D8(패리티) 결정대로 유지.

### UI-3. 단축키 입력 가드·busy 정책 미구현 (P0)

- **현재**: `crates/app/src/ui/actions.rs::Shortcut::matches`가
  `ctx.input(|i| i.modifiers == self.modifiers && i.key_pressed(self.key))`로만 판정하고,
  `crates/app/src/shell.rs::ui()`가 매 프레임 `find_matching`을 호출한다.
  - `ctx.wants_keyboard_input()` 가드가 없어 **텍스트 입력 중 단일키 도구 단축키**
    (`b/r/l/o/t` 등)가 발동할 수 있다(레이어 이름·HEX·문서 이름·텍스트 도구 입력).
  - 다이얼로그 열림 상태(busy)도 미구현 — phase7.2가 "native UI busy 상태는 현재
    미구현"으로 명시하고, 2026-10-14 로그가 남은 과제 ④로 적어 둔 그 항목이다.
  - Escape로 포커스 해제(`surrender_focus`) 경로도 없다.
- **제안**: `find_matching`에 `!ctx.wants_keyboard_input() && !busy` 가드 추가,
  다이얼로그/토스트 상태를 busy로 묶어 Undo/Redo 버튼·단축키를 비활성화,
  JS `isEditableTarget`/`isBusyUi` 의미 그대로. `input::controller`의
  `text_focused` 가드(캔버스 tool 키)는 이미 있으므로 ActionMap 쪽만 메운다.

### UI-4. 패널 폭 영속화 없음 (P1)

- **현재**: 좌/우 패널 폭은 egui `PanelState`(프레임 간 기억)만 되고 **재시작하면 기본값**.
  `crates/app/Cargo.toml`의 eframe이 `persistence` feature를 쓰지 않는다
  (`default-features = false`, `glow`, `default_fonts`만).
- 참고: `docs/native_ui_layout_audit.md`가 "폭 영속화는 설정 스키마 변경이 필요해
  이번 패치 범위 밖"으로 보류해 둔 상태.
- **제안**: JS 키 `dt.panelWidths.v1`(사용자가 명시 조절한 쪽만 저장)로
  `settings.json`에 병합 저장. UI-1과 같은 커밋에서 처리하면 스키마 변경이 한 번에 끝난다.

### UI-5. 캔버스 텍스처 매 프레임 전체 업로드 (P1 — 성능과 겹침)

- **현재**: `crates/app/src/canvas_view.rs::show_canvas`가 repaint 프레임마다
  `texture.upload(ctx, &fb.rgba, …)`를 무조건 호출한다. `CanvasTexture::upload`는
  `ColorImage::from_rgba_unmultiplied`(전체 복사) + `handle.set`(GPU 업로드)이다.
  모듈 주석("uploads happen only when the size or the bytes change")과 실제 동작이 불일치.
- **제안**: Phase 5.6 명세대로 변경 청크만 부분 업로드(`TextureHandle::set_partial`)하거나,
  "변경 없음" 플래그로 스킵. 대형 뷰포트에서 마우스 이동 프레임마다 복사 + 업로드가
  발생하는 것을 줄인다.
- **결과(2026-10-17)**: `CanvasTexture::present(ctx, fb, Upload)` — ①서명 동일 + 손상
  없음 `Skip`(그리기·복사·업로드 전부 생략), ②서명 변경·첫 프레임 `Full`, ③그 외에는
  손상 union rect 하나를 `set_partial`. 손상 = 이번 프레임이 실제로 그린 청크의 device
  rect(여백 2px, fb 클립) ∪ (이전 ∪ 현재 오버레이 bbox). 전체 fb 복사 **0.281 ms →
  영역 ~1 µs**(16×16 크로스헤어 ~0.4 µs · 64×64 wash ~1.1 µs), 무변경 프레임은 0.
  상세·안전성 근거·테스트는 `perf_js_vs_rust.md` §14-1.

### UI-6. 수동 스모크 미실행 (P0, 사람 확인)

- `docs/native_smoke.md`의 **UI 레이아웃 체크리스트 전 항목이 `[ ]`**, 실행 기록은
  "GUI 수동 확인 필요"로 끝나 있다. Phase 2 게이트(그리기·파일 포함)와 Phase 3 UI 항목도
  같은 상태이며, Phase 7.5의 accept 조건이 바로 이 확인이다.
- **제안**: release exe로 1280×820 / 900×560에서 체크리스트를 1회 실행하고,
  실패 항목을 이슈화한 뒤 실행 기록을 채운다. 이것이 끝나야 Phase 2·3·7의 공식 게이트가 닫힌다.

### UI-7. 잔손질 (P2)

- `canvas_view.rs`의 dead code: `let cursor = state.tools.cursor(); let _ = cursor;` 제거.
- **SaveAs가 Save와 동일**(`shell.rs` — "Same as Save for now"): 현재 파일 경로 추적과
  "다른 이름으로 저장" 분리, 최근 디렉터리 기억을 검토.
- `ui/shared/strings.rs` 중앙화 잔여(일부 라벨이 피처 모듈에 남아 있는지 미확인) —
  기계적 정리 후보.
- 메뉴 hover 220ms 지연 닫힘 등 JS 세부 동작은 egui 기본 동작과 다르면 조정(선택).

---

## 2. 개선 과제 — 성능

기준: `docs/perf_js_vs_rust.md` §11 (2048×1088·64레이어) + `perf_optimization_candidates.md`.
적용 완료 기법(커버리지 비트마스크·스팬 쓰기·청크 해석 호이스트·루프 인터체인지·워드 스킵·
dense 인덱스·스탬프 중복 제거·rayon)의 효과는 그 문서에 있다. 아래는 **남은 것**이다.

| # | 항목 | 상태 | 우선순위 |
|---|---|---|---|
| PERF-1 | 6ms 프레임 경로가 단일 스레드 | **완료**(2026-10-17) — 무예산 pass 1만 병렬(게이트 `chunks×layers ≥ 3072`), 예산 pass 2는 순차 유지 | ~~P1~~ |
| PERF-2 | 목표 미달 3항목(composite/flatten/pen=8) | 미달 | P1 |
| PERF-3 | 저장(documentToJson) 측정·compact·rayon | **완료**(2026-10-17) — 578.9 ms, 목표 4.8× 미달 | P1 |
| PERF-4 | 5.6 텍스처 부분 업로드·5.7 프레임 HUD | 5.6 **완료**(2026-10-17) · 5.7 미착수 | P1/P2 |
| PERF-5 | 미측정 항목(Undo 메모리 등) | 미측정 | P2 |
| PERF-6 | 후보 기법(실험 기록 존재) | 보류 | P2 |

### PERF-1. 프레임 경로 병렬화 (P1)

- **현재**: rayon은 엔진의 `composite_canvas`/`flatten_to_rgba`(전체 캔버스 경로)에만 적용.
  실제 화면 갱신은 `comp_cache::paint`의 **단일 스레드** `composite_chunk`가 6ms 예산
  (`FRAME_BUDGET_MS`, `session_glue::frame_budget()`) 안에서 돈다.
- **리スク**: dirty 청크가 많을 때(팬·새 문서·다수 레이어 편집) 프레임이 예산에 눌려
  여러 프레임에 걸쳐 늦게 완성된다.
- **제안**: 예산 인지 병렬 합성(가시 청크 우선 → dirty 청크 병렬) 또는 청크 행 단위
  rayon. `perf_optimization_candidates.md`의 "chunk-granular rayon" 실험은 **느려서
  롤백된 이력**이 있으므로, 프레임 경로용으로는 워크로드·스레드 풀을 달리해 재검증한다.
- **결과(2026-10-17)**: 무예산 **pass 1**(가시 청크, JS 패리티라 순서·바이트 고정)만
  병렬 — 합성은 64청크 배치 `par_iter`, blit은 원 스레드·JS 키 순서. 바이트·dirty·
  `last_painted_keys`가 스케줄과 무관함을 쌍둥이 테스트로 고정
  (`paint_dirty_budget_serial` = A/B·회귀용 1스레드 쌍둥이). 게이트는
  `chunks × layers ≥ 3072`: 이하면 기존 순차 경로 그대로라 **느려지는 케이스가 없다**
  (게이트 없던 첫 실행에서 1L 콜드 그리드가 17% 손실이었다). framebench A/B —
  drag 60청크×64L **8.9 → 1.4 ms(6.4×)**, 콜드 그리드 2176×64L **102.4 → 24.5 ms(4.2×)**.
  예산 pass 2(6 ms 약속)는 설계대로 순차 유지. 상세는 `perf_js_vs_rust.md` §14-2/§14-3.

### PERF-2. 목표 미달 3항목 (P1)

`perf_js_vs_rust.md` §11-1 기준(median, JS ÷ Rust 배수 표기가 아니라 목표 대비):

| 항목 | 실측 | Phase 5 목표 | 격차 |
|---|---:|---:|---:|
| composite 2176청크×64 | 14.266 ms | ≤ 2 ms | 7.1× |
| flattenToRgba 2048×1088 | 14.697 ms | ≤ 3 ms | 4.9× |
| stroke pen=8 (20세그) | 4.124 ms | ≤ 2 ms | 2.1× |

- composite/flatten은 이미 rayon 적용 후다. 남은 것은 `over_blend` f64 연산 자체와
  인덱싱 — **5.1 u32 워드 SIMD**와 로드 단위 스킵 확장이 다음 수순(§11-5).
- pen=8은 스탬프 경로 미세 최적화(스팬 폭 임계, footprint 재사용).

### PERF-3. 저장 성능 — 측정·포맷 전환 완료 (2026-10-17), 목표 미달은 잔존 (P1)

- **측정**: `bench.rs`의 낡은 주석("Phase 1.13 owns io/serialize")을 걷어내고
  `documentToJson (save)` 항목을 신설했다. 워크로드는 §1·§11과 동일(2048×1088·64레이어,
  **37,504청크** — JS 오라클 프로브로 청크 수 일치 확인). Rust는 순차 **1159.2 ms** →
  청크 인코딩 rayon **578.9 ms**(2.0×)로 **JS 2114.6 ms 대비 3.65× 빠름** —
  그러나 목표 120 ms에는 **4.8× 미달**(잔존 과제).
- **compact 전환(완료)**: `save_json_bytes`가 `to_vec_pretty` → `to_vec`. 같은 문서에서
  9806 KB → **7455 KB(−24%)**, 문자열화 9.1 → 6.3 ms(저장 전체의 ~1%).
  JSON 파서는 둘 다 읽으므로 저장 호환성 불변.
- **전제 정정**: "JS는 compact `JSON.stringify`였다"는 이전 서술은 틀렸다 — 그건 벤치의
  크기 측정용 문자열이었고, 실제 JS 저장 파일은 `stringifyReadableJson`(실측 9909 KB)이다.
  Rust는 readable 포터가 없어 공백만 다른 compact로 정했다(양쪽 파서 호환).
- **남은 미달의 원인**: 문자열화(6.3 ms)가 아니라 청크당 PNG 인코딩(37,504회)이다.
  다음 후보(할당 재사용·deflate 튜닝)는 `perf_js_vs_rust.md` §13-4 — deflate는
  후보 문서에 롤백 이력이 있어 재시도 시 하네스 기록이 필요하다.

### PERF-4. 5.6·5.7 (P1/P2)

- ~~**5.6 텍스처 부분 업로드**~~: **완료(2026-10-17)** — UI-5로 처리(`Skip`/`Full`/`Region`
  + `set_partial`). 측정·근거는 `perf_js_vs_rust.md` §14-1.
- **5.7 tracing + 프레임 타이밍 HUD**: 미착수. 개발 빌드에서 프레임 ms·dirty 청크 수·
  레이어 수·comp 예산 초과 횟수를 오버레이로 표시하면 회귀를 눈으로 잡는다.

### PERF-5. 미측정 항목 (P2)

`perf_js_vs_rust.md` §11-5가 그대로 백로그다:

- Undo 바이트 예산 회귀(커버리지 비트마스크가 청크당 +128B = 문서 +3.1%) 미측정.
- 빈 레이어 메모리(dense 슬롯 테이블 레이어당 최대 17KB) 미측정.
- 레이어 1~3개의 composite(비트마스크 이득 최대 구간) 미측정.
- ~~저장 JSON 크기(JS 7557KB) 비교 미측정.~~ **측정(2026-10-17)**: JS 실제 저장 포맷(readable) 9909KB / JS compact 7557KB / Rust compact 7455KB (§13).

### PERF-6. 보류 중인 후보 (P2, 실험 기록 있음)

`perf_optimization_candidates.md`의 미적용 후보: ①계층형 플러드필+비트 연산 스캔라인
②레이어 스킵+SIMD 요소별 블렌드 ③스트로크 타일 재사용·브러시 footprint 캐시
④PNG 필터/deflate 튜닝 ⑤target-cpu-native/PGO 프로필.
단, `#[inline(always)]`·deflate `fast`·청크 rayon은 실험 후 롤백된 이력이 있으므로
재시도 시 같은 벤치 하네스로 전후를 남긴다.

---

## 3. 검증 · 안정성

### V1. 오라클 매니페스트 드리프트 4건 (P0 — 결정 필요)

- **현재**: `node tools/oracle_manifest.mjs --check` →
  ```
  DRIFTED  src/core/blend.js
  DRIFTED  src/features/layers/panel_layers.js
  DRIFTED  src/io/export_png.js
  DRIFTED  src/render/composite.js
  oracle moved: 4 change(s) against tools/oracle-manifest.json.
  ```
- **원인**: `48953ec`("feat(native): … blend zero-alloc")가 JS 오라클을 의도적으로 수정했다
  (`overBlendInto` zero-alloc 경로 도입, composite/export_png가 재사용 tmp 사용,
  opacity 드래그 리스트 재구축 스킵). 매니페스트는 Phase 0.3 동결(`d5c2744`) 이후
  **재동결되지 않았다**.
- **영향**: README 검증 목록의 게이트 1개가 현재 레드다. 단, JS 동작 동일성은
  `npm test 410/410` + golden 201/201 + assets 39/39로 확인된다(연산 순서 변화 없음).
- **제안(택 1, 사용자 결정 필요)**:
  - (a) **재동결**: "동작 동일, 할당만 감소"임을 근거로 `tools/oracle-manifest.json`을
    갱신하고 변경 이유를 로그에 기록. `tests/fixtures.rs`가 단언하는 키 목록은
    파일 집합이 그대로이므로 유지된다(재생성 전 키 보존 확인 필수).
  - (b) **원복**: JS 수정을 revert해 오라클 순수성을 회복한다(Rust 포팅에는 영향 없음 —
    Rust 쪽 장부 수정과 다른 계통의 변경이다).
  - 어느 쪽이든 `--check` 그린 복구가 목표다.

### V2. 수동 스모크 미실행 (P0 — 사람)

- `docs/native_smoke.md`: UI 체크리스트 전 항목 미체크, 실행 기록 "미실행".
  Phase 2·3의 공식 게이트가 여기에 걸려 있다(UI-6과 동일 작업).
- **제안**: 릴리스 exe로 1회 돌리고 실패 항목만 픽스. 성공 시 Phase 2·3·7을 보드에서 닫는다.

### V3. 소소한 견고성 (P2)

- `AppState::drain_events`가 큐를 무제한 drain한다. 계획(index.yaml 번역 규약)은
  "1회 최대 1024개, 초과 시 `log::error`"이다 — 무한 루프 방지 캡을 넣는다.
- 저장 후 현재 파일 경로·dirty 플래그 정책(저장하면 dirty 해제 표시가 맞는지) 확인.

---

## 4. 문서 · 계획 정합성

| # | 항목 | 현재 | 제안 |
|---|---|---|---|
| D1 | `docs/progress/README.md` 보드 stale | 누적 게이트가 **321**(2026-10-11) — 실제 567. Phase 2 체크박스 `[ ]`(코드는 완료), Phase 3 `[x]`인데 3.9/3.10은 미구현, Phase 5 `[ ]` | Phase 7.5 종료 시 보드·게이트·커밋 이력 일괄 갱신 |
| D2 | `docs/native_plan.md` 상태줄 | "Phase 1 완료"에 고정 | Phase 상태표 갱신(규칙 5: 보드가 최신) |
| D3 | `docs/ui_ux_port_analysis.md` | 초기 분석 — dioxus/yew/Tauri 권고, 셀/타일 서술 오류(index.yaml에 기재) | 문서 상단에 "초기 분석·현 아키텍처(eframe/egui)와 불일치" 표기 또는 Phase 6에서 정리 |
| D4 | `known_doc_code_discrepancies` | 일부 미수정(`docs/README.md` 표 깨짐 등) | Phase 6.6 문서 재작성 때 일괄 |
| D5 | Phase 7 로그 미기록 | `log.md` 최신 = 2026-10-15, `phase7.yaml`만 갱신 | 로그 초안 작성(규칙 6 — 오케스트레이터 합류) |
| D6 | 미커밋 변경 | `crates/app/src/ui/{mod,menubar,toolbar,panels/color}.rs`, `docs/native_smoke.md`, `docs/roadmap/index.yaml`, `run.py`(1줄), 신규 `docs/native_ui_layout_audit.md`·`native_ui_layout_compare.html`·`phase7.yaml` | 7.5 수동 확인 후 **사용자 승인 하에** Phase 7 배치 커밋 |

---

## 5. 패키징 (Phase 6) — 미착수

단일 exe 빌드는 동작한다(`target/release/pixeldraw.exe` 7.0MB, WebView2 불요).
남은 것:

- **6.2 NSIS**: D13 결정(별도 NSIS vs Tauri 캐시 `makensis.exe`, per-user/per-machine,
  언인스톨 시 `%APPDATA%/pixeldraw` 보존). 아이콘·버전 리소스도 여기서.
- **6.3 삭제**: `src/`·`index.html`·`style_*.css`·`run.py`·`src-tauri/`·
  `tools/{stage,bundle,check_stage}_*.mjs`. 오라클 은퇴(D7)와 연동 — 골든 원본
  (`reference/`, `tests/fixtures/*`, `tools/oracle-manifest.json`)은 유지.
- **6.4~6.6 문서**: `contract.md` §4~§5, README·desktop.md 폐기/대체,
  core/render/model/io/tools/ui 문서를 크레이트 기준으로 재작성.
- **6.7 CI 게이트**: `cargo test/clippy/fmt` + 오라클(Windows 러너).
- **폰트(D3)**: 시스템 폰트 의존이므로 배포판에서 한글 폰트가 없는 환경이면 UI가 깨진다 —
  번들 여부(라이선스·크기)를 Phase 6에서 확정.

---

## 6. 권장 순서

1. ✅ **(P0) 오라클 드리프트 결정**(2026-10-17 재동결) → `oracle_manifest --check` 그린 복구 (V1)
2. **(P0) Phase 7.5 수동 스모크 + Phase 2~3 체크리스트 1회** → Phase 7 배치 커밋·로그·보드 (UI-6, V2, D1/D5/D6)
3. ✅ **(P0) 설정 영속화(3.9) + 자동저장·복원(3.10)** (2026-10-17, UI-1·UI-2)
4. ✅ **(P0) 단축키/포커스/busy 가드** (2026-10-17, UI-3)
5. ✅ **(P1) 저장 벤치 측정 + compact 전환 + 청크 인코딩 rayon** (2026-10-17, PERF-3 — 578.9 ms · 목표 4.8× 미달은 잔존)
6. ✅ **(P1) 프레임 경로 병렬화 + 텍스처 부분 업로드** (2026-10-17, PERF-1·UI-5·PERF-4 5.6 — `perf_js_vs_rust.md` §14: 64L 콜드 102.4→24.5 ms · 업로드 0.281 ms→영역 ~1 µs/스킵)
7. **(P1) 성능 잔여 미달 4항목(composite/flatten/pen=8/documentToJson)** (PERF-2·PERF-3 잔여)
8. **(P1/P2) 파일 UX 잔손질(SaveAs·최근 경로), 패널 폭 영속화** (UI-7, UI-4)
9. **(P2) 문서 정합성 정리 → Phase 6 착수** (D2~D4, §5)

---

## 부록 A. 검증 재현 명령 (2026-10-17 실행)

```powershell
cargo test --workspace                 # 595 pass / 0 fail
cargo clippy --workspace --all-targets -- -D warnings   # exit 0
cargo fmt --all --check                # exit 0
npm.cmd test                           # 410 / 410  ※ 이 환경에서 `npm`(.ps1)은 실행 정책 차단 → npm.cmd 사용
npm.cmd run parity                     # 201 golden checks passed
npm.cmd run parity:assets              # 39 / 39
node tools/png_cases_check.mjs         # 5 / 5
node tools/view_check.mjs              # OK
python schema/schema_selfcheck.py      # OK
node tools/oracle_manifest.mjs --check # OK (193 files, 2026-10-17 재동결)
cargo build --release -p pixeldraw     # target/release/pixeldraw.exe (7.0 MB, 배선 후 재빌드)
cargo run -p pixel-engine --release --example bench   # documentToJson 578.9 ms (median) — §13
cargo run -p pixel-engine --release --example framebench  # UI-5/PERF-1 A/B (§14) — 64L 콜드 24.5 ms vs 102.4 ms
```

## 부록 B. 참조 파일

- 계획/로그: `docs/native_plan.md`, `docs/progress/{README.md,log.md,board.html}`, `docs/roadmap/*.yaml`
- 성능: `docs/perf_js_vs_rust.md`, `docs/perf_optimization_candidates.md`, `crates/engine/examples/bench.rs`
- UI: `docs/native_ui_layout_audit.md`, `docs/native_ui_layout_compare.html`, `docs/native_smoke.md`, `docs/ui_ux_port_analysis.md`
- 구현 핵심: `crates/app/src/{shell.rs,canvas_view.rs,canvas_texture.rs,session_glue.rs,file_ops.rs,ui/}`, `crates/platform/src/{ui_prefs.rs,paths.rs}`, `crates/engine/src/{comp_cache/,display/,io/}`
- 오라클: `tools/oracle-manifest.json`, `tools/oracle_manifest.mjs`, `tools/parity_check.mjs`
