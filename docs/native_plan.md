# native — 완전 오프라인 exe화 이전 계획

`src/`(ESM JS 11,492줄)를 Rust 네이티브 앱으로 이전하는 실행 계획이다.
웹 런타임(WebView2·브라우저·`run.py`·Node 테스트)에의 의존을 **끝낸다**.
규범은 계속 `docs/contract.md`이며, 이 문서는 이전 **작업 계획**이다.

작성: 2026-10-10 · 상태: **Phase 1 완료**(0.1~0.4 베이스라인·워크스페이스·동결·픽스처 하네스 및 1.1~1.13 완료; Rust native recipe parity 39/39)

---

## 0. 요약

| 항목 | 결론 |
|---|---|
| 언어 | **Rust**(기존 `src-tauri/` 크레이트·Rust 1.99 툴체인 재사용) |
| 목표 산출물 | `pixeldraw.exe` **단일 파일**, WebView2 불요, 오프라인 완전 동작 |
| 이전 범위 | 엔진 → 문서 모델 → 도구 → UI → 저장소 **전부** |
| 웹 경로 | `run.py`·브라우저 분기 **폐기**(`src/io/file_io.js` 분기 삭제) |
| 검증 | JS 오라클(골든 픽스처·패리티)을 Rust 테스트가 그대로 소비 |
| 공수 | 관측 속도 기준 **약 1~2주**(보수적 3~4주). 근거는 §0-1 |

### 왜 "그 이상"까지 가는가

`docs/desktop.md` §1은 JS를 유지한 이유로 ①Canvas 2D면 충분 ②Pillow 대비 패리티
재검증 비용 ③Node 위 테스트 7,900줄를 들었다. 방향이 뒤집히면 셋 다 **비용이
아니라 자산**이 된다.

1. **Canvas 2D가 충분했던 이유가 사라진다.** 줌 32배·DPR 1.25·`difference` 합성
   그리드·체커 배경은 전부 프레임버퍼 연산으로 더 정확하고 빠르게 재현된다.
   네이티브에서는 합성 결과를 GPU 텍스처 한 장으로 올리는 것이 끝이다.
2. **패리티 자산은 그대로 쓴다.** `reference/raster_ref.mjs`(PIL 오라클 이식)와
   `tests/fixtures/raster_golden.json` 201건·recipe 39/39·png 5/5는 **언어와
   무관**하다. Rust 크레이트가 같은 JSON을 읽어 같은 기대값을 비교하면 된다.
   JS 구현은 오라클로 동결하고 엔진만 Rust로 쓰면, 재작성이 아니라 **검증 가능한
   이전**이 된다.
3. **Node 위 테스트는 `cargo test`로 대체된다.** DOM-free 모듈 계약
   (`contract.md` §4)은 Rust의 `no GPU·no OS 크레이트` 경계로 1:1 대응한다.

### 0-1. 공수 추정의 근거(에이전트 가속 관측 속도)

이 저장소는 **사람이 손으로 친 코드가 아니라 AI 에이전트로 생산**했다.
그 전제를 이전 계획에 그대로 쓴다.

| 항목 | 줄수 |
|---|---:|
| `src/` | 11,492 |
| `tests/` | 5,978 |
| `tools/` | 3,700 |
| `reference/` | 556 |
| **합계** | **21,726** |

이 분량이 **1주일**에 만들어졌다(에이전트 병렬). 관측 속도는 **약 3,100줄/일**,
Phase 6까지 Rust 산출 목표는 **8,000~9,000줄 + 테스트**이다.

- 순수 코드 생성만으로는 **3~4일** 분량이다.
- 차이는 **패리티 디버깅**에서 난다. 타수가 아니라 다음 네 곳이 실제 날짜를
  결정한다: ①polygon·arc·wide_line·글리플의 경계 픽셀 ②`serialize` 바이트 동일
  ③모델/Undo 의미론 ④UI 동작 매칭(색환·옵션바·레이어 패널).
- 에이전트는 ①~④를 **병렬로 파고들 수 있다**(Phase 1의 13개 항목은 서로 의존성이
  낮아 동시 진행 가능). 따라서 일정은 "코드 양"이 아니라 **게이트를 몇 바퀴
  도느냐**로 잡는다. Phase별 게이트가 그 역할을 한다.

| 시나리오 | 기간 | 조건 |
|---|---|---|
| 낙관 | **1주** | Phase 1 항목을 에이전트 3~4개로 병렬, 게이트 한 바퀴 통과 |
| **기준** | **2주** | 래스터/직렬화에서 2~3회 수정 바퀴, UI는 매핑 중심 |
| 보수 | 3~4주 | 글리프·색환·near-parity 허용오차 재현에서 추가 바퀴 |

> Phase 2 말미에 "오프라인에서 그을 수 있는 exe"가 존재하도록 순서를 짰다.
> 이후 Phase는 그 위의 완성도 작업이므로, **조기 중단 지점**으로 삼을 수 있다.

---

## 1. 목표 / 비목표

### 목표

- 단일 `pixeldraw.exe`. 설치 런타임 0개(WebView2·Node·Python 불요).
- 현재 에디터의 **기능 전부**: 도구 10종(펜·지우개·도형 5종·채우기·그라디언트·
  스포이트·손·텍스트·호), 레이어 64·블렌드 6종·Undo 200걸음/96 MiB 예산,
  PNG·문서 JSON 저장, 스키마 2.0.0 그대로.
- 현재 **렌더링 동작과 픽셀 단위 동일**: 합성·그리드·체커·테두리·오버레이.
- 저장 파일 **호환**: 기존 `.json`(래스터/벡터/레시피)과 PNG를 읽고 쓴다.

### 비목표(명시적)

- 파일 포맷 버전 업(`SCHEMA_VERSION`은 `"2.0.0"` 유지).
- 새 도구·새 블렌드 모드 추가(이전 중 기능 추가 금지 — 패리티 오염).
- `tools/`·`reference/`·`asset_work/`(베이킹·오라클·생성 산출물) 제거.
  엔진/앱과 무관한 개발 자산이며 골든 픽스처의 **원본**이다.
- Windows 외 플랫폼(향후 `winit` 멀티플랫폼이므로 가능은 하나 범위 밖).

---

## 2. 현재 기반(이전의 재료)

| 자산 | 규모 | 이전 후 역할 |
|---|---|---|
| `src/` 108개 모듈 | 11,492줄 | 이전 원본. 완료 후 `reference-app/`로 격하했다 제거 |
| `contract.md` §1–§6 | — | **그대로 규범.** 상수·불변·에러 19종·DOM 금지 구역·DAG |
| `reference/raster_ref.mjs` | 514줄 | PIL 오라클 이식. Rust 패리티의 **정답지** |
| `tests/fixtures/raster_golden.json` | 201건 | brush·ellipse·rrect·outline·bresenham·stroke·wide_line·polygon·polygon_outline·fillet·gradient·arc·chord |
| `tools/recipe_parity_check.mjs` | 39/39 | 시트 레시피 합성 패리티 |
| `tests/fixtures/png_cases.json` | 5/5 | PNG 디코드·에러 케이스 |
| `tools/view_check.mjs`·`view_ref.mjs` | OK | 뷰 수학(줌·팬·클램프·fit) |
| `schema/schema_selfcheck.py` | OK | 스키마 자체 검사 |
| `tools/recipe/text_glyphs.json` | — | 텍스트 도구 글리프(비트맵) |
| `npm test` 410건 | ~60파일 | 이전 기간 **오라클**로 유지, cutover 후 은퇴 |

### 이전의 핵심 난제(미리 못 박기)

1. **부동소수 동일성.** `overBlend`는 f64 연산 후 `rnd(v)=Math.floor(v+0.5)`.
   Rust도 f64·동일 연산 순서면 IEEE 754로 **비트 동일**하다. 단
   `Uint8ClampedArray` 쓰기 클램프는 Rust에서 `u8::clamp`로 명시해야 한다.
2. **u32 패킹.** `packRGBA`는 `a===0 ? 0 : (a<<24|b<<16|g<<8|r)>>> 0`.
   JS `>>>0` 부호 없는 시프트를 Rust `u32`로 그대로 옮긴다.
3. **near-parity 허용 오차.** `arc`(missing 0·extras ≤16)·`chord`(missing ≤16)는
   **PIL이 아니라 JS 구현의 현재 동작**이 정답이다. Rust는 JS를 복제한다.
4. **PNG 바이트.** `CompressionStream("deflate")` → `flate2` 교체 시 **압축
   바이트는 달라지고 픽셀은 동일**하다. 게이트는 디코드 픽셀 비교
   (`png_cases_check`)이므로 통과 가능. 저장 JSON 안 base64 PNG도 라운드트립
   픽셀 비교로 검증한다.
5. **스키마 키 순서.** `serialize.js`는 문서 JSON의 **키 순서**가 고정되어 있다.
   `serde` 구조체 필드 선언 순서가 곧 직렬화 순서이므로 시리얼라이저를
   구조체로 1:1 모델링한다(동적 맵 금지).

---

## 3. 목표 아키텍처

```text
pixeldraw.exe  (단일 파일, 오프라인, 설치 런타임 0)
│
├── app/        eframe + egui + winit      창·메뉴·패널·입력·다이얼로그
│   ├── session_glue.rs   Document ↔ UI 알림(events.js 7종 → 관찰자)
│   ├── canvas_view.rs    픽셀 캔버스 위젯(텍스처 Nearest 블릿 + 줌/팬)
│   └── panels/           color·layers·pen·shape·gradient·arc·text·view
│
├── platform/   rfd(대화상자) · directories(앱 경로) · 파일 설정/자동저장 · log
│
└── engine/     pixel-engine  ← 순수 Rust. GPU·OS·네트워크 의존 0
    ├── constants errors events pixel blend        (contract §1~§3 직항)
    ├── chunkstore    ChunkStore · PixelWriter · changeset · changeSetBytes
    ├── raster/       brush masks segment polygon arc gradient text atlas
    │                composite_ops(clip_alpha/erase/visor/sheen/hud_brackets)
    ├── kernels/      pen stroke · flood fill(스캔라인) · applyShape
    ├── composite/    compositeChunk · samplePixel · flattenToRgba
    ├── model/        Document · Layer · IdGen · Commands · UndoManager · Session
    └── io/           png(CRC·필터0-4) · serialize(스키마 2.0.0) · validate
```

### 계층 규칙(JS DAG의 직항)

```
constants/errors/events ← pixel ← blend ← chunkstore ← raster ← kernels
                                                   ↓
                        composite ← model(commands ← history ← session)
                                                   ↓
                        io ← app ← platform
```

- `engine/`은 **외부 크레이트 의존 0**에 가깝게 유지한다(테스트용 `serde_json`·
  `flate2`는 dev/io로 한정). 의존 방향은 JS와 동일하게 바깥→안쪽.
- `engine/`의 모든 공개 함수는 `DrawToolError` 코드(`OUT_OF_RANGE` 등 19종)를
  반환형으로 노출한다. JS의 `DrawToolError(code,message,details)` 1:1.

---

## 4. 기술 스택

| 역할 | 크레이트 | 대체재 / 메모 |
|---|---|---|
| 창·이벤트루프 | `winit`(eframe 경유) | GLFW 불요 |
| UI | `egui`(immediate) | 패널·메뉴·슬라이더·드래그가 즉시 매핑. 색휠은 커스텀 페인트 |
| 픽셀 캔버스 표시 | `egui::TextureHandle` + `TextureFilter::Nearest` | 줌 32배·DPR 무관 픽셀 정확. 필요 시 `pixels`/`wgpu`로 분리 |
| 합성 결과 업로드 | 엔진 framebuffer `Vec<u8>` → 텍스처 | 체커·그리드·오버레이는 **프레임버퍼 안에서** 그림(egui 도형 분리 불가) |
| JSON | `serde` + `serde_json` | 키 순서 = 구조체 필드 순서 |
| PNG 압축 | `flate2`(`miniz_oxide`) | 인플레이트 포함. CRC·청크·필터는 직접 |
| 병렬 | `rayon` | 청크 합성·PNG·플러드필 분할 |
| 대화상자 | `rfd` | 저장·열기·확인 |
| 앱 경로 | `directories` | 설정·자동저장 |
| 로그 | `log` + `env_logger` | 디버그 빌드 |
| 에러 | `thiserror` | 내부용, 공개는 19종 코드 |

**의도적으로 쓰지 않는 것**: 웹 API 전부, `CompressionStream`, `Uint8ClampedArray`,
`EventTarget`, `CustomEvent`, `localStorage`/`IndexedDB`, Tauri 플러그인.

---

## 5. 포팅 매핑표와 공수

LOC는 `Measure-Object -Line` 측정값(2026-10-10)이다. Rust는 엔진이 1.2~1.6배로
길어지고(타입·`Result`), UI는 egui로 0.4~0.6배로 줄어든다.

| JS 그룹 | 파일 | LOC | Rust 모듈 | 난이도 | 비고 |
|---|---:|---:|---|---|---|
| 엔진 기초 | `core/{constants,errors,events,pixel,blend}.js` | 230 | `engine/src/{constants,errors,events,pixel,blend}.rs` | 하 | 즉시 패리티. `rnd` 순서 고정 |
| 희소 저장 | `core/chunkstore.js` | 242 | `engine/src/chunkstore.rs` | 하 | `Map<u64,Box<[u8;4096]>>`. `u32` 뷰로 `isAllZero` |
| 래스터 | `core/raster/` 11파일 | 1,284 | `engine/src/raster/*` | **최고** | polygon 301줄이 관건. `composite_ops`(294) 포함. `raster_brush`(29)는 `brush.js`로 통합 |
| 합성 커널 | `render/composite.js` | 113 | `engine/src/composite.rs` | 중 | `compositeChunk`·`samplePixel` 동치 불변 |
| 도구 커널 | `features/fill/fill.js`·`features/pen/brush.js` | 143 | `engine/src/kernels/` | 중 | 스캔라인 플러드필, `strokeSegment` |
| 문서 모델 | `features/document/` 4 + `features/layers/` 5 | 1,269 | `engine/src/model/*` | 중 | Command·Undo 바이트 예산·IdGen 카운터 |
| 저장/검증 | `io/{png,serialize,validate*,export_png,serialize_vector,serialize_recipe,base64,stringify_readable}.js` | 1,238 | `engine/src/io/*` | 중~상 | 스키마 키 순서. `file_io`/`idb_*` 7파일은 **삭제**(플랫폼 대체) |
| 뷰·표시 수학 | `features/viewport/{view,view_store}.js` | 172 | `engine/src/view.rs` | 하 | DOM-free. 줌 12단계 frozen |
| 표시 렌더 | `render/{renderer,renderer_composite,renderer_display,grid,background}.js` | 524 | `app/src/canvas_render.rs` | 중 | OffscreenCanvas → 프레임버퍼. `difference` 그리드는 CPU |
| 도구(로직) | `features/pen/pen.js`·`shape/` 6파일·`gradient/arc/text/eyedropper/hand` | ≈1,600 | `app/src/tools/*` | 중 | 상태 머신 이식. 입력은 이벤트루프에서 정규화 |
| 입력 플럼빙 | `tools/*` 9파일 | 797 | `app/src/input/*` | 중 | winit 이벤트 → `{x,y,button,shift,alt,coalesced}` 정규화 |
| UI·패널·메뉴 | `ui/**` 24파일(2,061) + 피처 패널 16파일(1,614) | 3,675 | `app/src/panels/*` | 중(양 많음) | egui. `panel_color_wheel` 312줄이 최대 |
| **합계** | **117파일(스테이징 108 모듈)** | **11,492**(실측) | | | Rust 예상 산출 **약 8,000~9,000줄** |

> 그룹 LOC은 근사값이다(일부 파일이 두 그륩에 걸쳐 센다). 총계 11,492는
> `Measure-Object -Line` 실측값이며 이 문서의 공수 추정 기준이다.

삭제·대체되는 JS(이전 완료 기준):

| 파일/도구 | 처리 |
|---|---|
| `src/io/file_io.js`(172) | 삭제 — `platform`이 네이티브 직접 쓰기 |
| `src/io/idb_*.js`·`store_idb.js`(425) | 삭제 — 파일 자동저장으로 대체 |
| `src/io/base64.js`(40) | 삭제 — IPC base64 브리지 소멸 |
| `index.html`·`style_*.css` 4개 | 삭제 — CSS 전부 |
| `run.py`·`tools/stage_app.mjs`·`bundle_app.mjs`·`check_stage.mjs` | 삭제 |
| `src-tauri/src/lib.rs`(178) | 대체 — 대화상자/쓰기는 `rfd`·`std::fs`. Tauri 의존 제거 |
| `src-tauri/{Cargo.toml,tauri.conf.json,build.rs}` | 대체 — 워크스페이스 루트 `Cargo.toml` |
| `tests/tool_integrity_check.mjs`(300줄 규칙) | 대체 — Rust 모듈 크기 규칙으로 |

---

## 6. 단계별 계획

각 Phase는 **독립 산출물 + 게이트 통과**가 종료 조건이다. Phase 2 이후에는
`cargo run`으로 실제 창이 떠야 한다(매 Phase 말미에 눈으로 확인).

### Phase 0 — 기반 정비 ✅ (0.5일)

| # | 작업 | 산출물 | 상태 |
|---|---|---|---|
| 0.1 | 성능 베이스라인 계측 | `tools/bench_baseline.mjs`, 수치표(아래) | ✅ |
| 0.2 | 워크스페이스 전환 | 루트 `Cargo.toml` + `crates/{engine,platform,app}`, `cargo test` 18그린 | ✅ |
| 0.3 | 오라클 동결 선언 | `tools/oracle_manifest.mjs` + manifest(193파일, digest `de5de4f5…`) | ✅ |
| 0.4 | 픽스처 공용 로더 | `crates/engine/tests/common/mod.rs`, 그룹 크기 핀 | ✅ |
| 0.5 | `src/` 이동 | **계획 변경: 보류**(아래) | ⏸ |

**게이트 통과**: `cargo test --workspace` 18/18 · `npm test` 410/410(동결 확인) ·
`node tools/oracle_manifest.mjs --check` 그린 · `cargo run -p pixeldraw` 동작.

#### 0.5를 보류한 이유

`src/`를 `reference-app/`으로 옮기면 `tests/**`의 상대 import 수백 개가 한 번에
깨진다. 오라클은 **그 자리에서 그대로 도는 편이** 훨씬 값지다(`npm test` 410이
계속 같은 위치에서 검증됨). 따라서 이동은 폐기하고, 대신:

- `tools/oracle_manifest.mjs --check`가 드리프트를 잡는다(태그보다 정확함).
- 삭제는 Phase 6.3에서 한 번에 한다(그때는 이미 대체물이 검증된 뒤).

#### 0.1 측정 결과 — JS 베이스라인

`node tools/bench_baseline.mjs` · 2048×1088 · 64레이어 · Node v24.21.0 · median 기준.

| # | 작업 | median | min | max |
|---|---|---:|---:|---:|
| 1 | stroke pen=1 (20 세그) | 2.69 ms | 1.51 | 6.08 |
| 2 | stroke pen=8 (20 세그) | 44.9 ms | 43.9 | 57.5 |
| 3 | **stroke pen=64 (20 세그)** | **2785.7 ms** | 2463.3 | 2914.6 |
| 4 | strokePoint pen=64 (1점) | 0.097 ms | 0.082 | 0.348 |
| 5 | floodFill 빈 캔버스 | 146.2 ms | 138.0 | 153.2 |
| 6 | floodFill 빨강→초록 | 165.2 ms | 147.8 | 166.0 |
| 7 | applyShape ellipse 1024² fill | 20.4 ms | 19.2 | 24.3 |
| 8 | applyShape rrect 1024² r64 외곽 sw=4 | 11.1 ms | 10.7 | 26.9 |
| 9 | applyShape line 2048×1088 sw=8 | 4.22 ms | 3.79 | 10.2 |
| 10 | applyShape polygon 32각형 fill | 26.8 ms | 24.3 | 30.2 |
| 11 | **composite 2176 청크 × 64 레이어** | **391.2 ms** | 377.8 | 404.0 |
| 12 | **flattenToRgba 2048×1088** | **2002.3 ms** | 1944.9 | 2085.0 |
| 13 | exportPngBytes 2048×1088 | 2070.7 ms | 2054.8 | 2078.9 |
| 14 | documentToJson (저장) | 1909.7 ms | 1893.2 | 2199.8 |
| 15 | 저장 JSON 크기 | 7557 KB | | |

**읽어낸 것**:

- **pen=64 한 세그먼트가 약 139 ms.** 드래그 중 UI가 굶는다. 3번이 이전의
  1순위 타겟이며, "강력한 속도"가 체감되는 곳은 바로 여기다.
- flatten(12번)이 composite(11번)의 5배다. 픽셀당 배열 2개 할당 + `overBlend`
  호출 오버헤드가 원인이다. Rust에서는 둘 다 한 자버퍼에서 끝난다.
- 저장 JSON이 7.5 MB. `serialize`의 base64 청크 인코딩이 대부분이다.

#### 0.x 부산물 — 문서와 다른 실제 동작 2건

포트 도중 발견한 문서/코드 드리프트. 둘 다 **코드가 진실**이므로 코드를 따른다.

1. **에러 카탈로그가 21종이다.** `docs/contract.md` §3은 19종이라 적었지만
   `src/core/errors.js`는 `VIEWPORT_INVALID`·`VIEWPORT_OUT_OF_RANGE`를 더 갖고
   있다(`features/document/document.js` 뷰포트 검증).
   `crates/engine/src/errors.rs`가 21종으로 포팅했고 `CATALOG`가 목록을 핀한다.
2. **`GRID_MODES`가 문서에 없다.** `src/core/constants.js`에 4종
   (`off`·`unit`·`tile`·`pixel`)이 있으나 `docs/core.md` §3 표에는 없다.
   `crates/engine/src/constants.rs`에 함께 포팅했다.

### Phase 1 — 엔진 크레이트 (3~5일, 에이전트 3~4개 병렬) — ✅ 완료 (13/13)

`engine/`은 네이티브 `cargo test`로만 검증한다(wasm 불요).

| # | 작업 | 패리티 대상 | 게이트 | 상태 |
|---|---|---|---|---|
| 1.1 | `constants`·`errors` 21종·`events`·`pixel` | `tests/core_basic.test.mjs` | `cargo test core::` | ✅ |
| 1.2 | `blend`(`rnd`·`overBlend` 6모드·알파 모델) | `tests/blend_modes.test.mjs` · golden `composite` 8건 | 동일 기대값 | ✅ |
| 1.3 | `chunkstore`(`ChunkStore`·`PixelWriter`·`Changeset`) | `core_basic`·`oob_drawing` | 청크 diff 동일 | ✅ |
| 1.4 | `raster/brush`+`masks` | golden `brush`(14)·`ellipse`(15)·`rrect`(8)·`outline`(10)·`bresenham`(7)·`hashes` | 47건 + 해시 6건 | ✅ |
| 1.5 | `raster/segment` | golden `wide_line`(54) | 전건 | ✅ |
| 1.6 | `raster/polygon` | golden `polygon`(6)·`polygon_outline`(6)·`fillet`(5) | 전건 | ✅ |
| 1.7 | `raster/arc` | golden `arc`(2)·`chord`(2) | 허용오차 동일 | ✅ |
| 1.8 | `raster/gradient`·`text`·`atlas` | golden `gradient`(2)·`paste`(5) | 전건 | ✅ |
| 1.9 | `raster/composite_ops` | `recipe_*` 8파일 | 전건 | ✅ |
| 1.10 | `composite` + `kernels` | `render_grid`·`shape_*`·`autosave_blend` | 동치 불변 | ✅ |
| 1.11 | `io/png` | `png_cases` 5/5 | 동일 픽셀·동일 에러 | ✅ |
| 1.12 | `model` | `model_basic`·`model_history`·`e2e_session` | 전건 | ✅ |
| 1.13 | `io/serialize`·`vector`·`recipe`·`validate` | `io_basic`·Rust `recipe_parity` 39/39·schema | 왕복·validator·pixel parity | ✅ |

**게이트(Phase 1 종료)**: `cargo test --workspace` = 골든 201 + png 5 + Rust recipe 39/39 + model/session + view checks 전부 그린. `npm test` schema consistency도 통과.
실측 골든 커버리지(준비된 21개 그룹 전부, 197건 + 해시 6건):
brush 14 + ellipse 15 + rrect 8 + outline 10 + bresenham 7 + stroke 7 +
angle_snap 8 + resize 15 + drag_bbox 8 + unit_snap 6 + composite 8 + fill 3 +
wide_line 54 + polygon 6 + polygon_outline 6 + fillet 5 + gradient 2 + arc 2 +
chord 2 + paste 5 + hashes 6 = **201건**, png_cases 5/5.
남은 Phase 1 게이트 없음. 후속 작업은 Phase 2.1 이후 네이티브 캔버스 단계.

#### 1.4에서 나온 구현 노트

- **픽스처 하네스를 `src/fixtures.rs`로 옮겼다.** 처음엔 `tests/common/`에
  뒀는데, 단위 테스트가 통합 테스트와 같은 리더를 쓰려면 라이브러리 안에 있어야
  한다. 부수적으로 `serde_json`이 정규 의존성이 됐는데, 1.13(스키마 직렬화)에서
  원래 필요하므로 순서가 앞당겨진 셈이다.
- **큰 마스크는 `hashes` 그룹으로 검증한다.** brush 64·33, ellipse 128×64,
  rrect 200×120 외곽은 행을 인라인하기 너무 크다. 픽스처가 sha256를 갖고 있어
  `sha2` dev-의존성으로 재현해 확인한다(직접 구현했다가 100줄 낭비임을 인정하고
  크레이트로 교체).
- **`usize` 부호있는 뺄셈 금지.** `ellipseMask`의 `2x+1-w`는 왼쪽 반에서 음수.
  JS는 float라 통과하지만 Rust `usize`는 디버그에서 패닉한다. 전부 i64로.
- **`PixelSink` 트레이트로 스탬프 대상 분리.** `PixelWriter` 구현은 음수 좌표를
  버린다(캔버스 반대편으로 감싸지 않게). JS는 `writer.set`의 범위 무시에 기댔는데,
  `u32` 캐스팅을 그대로 쓰면 (−5,−5)가 (2043,1083)에 찍히는 사고가 났을 것이다.
  테스트로 못 박았다.

#### 1.5~1.6에서 나온 구현 노트

- **`Math.fround`은 `f32` 캐스팅이다.** 폴리곤 스캔라인의 `dx`와 교점 좌표는
  C `float`. 두 f32의 곱은 f64에 정확히 담기므로, f32 곱셈과 "정확한 곱의
  fround"가 비트 동일하다. 이 성질 덕에 `inter_x(y, e)` 헬퍼 하나로
  `Math.fround(Math.fround((y-y0)*dx) + x0)`를 그대로 옮길 수 있었다.
- **반올림이 네 종류다.** `round_up`/`round_down`(스캔라인 span 경계) ·
  `round_c`(C `roundf`) · `py_round`(Python half-to-even). 서로 바꾸면 결과가
  망가진다. 대비 테스트로 못 박았다.
- **`lineMask`는 최소 bbox를 돌려준다.** 골든 `wide_line`은 64×64 **전체 캔버스**
  기준이라, 테스트에서 마스크를 W×H에 블릿한 뒤 행을 비교한다.
- **픽스처 키 이름이 그룹마다 다르다.** `pts`(polygon 계열) vs `points`,
  `w/h` vs `W/H`(wide_line·stroke). 원본의 일관성 부족이며, 포팅에서 규칙을
  새로 만들지 않았다 — `points_from_json`은 두 키를 모두 받고 `wide_line`
  테스트만 대문자를 쓴다.

#### 1.1~1.3에서 나온 구현 노트

- **에러 코드가 21종.** 문서(19)와 코드가 달라 코드를 따랐다. `CATALOG` 상수가 핀.
- **`is_all_zero`는 `unsafe` 없이.** 워크스페이스가 `unsafe_code = "forbid"`.
  바이트 스캔으로 쓰면 LLVM이 같은 워드 단위 루프로 벡터화한다.
- **`forEachChunk` 순서는 키 오름차순(`cy*64+cx`), 행 우선이 아니다.**
  `BTreeMap`이 JS의 명시적 `sort`를 대신한다.
- **`PixelWriter.finish`가 빈 청크를 저장소에서 지운다.** `after = None`으로
  기록하므로 지운 영역은 파일에 흔적이 남지 않는다(JS 동일).
- **`ChunkDiff` 타입 별칭.** `(cx, cy, before, after)` 4-튜플이 여기저기 쓰여
  clippy의 복잡 타입 경고를 막기 위해 이름을 붙였다.

| # | 작업 | 패리티 대상 | 게이트 |
|---|---|---|---|
| 1.1 | `constants`·`errors`(19종)·`events`(7종)·`pixel`(pack/unpack/parseHex/toHex) | `tests/core_basic.test.mjs` | `cargo test core::` |
| 1.2 | `blend`(`rnd`·`overBlend` 6모드·알파 모델) | `tests/blend_modes.test.mjs` 384줄 전건 | 동일 기대값 |
| 1.3 | `chunkstore`(`ChunkStore`·`PixelWriter`·`changeSetBytes`·`pruneEmpty`·`resizeTo`) | `core_basic`·`oob_drawing` | 청크 diff 동일 |
| 1.4 | `raster/brush`+`masks`(brush 1·2·3px·ellipse·rrect·outlineRing) | golden `brush`/`ellipse`/`rrect`/`outline` | 201건 중 해당 건 |
| 1.5 | `raster/segment`(wide line → 회전 사각형 + 스캔라인) | golden `wide_line` 54건 | **전건** |
| 1.6 | `raster/polygon`(fill·outline·fillet) | golden `polygon`·`polygon_outline`·`fillet` | 전건 |
| 1.7 | `raster/arc`(arc·chord·near-parity 허용오차 재현) | golden `arc`·`chord` | missing/extras 바운드 동일 |
| 1.8 | `raster/gradient`·`text_mask`+글리프·`atlas` | golden `gradient`·`text_mask`·`atlas_assemble` | 전건 |
| 1.9 | `raster/composite_ops`(clip_alpha·erase·visor·sheen·hud_brackets) | `recipe_*` 테스트 8파일 | 전건 |
| 1.10 | `composite`(compositeChunk·samplePixel) + `kernels`(floodFillScanline·strokeSegment·applyShape) | `render_grid`·`shape_*`·`autosave_blend` | 동치 불변 |
| 1.11 | `io/png`(CRC32·필터0-4 디코드·필터0 인코드·에러 코드) | `png_cases` 5/5 | 동일 픽셀·동일 에러 |
| 1.12 | `model`(Document·Layer·Commands 6종·UndoManager·Session) | `model_basic`·`model_history`·`e2e_session` | 전건 |
| 1.13 | `io/serialize`·`serialize_vector`·`serialize_recipe`·`validate*` | `io_basic`·`recipe_parity` 39/39·`spec_consistency` | **바이트 동일 저장** |

**게이트(Phase 1 종료)**: `cargo test` = 골든 201 + png 5 + recipe 39/39 + view OK
전부 그린. JS 구현과 같은 픽스처를 소비하므로 **한 줄이라도 어긋나면 실패**.

> Phase 1의 징검다리: 1.2 완료 시 `reference/` 오라클과 JS 양쪽과 대조하는
> 3자 비교 테스트를 만든다(JS vs Rust vs PIL 정답). 이후 모든 파일에 적용.

### Phase 2 — 네이티브 창 + 픽셀 캔버스 (2~3일)

| # | 작업 | 산출물 |
|---|---|---|
| 2.1 | `app/` = eframe 뼈대. 창 제목 `Pixeldraw`, 최소 크기, DPR 인식 | 창이 뜨는 exe |
| 2.2 | 프레임버퍼 렌더: 합성 결과 → `TextureFilter::Nearest` 텍스처. `imageSmoothingEnabled=false` 동치 | 캔버스에 문서가 보임 |
| 2.3 | `view` 이식(줌·팬·클램프·fit·actualSize·visibleChunkRange 12단계 frozen) + `view_store` | `tools/view_check` 대응 테스트 |
| 2.4 | 합성 캐시(dirty 청크)·6ms 프레임 예산 이식 → 프레임버퍼 갱신 | Dirty 청크만 재합성 |
| 2.5 | 배경(체커·단색)·그리드(베벨 2톤 + `difference` 헤어라인)·테두리 1px → CPU 프레임버퍼 | 픽셀 단위 동일 |
| 2.6 | 입력 루프: 포인터(버튼·shift·alt·coalesced)·키보드·휠(누산기) → 이벤트 정규화 구조체 | `pointer_bindings` 동치 |
| 2.7 | 도구 배선: pen·eraser·shape 5종·fill·eyedropper·hand | **마우스로 그을 수 있음** |

**게이트**: 수동 스모크 문서(`docs/native_smoke.md`) — 새 문서·펜·지우개·
사각·타원·선·채우기·스포이트·손·줌/팬/리셋·Undo/Redo·PNG 저장·PNG 열기·
JSON 저장/열기 전부 동작.

### Phase 3 — UI 전면 이식 (3~5일)

`ui/**` 42파일 4,007줄 → egui 패널.

| # | 작업 | 원본 |
|---|---|---|
| 3.1 | 메뉴바(파일/편집/레이어/보기/도움) + 단축키 맵 151줄 | `ui/menubar/*`·`ui/toolbar/shortcuts.js` |
| 3.2 | 툴바 10종 아이콘 + 활성 상태 | `ui/toolbar/icons.js` |
| 3.3 | 옵션바: 도구별 동적 옵션(펜 크기·도형 채움/반경/비율·스냅·그라디언트·호·텍스트) | `ui/optionbar/panel_options.js` + `panel_*_options.js` 4파일 |
| 3.4 | 컬러 패널 8파일: 슬롯 2색·최근·팔레트·**색환**(HSV 수학 + 포인터)·필드(rgb/hex) | `features/color/panel_color_wheel.js` 312줄 |
| 3.5 | 레이어 패널: 목록·가시성·잠금·이름·불투명도·블렌드 6종·순서·병합·복제·삭제 | `features/layers/panel_layers.js` 297줄 |
| 3.6 | 히스토리 버튼·상태바(좌표·크기·줌·메시지) | `ui/menubar/history_buttons.js`·`ui/statusbar/statusbar.js` |
| 3.7 | 다이얼로그: 새 문서(크기/배경)·캔버스 리사이즈 | `ui/dialogs/dialog_new_doc.js`·`dialog_resize.js` |
| 3.8 | 토스트·툴팁·문자열 중앙화·접기 섹션·패널 리사이즈 | `ui/shared/*` 10파일 |
| 3.9 | 설정 저장(브라우저 localStorage → 앱 디렉터리 JSON) | `ui/shared/storage.js` |
| 3.10 | 자동저장(IndexedDB → 앱 디렉터리 단일 파일) | `io/idb_*.js` 대체 |

**게이트**: 위 수동 스모크 문서에 UI 항목 전부 추가 + 패리티: 기존 저장 파일을
열었을 때 레이어/색상/히스토리가 동일.

### Phase 4 — 자산 파이프라인 판단 (0.5일)

| 선택 | 내용 |
|---|---|
| A(권장) | `tools/recipe/*`·`tools/gen_fixtures.py`·`asset_work/`는 **개발 자산으로 유지**. 골든 픽스처 원본이므로 제거 불가 |
| B | 레시피 이식·내보내기를 Rust로 재구현(`engine/io/serialize_recipe.rs`에 이미 있음) |

`viewer/`(레이어 갤러리 34/34)는 앱 기능이 아니므로 제거. 문서에서도 삭제.

### Phase 5 — 성능 단계: 강력한 속도 (1~2일)

| # | 기법 | 대상 | 예상 |
|---|---|---|---|
| 5.1 | 채널 연산 `u32` 워드 단위 + `wide`(SIMD) | `overBlend`·`compositeChunk` | 2~4× |
| 5.2 | `rayon` 청크 병렬 합성 | 2048×1088 64레이어 | 레이어 수배 |
| 5.3 | 불투명+opacity1 고속경로를 **런 단위**로 확장(가로 런 레벨 스킵) | 평면 대역 | 케이스별 큰 이득 |
| 5.4 | 플러드필 분할(행 밴드별 `rayon`) | 전체 화면 채우기 | 4~8× |
| 5.5 | PNG 인코드 병렬 + `miniz_oxide` 레벨 조정 | 대형 저장 | 3× |
| 5.6 | 텍스처 갱신 최소화(변경 청크만 GPU 업로드) | 매 프레임 | 프레임 시간 안정 |
| 5.7 | 프로파일: `tracing` + 프레임 타이밍 HUD(개발 빌드) | 회귀 방지 | — |

**성능 목표(2048×1088·64레이어 기준, Phase 0.1 실측 베이스라인 대비)**:

| 작업 | JS median(실측) | 목표(Rust) | 배수 |
|---|---:|---:|---:|
| stroke pen=1 (20 세그) | 2.69 ms | ≤ 0.3 ms | ~9× |
| stroke pen=8 (20 세그) | 44.9 ms | ≤ 2 ms | ~22× |
| **stroke pen=64 (20 세그)** | **2785.7 ms** | **≤ 30 ms** | **~93×** |
| floodFill 전체 화면 | 146.2 ms | ≤ 10 ms | ~15× |
| applyShape ellipse 1024² fill | 20.4 ms | ≤ 1 ms | ~20× |
| applyShape polygon 32각형 fill | 26.8 ms | ≤ 2 ms | ~13× |
| composite 2176 청크 × 64 레이어 | 391.2 ms | ≤ 2 ms | ~195× |
| flattenToRgba 2048×1088 | 2002.3 ms | ≤ 3 ms | ~670× |
| exportPngBytes 2048×1088 | 2070.7 ms | ≤ 40 ms | ~52× |
| documentToJson (저장) | 1909.7 ms | ≤ 120 ms | ~16× |

배수는 참고값이며, **median 절대치가 계약**이다. 실측이 목표를 넘기면 Phase 5가
끝난 것이 아니다(5.7 프로파일로 병목을 찾아 5.1~5.6을 다시 돈다).

### Phase 6 — 절삭·패키징·문서 (1일)

| # | 작업 |
|---|---|
| 6.1 | `cargo build --release` → 단일 `pixeldraw.exe`. 아이콘(`make_icon` 결과 재사용) |
| 6.2 | 설치 파일(NSIS) |
| 6.3 | `src/`·`index.html`·`run.py`·`tools/{stage,bundle,check_stage}_*.mjs`·`src-tauri/` **삭제** |
| 6.4 | `contract.md` 갱신(§4 DOM 금지 → `engine/` 순수성, §5 진입점 = exe, §2-9 의존성 0 → 런타임 의존 0) |
| 6.5 | `README.md`·`docs/README.md`·`docs/desktop.md` 폐기 or 대체 |
| 6.6 | `docs/core.md`·`render.md`·`model.md`·`io.md`·`tools.md`·`ui.md` → 크레이트 기준으로 재작성 |
| 6.7 | CI 게이트: `cargo test`(골든+패리티) + `cargo clippy -D warnings` + `cargo fmt --check` |

---

## 7. 검증 전략(패리티)

### 게이트 전체

| 게이트 | 현재 | 이전 후 |
|---|---|---|
| 래스터 골든 | parity 201건 | `cargo test raster_golden` **동일 픽스처** |
| 레시피 합성 | recipe 39/39 | `cargo test recipe_parity` 동일 픽스처 |
| PNG | png_cases 5/5 | `cargo test png_cases` 동일 픽스처 |
| 뷰 수학 | view_check OK | `cargo test view` |
| 스키마 | schema_selfcheck | `cargo test schema` |
| 저장 호환 | — | **신설**: 기존 `.json`·PNG 39개 열어 → 동일 레이어/픽셀/히스토리 |
| UI | npm 410(일부) | 수동 스모크 + 회귀 골든 PNG |

### 3자 비교 원칙

```text
        ┌─ 입력(마스크 파라미터·픽셀 버퍼·도형 좌표)
        │
  오라클(reference/raster_ref.mjs, PIL 정답)
        │      ↘
        │        JS(src/ 또는 reference-app/)  ← 이전 기간의 기준 구현
        │      ↙
   Rust(engine/)  ── 같은 출력이면 통과. 어긋나면 Rust를 고친다.
```

- 골든 픽스처는 **PIL 오라클 대 JS**로 만들어졌다. 따라서 Rust는 최종적으로
  **JS 동작**과 같아야 하며, 그것은 곧 오라클과도 같다(arc·chord의 근사 제외).
- 참조 방향은 **오라클 → JS → Rust**로만 흐른다. Rust를 먼저 고치고 JS를 맞추는
  역방향은 금지한다(오라클 오염).

### 절대 금지 항목(리그레션 체크)

- `rnd(v)` = `Math.floor(v + 0.5)`의 Rust 이식은 `(v + 0.5).floor()`.
  음수 입력 경로가 생기면 `f64::floor`/`i64` 캐스팅 선택을 테스트로 못 박는다.
- `overBlend`의 연산 순서(`sa → oa → inv → 채널 3회 → a`)와
  `blend` 플래그(`mode !== "normal" && da > 0`) **그대로**.
- `chunkKey = cy*64 + cx` u64. `CHUNK_LEN=4096` 하드코딩 금지, 상수 참조.
- 에러는 19종 카탈로그 외 코드 금지(테스트가 `ERROR_CODES`와 대조).
- 레이어/도형 ID는 카운터 기반(`layer-0001`). 무작위 금지.

---

## 8. 리스크 레지스터

| # | 리스크 | 확률 | 영향 | 대응 |
|---|---|---|---|---|
| R1 | **UI 공수 폭발.** UI·패널 3,675줄이 이전 중 가장 크다 | 높음 | 일정 | Phase 2에서 캔버스 먼저 동작시켜 **도구로 쓸 수 있는 상태**를 P2 종료 전에 확보. §6 Phase 3 순서를 필수→부가로 배치해 절반만 이식해도 쓸 수 있게 한다 |
| R2 | 부동소수/패킹 드리프트로 골든 실패 | 중 | 품질 | Phase 1을 함수 단위로 쪼개고, 1.2에서 3자 비교 하네스를 먼저 만든다. 실패 시 해당 함수만 격리 |
| R3 | 대체 deflate로 저장 바이트 변경 | 높음 | 낮음 | 게이트가 **디코드 픽셀 비교**이므로 통과. 단 저장→열기 라운드트립 테스트로 확인 |
| R4 | 텍스트 도구 글리프 이식 누락 | 중 | 중 | `tools/recipe/text_glyphs.json`을 그대로 임베드(`include_str!`)하고 글리프 래스터는 1:1 포팅 |
| R5 | 색환/HSV 수학 미세 차이 | 중 | 낮음 | `view_ref.mjs`와 같은 방식으로 색환 레퍼런스 테스트 신설 |
| R6 | egui가 줌 32배 Nearest 블릿에 부적합 | 낮음 | 중 | Phase 2.2에서 조기 검증. 실패 시 캔버스만 `pixels`+`wgpu`로 분리(egui는 그대로) |
| R7 | IME(한글 입력) 미지원 | 중 | 중 | 텍스트 도구의 한글 입력은 egui의 텍스트 편집에 위임. 라틴 문자·기호는 글리프 세트와 충돌 여부 확인 |
| R8 | Undo 의미론(스트로크 병합·바이트 예산) 차이 | 중 | 중 | `model_history.test.mjs` 200줄을 Rust 테스트로 그대로 옮긴다 |
| R9 | 이전 중 기능 추가 욕구(스코프 크립) | 높음 | 일정 | Phase 0.3 동결 태그 + "이전 중 신규 기능 금지" 원칙 |
| R10 | WebView2 제거로 일부 사용성 손실(링크·드래그앤드롭) | 낮음 | 낮음 | 네이티브 파일 드롭은 `winit`이 지원. 외부 링크 열기는 `open` 크레이트 |

---

## 9. 문서·규칙 변경 목록

| 문서 | 변경 |
|---|---|
| `contract.md` | §4 DOM 금지 구역 → `engine/**`(의존 방향 규칙은 유지). §5 진입점: `run.py` → `pixeldraw.exe`. §2-9 "npm/CDN/빌드 없음" → "런타임 의존 0(네이티브)". §1-1 블렌드 수식·골든 **그대로** |
| `README.md` | 실행 방법 `cargo run --release` / exe. 검증 명령 `cargo test` |
| `docs/README.md` | 모듈 문서 6종(`core`·`model`·`io`·`render`·`tools`·`ui`)을 Rust 기준으로 재작성하고 본 문서를 목록에 등록 |
| `docs/desktop.md` | 폐기(네이티브가 유일 경로) |
| `docs/core.md` | 크레이트/모듈 서술 갱신. 상수 테이블은 그대로 |
| `docs/render.md` | 캔버스 렌더 → 프레임버퍼 서술로 |
| `docs/model.md`·`io.md`·`tools.md`·`ui.md` | Rust 기준 재작성 |
| `docs/native_plan.md` | 본 문서. 각 Phase 완료 시 상태 갱신 |
| `Cargo.toml` 규칙 | `cargo clippy -- -D warnings` 그린, `rustfmt` 그린 |
| 줄 수 규칙 | `src/` 300줄 규칙 → 엔진/앱 모듈 400줄 가이드(폴리곤 래스터 예약 면제 유지) |

### 검증 명령(이전 후)

```bash
cargo test --workspace        # 골든 201 + png 5 + recipe 39/39 + 모델 + 히스토리
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
node tools/oracle_manifest.mjs --check   # 오라클(JS 원본)이 움직였는지
cargo build --release         # crates/app → pixeldraw.exe
```

---

## 10. 마일스톤

```text
M0  기반 ────── 0.5일 ── 워크스페이스·동결·베이스라인
M1  엔진 ────── 3~5일 ── cargo test = 골든201+png5+recipe39/39   ★ 가장 중요
M2  캔버스 ──── 2~3일 ── 창에서 펜/도형/채우기로 그림
M3  UI ──────── 3~5일 ── 패널·메뉴·다이얼로그·컬러휠·레이어·설정·자동저장
M4  파이프라인 판단 ─ 0.5일
M5  속도 ────── 1~2일 ── SIMD·rayon·목표 수치 달성
M6  절삭 ────── 1일   ── 단일 exe·문서·CI
                 ≈ 11~17일(에이전트 병렬 기준 2~3주)
```

**조기 가치 지점**: M2 말미에 이미 "그릴 수 있는 오프라인 exe"가 존재한다.
M3는 그 위의 완성도 작업이다. 일정이 늘어나도 M2는 반드시 지킨다.

> 일정이 밀리는 주범은 타수가 아니라 **게이트 재돌파**다. 특히 M1. M1이 그린이면
> M2~M6은 매핑 작업이라 밀릴 이유가 없다. 반대로 M1을 대충 넘기면 M5에서
> 성능도, 저장 호환도 동시에 깨진다.

---

## 11. DoD(완료 정의)

- [ ] `cargo build --release` → `pixeldraw.exe` 단일. WebView2·Node·Python 없이 실행
- [ ] `cargo test` 전부 그린(골든 201·png 5·recipe 39/39·view·schema·모델·히스토리)
- [ ] 기존 저장 파일(래스터 JSON·벡터 JSON·레시피 JSON·PNG) **열고 다시 저장**해도
      레이어·블렌드·불투명도·히스토리·픽셀이 동일
- [ ] 수동 스모크 문서 항목 전부 통과
- [ ] §6 Phase 5 성능 목표 전부 달성(베이스라인 대비)
- [ ] `src/`·`index.html`·`run.py`·`src-tauri/` 삭제, 문서 갱신 완료
- [ ] `cargo clippy -D warnings`·`cargo fmt --check` 그린
- [ ] 오프라인(에어갭) 환경에서 설치 파일로 설치 후 전 기능 동작 확인

---

## 12. Handoff

- Wrote: `docs/native_plan.md`
- Read: `README.md`·`docs/{contract,core,render,desktop,README}.md`·`src/**` 중
  핵심 24개·`src-tauri/src/lib.rs`·`tools/parity_check.mjs`·`reference/raster_ref.mjs`
- 기반 사실: Rust 1.99 + `wasm32`(미사용) 툴체인 설치, VS Community 2026 설치
  (`cl`은 PATH에 없음 — 네이티브는 MSVC 링커를 `cargo`가 자동 탐색)
- 공수 전제: **에이전트 병렬 생산**(본 저장소 21,726줄/1주 관측). §0-1 참조.
- Next: **Phase 0.1 베이스라인 계측**과 **Phase 0.2 워크스페이스 전환**부터 시작.
  순서를 바꾸지 말 것(베이스라인 없이는 Phase 5 목표가 검증 불가).
