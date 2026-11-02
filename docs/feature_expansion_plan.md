# Pixeldraw 기능 확장 구현 계획

> 작성일: 2026-10-21  
> 대상: Pixeldraw 네이티브 Rust 트리(`crates/engine`, `crates/platform`, `crates/app`)  
> 문서 성격: **구현 계획서**. 코드 전문을 붙이지 않고, 다른 AI가 바로 작업할 수 있도록 모듈·공개 시그니처·동작·의존성·수용 기준을 고정한다.  
> 범위: 사용자 요청 4개를 포함한 **총 24개 기능 제안**. 이 문서는 기능을 구현하지 않으며, 계획과 계약만 정의한다.

---

## 0. 한눈에 보는 결론

- 사용자가 요청한 4개 기능은 **F01~F04**로 고정하고 모두 **P0(최우선)**으로 둔다.
  1. **F01 아틀라스 저작 워크벤치** — 그리드/슬롯 편집, 셀 단위 드로잉, 슬라이싱, PNG `dtAt` 왕복
  2. **F02 시작 레이어 프리셋** — 새 문서용 스타터 스택 + 기존 문서에 적용
  3. **F03 대칭 드로잉** — 수직/수평/사분면/방사형, 브러시·도형·채우기·그라디언트 공통 적용
  4. **F04 도형 프리셋** — 하트, N각 별 등 자주 쓰는 도형을 기존 폴리곤/도형 래스터 경로로 생성
- 나머지 20개는 편집 코어, 제작/레이어, 고급 워크플로로 묶는다.
- **총 24개**이며, P0 4개 / P1 9개 / P2 7개 / P3 4개로 배분한다.
- 선행 작업(M0) 없이 F01~F04를 바로 건드리면 액션 중복, 다이얼로그 취소 버그, 스키마 충돌, 오버레이 미구현 때문에 같은 코드를 두 번 고치게 된다. **M0를 먼저 끝낸다.**

### 0-1. 이 문서를 읽는 순서

1. §2 기준선에서 현재 코드가 무엇을 제공하는지 확인한다.
2. §3 인벤토리에서 기능 ID와 단계를 확인한다.
3. §5 공통 기반(M0)을 먼저 구현한다.
4. §6 이후 기능 카드 중 담당 기능만 읽고, 카드의 **수용 기준**을 테스트로 옮긴다.
5. §7 마일스톤 게이트를 통과한 뒤 다음 단계로 넘어간다.

### 0-2. 절대 규칙

- **코드가 문서보다 우선한다.** 이 문서와 실제 코드가 충돌하면 코드를 읽고, 문서를 고친 뒤 진행한다.
- **은퇴한 JS 트리(`src/`, `tests/`, `reference/`)를 수정하거나 복제하지 않는다.** 네이티브 Rust 트리만 대상으로 한다.
- 기능 하나는 가능한 한 하나의 PR/작업 단위로 끝낸다. 여러 기능을 한 번에 켜지 않는다.
- 공개 API를 바꾸면 이 문서의 시그니처를 갱신하고, 기존 호출부와 테스트를 함께 고친다.
- 스키마 변경은 §5.3의 **단일 마이그레이션 게이트**를 통해서만 한다. 기능별로 즉흥적으로 `schema_version`을 올리지 않는다.
- 사용자에게 보이는 문자열은 기존 관례대로 한국어로 쓴다. 코드 식별자·에러 코드는 영어를 유지한다.
- `unsafe` 금지, 모듈 300줄 권장/400줄 하드리밋, 기존 `cargo fmt`·`clippy -D warnings` 규칙을 지킨다.
- 성능 민감 경로(합성, 브러시, 채우기, 프레임 업로드)는 기능 추가 후에도 기존 프레임 예산과 청크 단위 캐시를 유지한다.

---

## 1. 문서의 계약 표기법

- `기존`: 현재 저장소에 실제로 존재하는 모듈/함수.
- `신규`: 이 계획에서 새로 만들 파일/타입/함수.
- `변경`: 기존 시그니처나 구조를 바꾸는 것. 호출부 수정 범위를 함께 적는다.
- 시그니처는 **의도된 공개 표면**이다. 구현 중 더 나은 형태가 필요하면 이유를 기록하고 이 문서를 갱신한다.
- 수용 기준은 `cargo test`로 옮길 수 있는 문장으로 쓴다. "잘 된다" 같은 주관적 표현은 금지한다.

---

## 2. 기준선: 현재 Pixeldraw가 제공하는 것

### 2-1. 크레이트와 의존 방향

| 크레이트 | 역할 | 규칙 |
|---|---|---|
| `crates/engine` | 픽셀 엔진, 문서/레이어/히스토리, 래스터, IO, 합성 | OS/UI/네트워크 의존 금지 |
| `crates/platform` | 파일 다이얼로그, 앱 데이터 경로, 설정/자동저장 파일, 로깅 | OS 접근은 여기서만 |
| `crates/app` | egui/eframe 셸, 도구, 패널, 캔버스 프레임 | 엔진을 소비, 역의존 금지 |

의존 방향은 `app → platform/engine`, `platform → (engine 없음)`, `engine → 없음`을 유지한다. 새 기능이 이 방향을 뒤집으면 설계를 다시 한다.

### 2-2. 현재 문서/캔버스 제약

- 캔버스는 32px 청크 정렬. `MIN_SIZE_PX=32`, `MAX_W_PX=2048`, `MAX_H_PX=1088`.
- 레이어는 **평면 `Vec<Layer>`**, 최대 64개. 그룹/마스크/조정 레이어 없음.
- 문서 스키마는 `SCHEMA_VERSION = "2.0.0"`. 검증기는 **정확히 2.0.0만** 허용하고, 허용 키 밖의 필드를 거부한다.
- 설정은 9개 키(`primaryColor`, `secondaryColor`, `penSize`, `activeTool`, `gridMode`, `snapUnit`, `shapeFill`, `shapeRadius`, `shapeLockAspect`)와 `TOOL_IDS` 14개가 동결되어 있다.
- `Session::stroke_begin/stroke_with/stroke_commit/stroke_cancel`이 다중 이벤트 드로잉의 안전한 경로다. 픽셀은 반드시 이 경로로 쓴다.
- 표시 프레임은 `CompCache` + `display::draw_display_into` + `Framebuffer` + `CanvasTexture` 업로드 추적 구조다.
- 현재 도구: 펜, 지우개, 채우기, 스포이트, 선, 사각형, 둥근 사각형, 타원, 다각형, 호, 현, 방사형 그라디언트, 텍스트, 손.
- 현재 없는 것: 선택/변형, 클립보드, 대칭, 도형 프리셋, 레이어 프리셋, 그룹, 마스크, 애니메이션, 타일 모드, 가이드/눈금자, 참조 이미지, 사용자 브러시, 필터, 사용자 팔레트, 다중 문서 탭, 명령 팔레트, 버전 스냅샷.

### 2-3. 재사용해야 하는 기존 자산

| 자산 | 위치 | 이 계획에서의 용도 |
|---|---|---|
| `ShapeSpec`/`ShapeKind`/`FillMode`/`apply_shape` | `crates/engine/src/raster/shape_raster.rs` | F04 도형 프리셋, F03 대칭 도형, F05 선택 채우기 |
| `polygon_mask`/`polygon_outline_mask`/`ellipse_mask`/`rrect_mask` | `crates/engine/src/raster/{polygon,mask}.rs` | F04, F05, F13 |
| `stroke_point`/`stroke_segment`/`stamp_brush`/`PixelSink` | `crates/engine/src/raster/brush.rs` | F03, F10, F20 |
| `flood_fill_scanline` | `crates/engine/src/kernels.rs` | F03 채우기 대칭, F21 패턴 채우기 |
| `flatten_to_rgba` | `crates/engine/src/io/export.rs` | F01, F13, F17 |
| `CompCache`/`composite_chunk` | `crates/engine/src/{comp_cache.rs,composite.rs}` | F11 그룹, F12 마스크, F16 애니메이션, F23 조정 레이어 |
| `Session::stroke_*`/`PaintCommand` | `crates/engine/src/model/{session,commands}.rs` | 모든 픽셀 편집의 단일 히스토리 경로 |
| `AtlasMetadata`/`dtAt` | `crates/app/src/file_ops.rs` | F01 메타데이터 v2 |
| `OverlayOp::MaskBlit` | `crates/app/src/tools/overlay.rs` | F05 선택 오버레이, F01 셀 미리보기 |
| `SettingsStore` | `crates/app/src/ui/settings_store.rs` | F03/F10/F19 등 새 설정 영속화 |

### 2-4. 지금 고쳐야 하는 기존 결함 (M0에서 처리)

1. **액션 디스패처 중복**: `crates/app/src/shell.rs::dispatch_action`와 `crates/app/src/ui/menubar.rs::dispatch_action`가 따로 존재한다. `Cut/Copy/Paste`, `MergeVisible`, `FlattenImage` 등이 한쪽에서만 no-op이다. 기능을 추가하기 전에 **단일 라우터**로 합친다.
2. **새 문서 다이얼로그의 취소 버그**: `crates/app/src/ui/dialogs.rs`의 후처리 함수는 다이얼로그가 닫히면 `open` 값과 무관하게 새 문서를 적용한다. `취소`/Esc도 문서를 바꾼다. **명시적 `DialogOutcome`**으로 바꾼다.
3. **아틀라스 가져오기/내보내기 불일치**: 내보내기는 현재 문서 캔버스를 한 셀로 보고, 가져오기는 시트 크기의 문서를 만든다. F01에서 **셀 크기 문서 + 슬롯당 레이어**로 왕복을 고친다.
4. **`MaskBlit` 미구현**: `mask_to_blit`/`packed_to_blit`가 `None`을 반환한다. F05의 올가미/선택 미리보기와 F01의 셀 강조에 필요하므로 M0에서 구현한다.
5. **스키마 추가 필드 거부**: `io/validate/structural.rs`의 `check_no_extra`와 `semantic.rs`의 하드코딩된 `2.0.0` 때문에 새 문서 메타데이터를 바로 넣을 수 없다. M0에서 버전/마이그레이션 하네스를 만든다.
6. **`ToolCx` 리터럴 52곳**: 필드를 추가하면 전체 호출부가 깨진다. 대칭/브러시/선택 상태는 **`ToolCx` 메서드**로 노출하고, 실제 상태는 `Session`에 둔다.
7. **`new_document`의 단일 레이어 가정**: F02가 시작 레이어 스택을 만들려면 `NewDocumentOptions`/`DocumentSpec` 생성 경로를 확장해야 한다.
8. **오버레이/프레임 업로드의 보수적 bbox**: 선택/가이드/타일 미리보기를 추가하면 `overlay_bbox`와 `UploadTracker`가 새 오버레이를 놓치지 않도록 확장한다.

---

## 3. 기능 인벤토리 (24개)

우선순위 정의:
- **P0**: 사용자 요청, 첫 릴리스 필수.
- **P1**: 핵심 편집/제작 워크플로, 다음 릴리스 필수.
- **P2**: 제작 생산성, 있으면 제품 완성도가 크게 오른다.
- **P3**: 고급/대규모, 후속 릴리스.

| ID | 기능 | 사용자 요청 | 우선순위 | 단계 | 규모 | 핵심 의존 |
|---|---|---|---|---|---|---|
| F01 | 아틀라스 저작 워크벤치 | ✅ | P0 | M1 | L | M0, `dtAt` |
| F02 | 시작 레이어 프리셋 | ✅ | P0 | M1 | M | M0, `NewDocumentOptions` |
| F03 | 대칭 드로잉 | ✅ | P0 | M1 | L | M0, `PixelSink` |
| F04 | 도형 프리셋(하트/N각 별) | ✅ | P0 | M1 | M | `apply_shape` |
| F05 | 선택·변형 |  | P1 | M2 | L | M0 `MaskBlit` |
| F06 | 클립보드·플로팅 붙여넣기 |  | P1 | M2 | M | F05 |
| F07 | 가이드·눈금자·스냅 |  | P1 | M2 | M | M0 스키마 |
| F08 | 심리스 타일 모드 |  | P1 | M2 | L | M0, F03/F05 |
| F09 | 참조 이미지 오버레이 |  | P1 | M2 | S | `decode_png` |
| F10 | 사용자 브러시 팁·동역학 |  | P1 | M2 | L | `PixelSink`, F03 |
| F11 | 레이어 그룹 |  | P1 | M3 | L | M0, v3 스키마 |
| F12 | 레이어 마스크 |  | P1 | M3 | L | F11, v3 스키마 |
| F13 | 필터·이펙트 스위트 |  | P1 | M3 | L | F05 |
| F14 | 그라디언트 스튜디오 |  | P2 | M3 | M | 기존 `raster::gradient` |
| F15 | 팔레트 관리·색 추출 |  | P1 | M3 | M | 기존 팔레트 패널 |
| F16 | 애니메이션 워크스페이스 |  | P2 | M3 | XL | F11, F17 |
| F17 | 내보내기 스튜디오 |  | P1 | M3 | M | F01, F11/F16 |
| F18 | 다중 문서 탭 |  | P2 | M4 | XL | M0 라우터 |
| F19 | 명령 팔레트·단축키 편집 |  | P2 | M4 | M | M0 라우터 |
| F20 | 픽셀 퍼펙트 드로잉 보조 |  | P2 | M4 | M | F03/F10 |
| F21 | 패턴 채우기·텍스처 라이브러리 |  | P2 | M4 | M | F05, F10 |
| F22 | 타일셋·나인슬라이스 저작 |  | P3 | M4 | L | F01, F08 |
| F23 | 비파괴 조정 레이어 |  | P3 | M4 | XL | F11/F12, v3 |
| F24 | 버전 히스토리·스냅샷 |  | P2 | M4 | M | 기존 autosave |

### 3-1. 사용자 요청 4개와의 대응

- **F01** = 아틀라스 드로잉/저작.
- **F02** = 시작 레이어 프리셋 UI/기능.
- **F03** = 대칭 도구.
- **F04** = 하트/N각 별 등 도형 프리셋.

이 4개는 서로 독립적으로 구현할 수 있지만, 모두 M0의 라우터/다이얼로그/스키마/오버레이 기반을 공유한다. **M1 게이트에서 4개 모두 수용 기준을 통과해야 P0 완료**로 본다.

---

## 4. 로드맵

```text
M0 기반 정리
 ├─ ActionRouter 단일화
 ├─ DialogOutcome 도입 (취소 버그 수정)
 ├─ 스키마 v2.1 마이그레이션 하네스
 ├─ MaskBlit 구현
 ├─ CompositeCommand(다중 명령 1스텝)
 └─ 공통 테스트/게이트
        │
        ▼
M1 P0: 사용자 요청 4개
 ├─ F01 아틀라스 워크벤치
 ├─ F02 레이어 프리셋
 ├─ F03 대칭
 └─ F04 도형 프리셋
        │
        ▼
M2 P1: 편집 코어
 ├─ F05 선택·변형
 ├─ F06 클립보드
 ├─ F07 가이드·스냅
 ├─ F08 타일 모드
 ├─ F09 참조 이미지
 └─ F10 사용자 브러시
        │
        ▼
M3 P1/P2: 제작·레이어
 ├─ F11 그룹
 ├─ F12 마스크
 ├─ F13 필터
 ├─ F14 그라디언트
 ├─ F15 팔레트
 ├─ F16 애니메이션
 └─ F17 내보내기
        │
        ▼
M4 P2/P3: 고급 워크플로
 ├─ F18 다중 탭
 ├─ F19 명령 팔레트
 ├─ F20 픽셀 퍼펙트
 ├─ F21 패턴
 ├─ F22 타일셋
 ├─ F23 조정 레이어
 └─ F24 버전 히스토리
```

### 4-1. 마일스톤 게이트

| 게이트 | 통과 조건 |
|---|---|
| **G0** | `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` 전부 그린. 기존 스키마 2.0.0 문서가 그대로 열린다. |
| **G1** | F01~F04 수용 기준 통과. 아틀라스 왕복, 프리셋 적용/취소, 대칭 1회 실행 취소, 하트/N각 별 렌더가 모두 테스트로 고정된다. |
| **G2** | F05~F10 수용 기준 통과. 선택/클립보드/가이드/타일/참조/브러시가 서로의 상태를 오염시키지 않는다. |
| **G3** | v3.0.0 스키마 마이그레이션, 그룹/마스크/애니메이션 저장·복원, 화면/PNG/스포이트 합성 일치. |
| **G4** | 고급 기능별 플래그가 꺼진 상태에서도 G3 문서가 열리고, 기능을 켰을 때만 새 UI/데이터가 활성화된다. |

---

## 5. M0 공통 기반 — 완료 요약

M0는 M1~M4의 선행 조건이다. 상세 설계는 코드와 `docs/progress/log.md`가 기준이고, 여기서는 상태와 후속만 남긴다.

| 항목 | 상태 | 비고 |
|---|---|---|
| 5-1 ActionRouter | 🟡 | `ui/action_router.rs` 신설, menubar 경유. `shell.rs`의 자체 `dispatch_action`은 후속 통합. |
| 5-2 DialogOutcome | 🟡 | `Dialog.confirmed`로 취소/Esc 적용 차단. outcome enum 분리는 후속. |
| 5-3 스키마 2.1 | ✅ | `migrate_document`·`validate_supported_schema`, 읽기 `2.0.0/2.1.0`. 새 optional 필드(`atlas` 등)는 각 기능 카드에서 추가. |
| 5-4 MaskBlit | ✅ | `tools/overlay.rs`: 마스크→packed RGBA, `alpha_scale`은 알파에만. |
| 5-5 CompositeCommand | ✅ | `model/commands/composite.rs`: apply 순서, revert 역순, effects 병합, 실패 시 롤백. |
| 5-6 ToolCx 확장 원칙 | ✅ | 도구 공통 상태는 `Session`, `ToolCx`는 메서드만. |
| 5-7 게이트 | ✅ | 2026-10-27: workspace **651 green**, clippy/fmt green. |

**후속**: 5-1/5-2는 F01/F05에서 라우터·다이얼로그가 늘어날 때 함께 정리한다. 스키마 2.1의 `atlas` 필드는 ATL-1(`atlas_plan.md`)에서 추가한다.

---

## 6. 기능 카드

### F01. 아틀라스 저작 워크벤치

- **우선순위/단계:** P0 · M1 · L
- **사용자 요청:** ✅ 아틀라스 드로잉/저작

#### 현재 상태 (2026-10-27)

**구현 완료 — 설계는 초안과 다르다.** 초안의 `Document.atlas` + `create_atlas_document`/`convert_to_atlas`/`slice_png_grid` 대신, 레이아웃을 UI/CLI가 소유하고 PNG `dtAt` v2로 왕복하는 방식으로 먼저 완성했다.

- **엔진**: `pixel_engine::atlas` — `AtlasLayout`/`AtlasCellSpec`, 레이어/합성 모드, 64셀·셀당 16레이어·64 Mi px, 검증, aspect-fit 셀 베이크.
- **메타데이터**: `dtAt` v2 writer/reader + v1 읽기, `encode_png_large`/`decode_png_large`.
- **GUI**: 파일 → 아틀라스, 모드/열/셀 크기/프리뷰/포커스/선택, 슬롯 편집, PNG 저장, PNG 가져오기(셀 크기 문서·슬롯당 1레이어).
- **CLI**: `pixeldraw-cli atlas spec|plan|export|inspect|render-cell`.
- **검증**: workspace **651 green**, clippy/fmt 그린 (2026-10-27).

**남은 작업**: [`atlas_plan.md`](atlas_plan.md)의 ATL-1~ATL-9.

- **P0**: 레이아웃 영속화(JSON), PNG 가져오기에서 슬롯 구조 복원.
- **P1**: 메타데이터 없는 PNG 그리드 슬라이스, 패딩/마진, 셀 편집 점프, 16개 초과·비정렬 셀 가드.
- **P2**: 슬롯 undo, 슬롯 UX(드래그/복제/검증), 대형 시트 성능.

**수용 기준 상태**

| 기준 | 상태 |
|---|---|
| 3슬롯(2병합) 내보내기→가져오기: 셀 크기 문서·이름/순서 유지 | 🟡 슬롯당 1레이어로 베이크, 슬롯 구조는 미복원 |
| 각 셀 픽셀 = 원본 슬롯 flatten | ✅ 동일 셀 크기 기준 |
| `dtAt` v1 열기, v1→v2 재저장 | ✅ |
| 슬롯 추가/삭제/재정렬/병합 1 undo | ⬜ ATL-7 |
| 패딩/마진 반영, 초과 시 미저장 | ⬜ ATL-4 |
| 메타데이터 없는 PNG 슬라이스 | ⬜ ATL-3 |
| JSON 저장/열기와 PNG 저장/열기가 같은 레이아웃 | ⬜ ATL-1 |

**위험/주의**: 32px 미만 셀은 계속 거부한다. `Document`에 atlas 필드를 추가할 때 기존 2.0.0 문서가 그대로 열려야 한다. 병합 슬롯은 PNG에 베이크되므로 소스 레이어 복원은 별도 저장 경로가 필요하다.

---

### F02. 시작 레이어 프리셋

- **우선순위/단계:** P0 · M1 · M
- **사용자 요청:** ✅ 스타터 레이어 프리셋 UI/기능

#### 현재 상태와 문제

- `NewDocumentOptions`는 캔버스/배경/이름만 받고, 새 문서는 항상 `Layer 1` 하나를 만든다.
- 레이어 패널에는 추가/복제/삭제/병합/순서만 있고, 프리셋/템플릿 개념이 없다.
- 새 문서 다이얼로그는 배경 2종만 선택한다.

#### 목표

새 문서를 시작할 때 픽셀 아트 작업별 **레이어 스택**을 한 번에 만들고, 기존 문서에도 같은 스택을 적용한다. 프리셋은 "빈 레이어 이름 묶음"이 아니라 배경 채움/블렌드/불투명도/잠금까지 포함한 재현 가능한 데이터여야 한다.

#### UX 진입점

- `새 문서` 다이얼로그: 프리셋 썸네일/드롭다운, 프리셋 설명, "레이어 미리보기".
- `레이어 > 레이어 프리셋 적용…`: 현재 문서의 활성 레이어 위에 적용.
- 프리셋 목록(초기 6종):
  - `empty` — 기본 1레이어
  - `pixel_sprite` — `배경`, `캐릭터`, `그림자`, `효과`
  - `character_shade` — `베이스`, `명암`, `하이라이트`, `외곽선`
  - `tileset` — `타일 베이스`, `디테일`, `충돌/마커`
  - `ui_mockup` — `배경`, `패널`, `아이콘`, `텍스트`
  - `scene` — `하늘`, `원경`, `중경`, `전경`, `조명`

#### 데이터 모델

`crates/engine/src/model/layer_preset.rs`(신규)에 정적 프리셋을 둔다. UI 문자열은 앱에서 매핑해도 되지만, 프리셋 ID와 레이어 이름은 엔진에서 안정적으로 유지한다.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerPresetId {
    Empty,
    PixelSprite,
    CharacterShade,
    Tileset,
    UiMockup,
    Scene,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PresetFill {
    Empty,
    Solid([u8; 4]),
}

#[derive(Debug, Clone)]
pub struct PresetLayer {
    pub name: &'static str,
    pub fill: PresetFill,
    pub opacity: f64,
    pub blend: &'static str,
    pub locked: bool,
    pub visible: bool,
}

pub struct LayerPreset {
    pub id: LayerPresetId,
    pub layers: &'static [PresetLayer], // bottom→top
}

pub fn layer_preset(id: LayerPresetId) -> &'static LayerPreset;
pub fn all_layer_presets() -> &'static [LayerPreset];
```

`NewDocumentOptions`에 프리셋을 추가한다.

```rust
impl NewDocumentOptions {
    pub fn preset(mut self, id: LayerPresetId) -> Self;
}
```

#### 제안 API

```rust
pub fn apply_preset_to_new_document(
    opts: NewDocumentOptions,
    preset: LayerPresetId,
) -> EngineResult<Document>;

pub fn apply_layer_preset(
    session: &mut Session,
    preset: LayerPresetId,
) -> EngineResult<Vec<String>>;
```

- `apply_layer_preset`은 기존 문서를 지우지 않고 **활성 레이어 위에** 프리셋 스택을 추가한다.
- 반환값은 새 레이어 ID 목록(bottom→top).
- 새 문서 경로는 `new_document`가 기본 레이어를 만든 뒤 프리셋 스택으로 교체하거나, 처음부터 프리셋 레이어를 생성한다. 중간에 `Layer 1`이 남지 않게 한다.

#### 동작 명세

1. 각 `PresetLayer`는 `Layer::create`로 만들고, `Solid`이면 캔버스 전체를 해당 RGBA로 채운다. 채움은 `ChunkStore`에 직접 하되, 문서 히스토리에는 프리셋 적용 전체를 1스텝으로 기록한다.
2. `blend`는 기존 `BLEND_MODES`만 허용하고, `opacity`는 0..=1, 이름은 `validate_layer_name`을 통과해야 한다.
3. 적용 후 새 레이어 중 최상단을 활성 레이어로 한다.
4. 64레이어 제한을 넘으면 아무것도 바꾸지 않고 `OUT_OF_RANGE`를 반환한다.
5. 기존 문서 적용은 `CompositeCommand`(AddLayerCommand 여러 개)로 기록한다. 실패 시 부분 적용이 남지 않아야 한다.
6. 새 문서는 히스토리를 비우고 `mark_saved` 상태로 시작한다(기존 `new_document` 의미 유지).
7. 프리셋은 문서 스키마에 저장하지 않는다. 결과 레이어 스택만 저장된다. 마지막 선택 프리셋은 `SettingsStore`에 별도 UI 설정으로 저장해도 되지만, 문서 복원에 의존하지 않는다.

#### 주요 모듈

- `crates/engine/src/model/layer_preset.rs`(신규) — 정적 데이터, 검증.
- `crates/engine/src/model/document/types.rs` — `NewDocumentOptions.preset`, 새 문서 생성.
- `crates/engine/src/model/operations.rs` — `apply_layer_preset`.
- `crates/engine/src/model/commands/composite.rs` — 다중 추가 1스텝.
- `crates/app/src/ui/dialogs.rs` — 새 문서 프리셋 선택.
- `crates/app/src/ui/panels/layers.rs` — 기존 문서 적용 메뉴.
- `crates/app/src/ui/settings_store.rs` — 마지막 프리셋(선택).

#### 수용 기준

- 6개 프리셋 각각이 정확한 레이어 수/이름/순서/블렌드/불투명도/잠금을 만든다.
- `Solid` 배경 레이어의 모든 픽셀이 지정 RGBA다(투명 배경 프리셋 제외).
- 새 문서에서 `Layer 1`이 중복 생성되지 않는다.
- 기존 문서에 적용하면 기존 레이어는 보존되고, 새 레이어가 활성 레이어 위에 추가된다.
- 적용 전체가 Ctrl+Z 한 번으로 사라지고, Ctrl+Y로 다시 나타난다.
- 64레이어 초과 시 문서가 전혀 변하지 않는다.
- 새 문서 다이얼로그에서 취소하면 프리셋이 적용되지 않는다.

#### 위험/주의

- 프리셋 레이어 이름을 한국어로 바꾸면 테스트/문서가 흔들릴 수 있다. **ID는 영어, 표시 이름은 앱 문자열**로 분리하는 것을 권장한다.
- `Solid` 채움을 `PaintCommand`로 기록하면 새 문서 히스토리에 원치 않는 스텝이 남는다. 새 문서 경로는 히스토리 clear 후 시작한다.

---

### F03. 대칭 드로잉

- **우선순위/단계:** P0 · M1 · L
- **사용자 요청:** ✅ 대칭 도구

#### 현재 상태와 문제

- 대칭/미러 설정이 전혀 없다.
- 펜은 `stroke_segment`를 한 번만 호출하고, 도형은 `apply_shape`를 한 번만 호출하며, 채우기는 단일 시드만 채운다.
- 오버레이에 대칭축/중심을 그릴 경로가 없다.

#### 목표

하나의 입력을 여러 대칭 변환으로 복제해 **한 번의 스트로크/한 번의 undo**로 그린다. 브러시, 지우개, 선/사각형/둥근 사각형/타원/다각형, 채우기, 그라디언트에 공통 적용한다. 텍스트는 1차 범위에서 제외한다.

#### UX 진입점

- 옵션바 또는 보기 메뉴에 `대칭` 토글.
- 모드: `끔`, `수직`, `수평`, `사분면`, `방사형`.
- 방사형: 분할 수 2..=32, 중심점 드래그, `거울` 옵션(각 방사 조각을 추가 미러).
- 중심 기본값은 캔버스 중앙. 중심 핸들을 드래그하면 캔버스 좌표로 저장.
- 오버레이: 수직/수평 축, 방사형 스포크, 중심 핸들. 브러시 미리보기는 변환된 고스트로 표시.

#### 데이터 모델

`crates/engine/src/model/symmetry.rs`(신규):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymmetryMode {
    Off,
    Vertical,
    Horizontal,
    Quadrant,
    Radial,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymmetrySpec {
    pub mode: SymmetryMode,
    pub center: Option<(f64, f64)>, // None = canvas center
    pub segments: u8,              // Radial: 2..=32
    pub mirror: bool,              // Radial only
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SymmetryTransform {
    Identity,
    MirrorX(f64),
    MirrorY(f64),
    Rotate90 { cx: f64, cy: f64, quarter: u8 },
    Rotate { cx: f64, cy: f64, radians: f64 },
}

impl SymmetrySpec {
    pub fn normalize(self, canvas_w: u32, canvas_h: u32) -> Self;
    pub fn transforms(self, canvas_w: u32, canvas_h: u32) -> Vec<SymmetryTransform>;
}

impl SymmetryTransform {
    pub fn map_point(self, x: i32, y: i32) -> (i32, i32);
    pub fn map_run(self, x: i32, y: i32, len: usize) -> Option<(i32, i32, usize)>;
}
```

`Session`에 비영속 상태를 둔다.

```rust
impl Session {
    pub fn symmetry(&self) -> SymmetrySpec;
    pub fn set_symmetry(&mut self, spec: SymmetrySpec) -> EngineResult<()>;
}
```

- 문서 교체 시 `SymmetryMode::Off`로 초기화한다.
- `SettingsStore`에는 `"symmetry"` 객체로 저장/복원한다. 9개 설정 키를 억지로 늘리지 않는다.
- `ToolCx::symmetry()`가 이 값을 읽는다.

#### 그리기 통합

**공통 싱크**: `crates/engine/src/raster/symmetry.rs`(신규)

```rust
pub struct SymmetrySink<'a, S: PixelSink> {
    inner: &'a mut S,
    transforms: &'a [SymmetryTransform],
}

impl<S: PixelSink> PixelSink for SymmetrySink<'_, S> {
    fn set(&mut self, x: i32, y: i32, packed: u32);
    fn get(&self, x: i32, y: i32) -> u32;
    fn set_run(&mut self, x: i32, y: i32, len: usize, color: &[u8; 4]);
}
```

- `set`: 각 변환의 `map_point`로 복제한다. 중복 좌표는 같은 값을 쓰므로 안전하지만, 성능을 위해 중심/축 중복은 변환 목록에서 제거한다.
- `set_run`: 수직/수평 미러는 run을 역방향으로 보낼 수 있으므로 `map_run`이 `(x, y, len)`을 반환한다. 회전은 run이 연속이 아니므로 **픽셀 단위로 분해**한다. run 길이가 큰 도형은 회전 시 비용이 커질 수 있으므로, 32px 이상 run은 별도 최적화 대상으로 기록한다.
- `get`은 원본 좌표만 읽는다. 대칭 복제는 쓰기 전용이다.

**도구별 적용**

| 도구 | 적용 방식 |
|---|---|
| 펜/지우개 | `stroke_point`/`stroke_segment`를 `SymmetrySink`로 감싼다. `stroke_segment`의 내부 memo는 원본 좌표 기준으로 유지된다. |
| 선/사각형/둥근 사각형/타원/다각형 | `apply_shape`를 `SymmetrySink`로 감싼다. 회전 변환에서 bbox 도형이 왜곡되지 않도록, `ShapeSpec`을 먼저 만든 뒤 픽셀 쓰기 단계에서 변환한다. |
| 채우기 | 현재 레이어의 연결 영역을 스크래치 마스크에 먼저 채운 뒤, 마스크의 각 셀을 `SymmetrySink`로 복제한다. 원본 시드만 채우고 복제된 시드에 별도 flood fill을 돌리지 않는다. |
| 그라디언트 | `GradientMask`를 만든 뒤 각 픽셀을 `SymmetrySink`로 복제한다. 그라디언트 색은 변환 후에도 원본 색을 유지한다. |
| 텍스트 | 1차 제외. UI에서 "텍스트는 대칭 미지원"을 표시한다. |

**오버레이**

- 수직/수평: `OverlayOp::Line`.
- 방사형: 중심에서 각도별 `Line`.
- 중심 핸들: `Circle`.
- 브러시 고스트: 기존 브러시 프리뷰를 변환별로 그린다. `OverlayInfo`에 대칭 정보를 넣지 말고, `ToolCx`/세션에서 읽는다.

#### 주요 모듈

- `crates/engine/src/model/symmetry.rs`(신규).
- `crates/engine/src/raster/symmetry.rs`(신규).
- `crates/engine/src/model/session/core.rs` — 세션 상태/초기화.
- `crates/app/src/tools/api.rs` — `ToolCx::symmetry()`.
- `crates/app/src/features/pen/core.rs` — 브러시 경로.
- `crates/app/src/features/shape/commit.rs` — 도형 커밋.
- `crates/app/src/features/fill.rs` — 채우기.
- `crates/app/src/features/gradient/tool.rs` — 그라디언트.
- `crates/app/src/ui/optionbar.rs` — 대칭 컨트롤.
- `crates/app/src/canvas_view.rs` — 오버레이/프레임 업로드.

#### 작업 순서

1. `SymmetrySpec`/변환/정규화 + 단위 테스트.
2. `SymmetrySink` + `PixelSink` 테스트(수직/수평/사분면/방사형).
3. 펜/지우개 적용 + 1 undo 테스트.
4. 도형 커밋 적용 + 미리보기.
5. 채우기 스크래치 마스크 경로.
6. 그라디언트.
7. 옵션바/오버레이/중심 드래그.
8. 성능 측정(펜 64px, 방사형 16분할).

#### 수용 기준

- 수직 대칭에서 `(x,y)`에 쓴 픽셀이 `(2cx-x, y)`에도 정확히 나타난다.
- 수평/사분면/방사형 N분할이 수학적으로 정확하고, 중심축에 중복 픽셀이 생기지 않는다.
- 펜 스트로크 전체가 **1회 undo**로 사라진다.
- 지우개도 같은 대칭을 따른다.
- 도형 프리뷰와 커밋 결과가 일치한다.
- 채우기가 원본 영역과 대칭 영역 모두에 적용되고, 다른 색 영역을 넘지 않는다.
- 그라디언트가 대칭 복제되어도 원본 색상 램프가 유지된다.
- 대칭을 끄면 기존 결과와 바이트 단위로 같다.
- 16분할 방사형 펜 스트로크가 프레임 예산을 과도하게 깨지 않는다(측정치 기록).

#### 위험/주의

- `stroke_segment`의 overlap memo는 원본 좌표 기준이다. `SymmetrySink`가 변환된 좌표에 쓰더라도 memo가 원본 기준으로 유지되므로, 같은 원본 픽셀을 중복 기록하지 않는다. 다만 서로 다른 원본 픽셀이 같은 대상에 매핑될 수 있으므로 **중복 제거는 변환 목록에서** 한다.
- 회전 run은 연속성이 깨진다. 큰 도형에서 성능이 나쁘면 run을 폴리곤으로 변환하는 최적화를 별도로 한다.
- 채우기는 `PixelWriter`의 `fill_span`을 직접 대칭화하면 안 된다. 스크래치 마스크 → 복제 순서를 지킨다.

---

### F04. 도형 프리셋(하트/N각 별)

- **우선순위/단계:** P0 · M1 · M
- **사용자 요청:** ✅ 하트, N각 별 등 흔한 도형 프리셋

#### 현재 상태와 문제

- `ShapeKind`는 `Line/Rect/Rrect/Ellipse/Polygon` 5종뿐이다.
- 다각형은 사용자가 정점을 하나씩 찍어야 하고, 하트/별/화살표 같은 흔한 모양을 빠르게 만들 수 없다.
- `crates/app/src/features/shape/geom/spec.rs::build_spec`은 `PendingGeom`의 points/bbox만 사용한다.

#### 목표

도형 도구 옵션바에 프리셋을 추가하고, 프리셋을 **기존 폴리곤 래스터 경로**로 변환한다. 별도의 래스터라이저를 만들지 않는다. 결과는 기존 `ShapeSpec`/`apply_shape`/`polygon_mask`를 그대로 통과해야 한다.

#### 프리셋 목록(초기)

- `heart` — 하트(큐빅 베지어 샘플링)
- `star` — N각 별(외곽/내곽 반지름)
- `burst` — N각 방사형 폭발(별과 유사, 내곽 비율/회전)
- `diamond` — 마름모
- `triangle` — 삼각형
- `arrow` — 화살표
- `cross` — 십자
- `gear` — 기어(근사 폴리곤)

사용자 요청의 핵심은 **하트**와 **N각 별**이다. 나머지는 같은 엔진으로 확장 가능한 예시로 둔다.

#### 데이터 모델/API

`crates/app/src/features/shape/geom/preset.rs`(신규):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapePreset {
    Heart,
    Star { points: u8, inner_ratio: u8 }, // inner_ratio 1..=99 (%)
    Burst { points: u8, inner_ratio: u8 },
    Diamond,
    Triangle,
    Arrow,
    Cross,
    Gear { teeth: u8 },
}

pub struct PresetParams {
    pub rotation_deg: i32,
    pub samples: u8, // heart curve sampling, 8..=128
}

pub fn preset_points(
    preset: ShapePreset,
    bbox: Bbox,
    params: PresetParams,
) -> EngineResult<Vec<Pt>>;
```

`PendingGeom`에 프리셋을 추가한다.

```rust
pub struct PendingGeom {
    pub bbox: Option<Bbox>,
    pub p0: Option<Pt>,
    pub p1: Option<Pt>,
    pub radius: i32,
    pub points: Vec<Pt>,
    pub preset: Option<ShapePreset>, // 신규
}
```

- `build_spec`은 `geom.preset`이 있으면 `preset_points`로 points를 만들고 `ShapeKind::Polygon` 스펙을 반환한다.
- `draw_spec`/`drag_geom`도 같은 방식으로 프리셋 points를 만든다.
- `PendingValue`에는 `PresetBox { x, y, w, h, preset }`을 추가해 옵션바가 bbox를 편집할 수 있게 한다. 기존 `Polygon`/`Box`/`LineEnds`는 유지한다.
- 설정 `shapePreset`(문자열)을 추가한다. `Settings`/`SettingKey`/검증/`SettingsStore`/`PERSIST_KEYS`를 갱신한다. `TOOL_IDS`는 변하지 않는다.

#### 동작 명세

1. **하트**: bbox를 정규화한 뒤 큐빅 베지어 2개를 샘플링해 닫힌 폴리곤을 만든다. 샘플 수는 `samples`(기본 32)로 제한한다. 픽셀 아트에서 작은 크기(8~16px)도 자연스럽게 보이도록 정수 좌표로 반올림하고 중복 정점을 제거한다.
2. **N각 별**: 외곽 반지름 = bbox 반폭/반높이, 내곽 반지름 = 외곽 × `inner_ratio/100`. `points`는 3..=64. 정점 2N개를 교대로 만든다. `rotation_deg`를 적용한다.
3. **Burst**: 별과 같은 정점 생성이지만 내곽 비율/회전 기본값이 다르다. 별도 래스터라이저 없이 `ShapeKind::Polygon`으로 처리한다.
4. **기어**: teeth 수를 3..=32로 제한하고, 외곽/홈/이빨 정점을 근사한다. 1차에서는 실루엣만 제공하고 정밀 기계 도형은 후속으로 둔다.
5. **채우기/윤곽선**: 기존 `FillMode::Outline/Fill/Both`를 그대로 따른다. `stroke_width`는 기존 펜 크기를 쓴다.
6. **미리보기**: 기존 `paint_preview`/오버레이 경로를 그대로 사용한다. 프리셋은 points만 만들고, 이후는 수동 다각형과 동일하다.
7. **핸들/이동/리사이즈**: bbox 기반 이동/리사이즈는 points를 다시 생성한다. 정점 편집은 프리셋에서는 비활성화하고 "수동 다각형으로 변환" 버튼을 제공한다.
8. **대칭**: F03의 `SymmetrySink`를 통과하면 자동으로 대칭 도형이 된다.

#### 주요 모듈

- `crates/app/src/features/shape/geom/preset.rs`(신규).
- `crates/app/src/features/shape/geom/{types,spec,ops}.rs` — `PendingGeom.preset`, points 생성.
- `crates/app/src/features/shape/{state,commit,overlay}.rs` — pending/preview/commit.
- `crates/app/src/features/shape/optionbar.rs` — 프리셋 선택, 별 점 수/내곽 비율/회전/샘플.
- `crates/app/src/features/shape/pending.rs` — `PendingValue::PresetBox`.
- `crates/engine/src/model/settings/{types,validate}.rs` — `shapePreset`.
- `crates/app/src/ui/settings_store.rs` — 영속화.

#### 수용 기준

- 하트가 bbox 안에 들어가고, outline/fill/both 모두 기존 `apply_shape`로 렌더된다.
- N각 별에서 `N=5`일 때 외곽 정점 5개가 정확히 bbox 극점에 닿는다.
- `points` 범위(3..=64), `inner_ratio` 범위(1..=99) 밖 입력이 에러가 된다.
- 프리셋 도형을 이동/리사이즈하면 points가 bbox에 맞게 재생성된다.
- 프리셋 도형 커밋이 1회 undo로 사라진다.
- 프리셋 선택이 설정에 저장되고 재시작 후 복원된다.
- 수동 다각형 모드의 기존 동작/정점 편집이 변하지 않는다.
- 대칭이 켜진 상태에서 하트/별이 대칭 복제된다.

#### 위험/주의

- 하트 베지어를 너무 적게 샘플링하면 작은 크기에서 뾰족한 아티팩트가 생긴다. bbox 크기에 따라 샘플 수를 조절하되 상한을 둔다.
- `PendingGeom`에 필드를 추가하면 기존 구조체 리터럴이 깨진다. `..Default::default()`를 쓰기보다, 모든 생성부를 명시적으로 수정해 `preset: None`이 빠지지 않게 한다.
- 프리셋을 `ShapeKind`에 추가하지 않는다. `ShapeKind`는 래스터 알고리즘 선택자이며, 프리셋은 **지오메트리 생성기**다.

---

### F05. 선택·변형

- **우선순위/단계:** P1 · M2 · L
- **사용자 요청:** —

#### 현재 상태와 문제

- 선택 영역 개념이 없다. 모든 도구가 캔버스 전체 또는 활성 레이어 전체에 쓴다.
- `OverlayOp::MaskBlit`는 존재하지만 `mask_to_blit`/`packed_to_blit`가 `None`을 반환해 선택 경계를 그릴 수 없다(M0에서 해결).
- 이동/변형을 위한 픽셀 재배치 경로가 없다. `Session::stroke_*`는 있지만 변형 결과를 한 스트로크로 커밋하는 도구가 없다.

#### 목표

사각/타원/올가미/매직완드 선택을 제공하고, 선택 영역만 이동·확대·회전·반전한다. 변형 중에는 원본 자리에 구멍이 생기고 대상 픽셀은 플로팅 미리보기로 보인다. 확정하면 **1회 undo**로 전체 변형이 기록된다.

#### UX 진입점

- 도구: `select_rect`, `select_ellipse`, `select_lasso`, `select_wand` 4종을 추가한다. `TOOL_IDS`는 14 → 18로 늘어난다.
- 옵션바: `새 선택 / 추가 / 빼기 / 교차`, 페더(0..=64px, 1차는 0만), `전체 선택`, `선택 해제`, `반전`.
- 선택 후 자동으로 `이동/변형` 핸들이 나타난다. 선택 내부 드래그 = 이동, 8개 핸들 = 크기, 바깥 핸들 = 회전, `Shift` = 비율 고정, `Alt` = 중심 기준.
- `Esc` = 변형 취소, `Enter` = 확정. 선택 밖 클릭 = 확정 후 새 선택.
- 오버레이: 1px marching ants(검정/흰색 교대), 핸들 사각형, 회전 중심점.

#### 데이터 모델

`crates/engine/src/raster/selection.rs`(신규):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub mask: crate::raster::Mask, // canvas-sized, 0/1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Replace,
    Add,
    Subtract,
    Intersect,
}

impl Selection {
    pub fn from_mask(mask: crate::raster::Mask) -> Self;
    pub fn bounds(&self) -> Option<(i32, i32, u32, u32)>;
    pub fn contains(&self, x: i32, y: i32) -> bool;
    pub fn combine(&mut self, other: &Selection, mode: SelectionMode);
    pub fn invert(&mut self, w: u32, h: u32);
    pub fn is_empty(&self) -> bool;
}
```

`crates/app/src/features/selection/state.rs`(신규):

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    pub tx: f64,
    pub ty: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation_deg: f64,
    pub flip_x: bool,
    pub flip_y: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resample {
    Nearest,
    Bilinear,
}

pub struct FloatingSelection {
    pub layer_id: String,
    pub source: Option<(i32, i32)>, // cut 원본 원점, paste면 None
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,              // tightly packed
    pub transform: Transform2D,
    pub resample: Resample,
    pub label: &'static str,
}

pub struct SelectionState {
    pub selection: Option<Selection>,
    pub floating: Option<FloatingSelection>,
    pub mode: SelectionMode,
    pub tool: SelectionToolKind,
}
```

- `Session`에 `selection: SelectionState`를 비영속 상태로 둔다. 문서 교체/로드 시 초기화한다.
- `ToolCx::selection()`/`selection_mut()` 메서드를 추가한다(필드 추가 금지, M0 §5-6).
- 선택은 문서 스키마에 저장하지 않는다. 저장/재열기 후 선택은 없다.

#### 선택 생성

| 도구 | 구현 |
|---|---|
| 사각/타원 | `drag_bbox` + `Mask` 채움. 기존 `ellipse_mask` 재사용. |
| 올가미 | `for_each_bresenham`으로 닫힌 폴리곤을 만들고 `polygon_mask` 재사용. 점이 3개 미만이면 선택 해제. |
| 매직완드 | 합성 픽셀 또는 활성 레이어 픽셀에서 시작. `tolerance`(0..=255)로 RGB 거리 비교, `contiguous`면 `flood_fill_scanline` 변형, 아니면 전체 스캔. |

- 매직완드 옵션: `sample_all_layers: bool`(기본 true), `tolerance: u8`(기본 0), `contiguous: bool`(기본 true).
- 선택 마스크는 항상 캔버스 크기다. `Mask.x/y=0`.
- `Select All`은 전체 1 마스크, `Deselect`는 `None`.

#### 변형/플로팅

1. 선택 내부 드래그 시작 시 `Session::stroke_begin(EditOptions::on_layer(layer_id).labeled("선택 이동"))`을 연다.
2. 원본 선택 영역의 RGBA를 읽어 `FloatingSelection.rgba`에 복사하고, 같은 스트로크 안에서 원본 영역을 투명(0)으로 지운다.
3. 변형 중에는 `FloatingSelection`을 오버레이로만 그린다. 레이어에는 아직 대상 픽셀을 쓰지 않는다.
4. 확정 시 `stroke_with`로 대상 픽셀을 쓴다. 역변환 좌표를 순회해 nearest/bilinear로 샘플링하고, 선택 마스크 밖은 쓰지 않는다.
5. `stroke_commit(Some("선택 변형"))`으로 1스텝 기록. 취소 시 `stroke_cancel()`이 원본을 복원한다.
6. 붙여넣기(F06)는 `source=None`으로 같은 플로팅 경로를 사용한다. 이때는 원본을 지우지 않는다.

- 변형 행렬은 `Transform2D`에서 만든다. 역변환으로 대상 픽셀 → 원본 좌표를 계산한다.
- 회전은 선택 bbox 중심을 기준으로 한다. `rotation_deg`는 시계 방향 양수.
- `Bilinear`은 알파 프리멀티플라이 없이 straight alpha로 보간한다. 1차 기본값은 `Nearest`(픽셀 아트).
- 변형 중 레이어 잠금/숨김/삭제가 발생하면 `cancel_floating`을 호출하고 상태 메시지를 낸다.

#### 주요 모듈

- `crates/engine/src/raster/selection.rs`(신규) — 마스크 결합/반전/경계.
- `crates/engine/src/raster/mask.rs` — 기존 `Mask` 재사용, 필요 시 `combine` 헬퍼.
- `crates/app/src/features/selection/{mod,state,tool,overlay,transform}.rs`(신규).
- `crates/app/src/tools/api.rs` — `selection()`, `selection_mut()`.
- `crates/app/src/session_glue.rs` — `SelectionState` 소유, 도구 등록.
- `crates/app/src/features/{pen,fill,shape,gradient}` — 선택 클리핑 훅.
- `crates/app/src/canvas_view.rs` — 선택 오버레이/프레임 업로드 bbox.
- `crates/app/src/ui/optionbar.rs` — 선택 옵션.
- `crates/app/src/ui/actions.rs`/`action_router.rs` — 전체 선택/해제/반전/변형 액션.

#### 다른 도구와의 관계

- 선택이 있으면 펜/지우개/채우기/도형/그라디언트는 선택 마스크 밖에 쓰지 않는다. 구현은 `PixelSink` 데코레이터 `ClipSink`로 통일한다.
- `ClipSink`는 `set`/`set_run`/`fill_span` 모두 마스크를 확인한다. `get`은 원본을 읽는다.
- F03 대칭과 함께 쓰면 `ClipSink(SymmetrySink(inner))` 순서로 감싼다. 대칭 복제본도 선택 안에 들어와야 한다.
- 선택이 없으면 `ClipSink`를 만들지 않아 기존 경로와 바이트 동일성을 유지한다.

#### 수용 기준

- 사각 선택이 정확한 픽셀 범위를 선택하고, 추가/빼기/교차가 집합 연산과 일치한다.
- 올가미가 자기 교차 폴리곤에서도 `polygon_mask` 규칙대로 채워진다.
- 매직완드가 tolerance 0에서 같은 색 연결 영역만 선택하고, tolerance를 올리면 더 넓어진다.
- 선택 이동 시 원본 자리가 투명해지고, 확정 후 대상 위치에 픽셀이 나타난다.
- 이동/크기/회전/반전 전체가 **1회 undo**로 원상복구된다.
- `Esc` 취소가 원본 픽셀을 정확히 복원한다.
- 선택 밖 펜 스트로크가 선택 영역을 바꾸지 않는다.
- 선택이 없을 때 기존 도구 결과가 바이트 단위로 같다.
- 선택 오버레이가 확대/축소/스크롤 후에도 캔버스 좌표에 정렬된다.
- 선택 상태가 저장/재열기 후 남지 않는다(문서에 비영속).

#### 위험/주의

- 선택 마스크는 최대 캔버스에서 2048×1088 ≈ 2.2MB다. 여러 선택을 히스토리에 저장하지 않는다.
- 변형 중 원본을 지우는 방식은 `stroke_cancel`에 의존한다. 스트로크가 외부 요인으로 끊기면 복원이 보장되지 않으므로, 변형 시작 전에 레이어 잠금/가시성을 확인한다.
- `Bilinear`은 픽셀 아트에서 흐려진다. 기본값을 `Nearest`로 두고, UI에 리샘플링 선택을 노출한다.
- 선택과 F08 타일 모드가 만나면 선택 마스크는 캔버스 밖으로 나가지 않는다. 타일 경계를 넘는 선택은 1차에서 지원하지 않는다.

---

### F06. 클립보드·플로팅 붙여넣기

- **우선순위/단계:** P1 · M2 · M
- **사용자 요청:** —

#### 현재 상태와 문제

- `Cut/Copy/Paste` 액션이 메뉴에는 있지만 한쪽 디스패처에서 no-op이다(M0에서 라우터로 통합).
- 이미지 클립보드 페이로드 타입도, 시스템 클립보드 연동도 없다.
- 붙여넣기는 새 레이어/새 문서만 만들 수 있고, 위치를 조정하는 플로팅 상태가 없다.

#### 목표

선택 영역을 잘라내기/복사/붙여넣기 한다. 붙여넣은 픽셀은 **플로팅 선택**으로 나타나 이동/변형 후 확정된다. 같은 앱 안에서는 문서 간 복사가 가능하고, 가능한 플랫폼에서는 PNG를 시스템 클립보드와 주고받는다.

#### 데이터 모델

`crates/app/src/features/clipboard/mod.rs`(신규):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardPayload {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,       // tightly packed, straight alpha
    pub origin: (i32, i32),  // 복사 당시 캔버스 좌표
    pub label: String,
}

pub trait ClipboardBackend {
    fn read_image(&mut self) -> Option<ClipboardPayload>;
    fn write_image(&mut self, payload: &ClipboardPayload) -> bool;
}

pub struct InternalClipboard {
    payload: Option<ClipboardPayload>,
}
```

- `AppState`에 `clipboard: InternalClipboard`와 `clipboard_backend: Option<Box<dyn ClipboardBackend>>`를 둔다.
- 시스템 클립보드 백엔드는 `crates/platform`에 둔다. Windows는 `arboard` 또는 Win32 `CF_DIBV5`를 사용한다. 새 의존성은 `platform`에만 추가하고 `engine`은 건드리지 않는다.
- 백엔드가 없거나 실패하면 내부 클립보드만 사용하고 상태 메시지로 알린다.

#### API

```rust
pub fn copy_selection(
    session: &mut Session,
    selection: Option<&Selection>,
    merged: bool,
) -> EngineResult<ClipboardPayload>;

pub fn cut_selection(
    session: &mut Session,
    selection: Option<&Selection>,
) -> EngineResult<ClipboardPayload>;

pub fn paste_payload(
    state: &mut AppState,
    payload: ClipboardPayload,
    at: Option<(i32, i32)>,
) -> EngineResult<()>;
```

- `merged=false`면 활성 레이어, `merged=true`면 합성 결과를 복사한다. 기본은 활성 레이어.
- 선택이 없으면 활성 레이어 전체를 복사한다. 잘라내기는 선택이 없으면 레이어 전체를 지운다(확인 토스트).
- `cut_selection`은 복사 후 `Session::stroke_*`로 선택 영역을 투명하게 만든다. 복사 실패 시 지우지 않는다.
- `paste_payload`는 `FloatingSelection`을 만들고, `at=None`이면 뷰포트 중앙에 놓는다. 캔버스보다 크면 캔버스 안으로 클립한다.
- 붙여넣기 자체는 히스토리에 기록하지 않는다. 확정 시 1스텝.
- `Ctrl+Shift+V`는 새 레이어에 바로 붙여넣기(플로팅 없이)로 정의한다.

#### 동작 명세

1. `Copy`: 선택/레이어에서 RGBA를 읽어 `ClipboardPayload`를 만든다. `origin`은 선택 bbox 좌상단 또는 `(0,0)`.
2. `Cut`: `Copy` 성공 후 원본을 지우는 스트로크를 커밋한다. 두 동작은 별도 undo 스텝이 아니라 **하나의 `CompositeCommand`**로 묶는다.
3. `Paste`: 페이로드를 `FloatingSelection`으로 변환한다. 원본 위치가 현재 캔버스 안이면 그 위치, 아니면 뷰포트 중앙.
4. 플로팅 상태에서 이동/변형/확정/취소는 F05 경로를 그대로 쓴다.
5. 시스템 클립보드 쓰기는 `Copy`/`Cut` 성공 후 best-effort. 읽기는 `Paste` 시 내부 클립보드가 비어 있거나 `Ctrl+Shift+V`일 때 시도한다.
6. 앱 종료 후에는 내부 클립보드가 사라진다. 시스템 클립보드는 OS가 유지한다.

#### 주요 모듈

- `crates/app/src/features/clipboard/{mod,payload,actions}.rs`(신규).
- `crates/app/src/features/selection/{state,transform}.rs` — 플로팅 재사용.
- `crates/app/src/ui/action_router.rs` — `Cut/Copy/Paste/PasteAsLayer`.
- `crates/app/src/ui/actions.rs` — 액션/단축키.
- `crates/platform/src/clipboard.rs`(신규) — OS 이미지 클립보드.
- `crates/app/src/session_glue.rs` — 클립보드 소유.

#### 수용 기준

- 선택 복사/붙여넣기가 RGBA와 알파를 정확히 보존한다.
- 잘라내기가 원본을 지우고, 붙여넣기로 원래 위치에 복원할 수 있다.
- 붙여넣은 플로팅을 이동/변형한 뒤 확정하면 1회 undo로 사라진다.
- `Esc`로 붙여넣기 취소 시 문서가 변하지 않는다.
- 문서 A에서 복사한 픽셀을 문서 B에 붙여넣을 수 있다.
- 선택이 없을 때 레이어 전체 복사/잘라내기가 동작한다.
- 시스템 클립보드 백엔드가 있을 때 PNG 붙여넣기가 되고, 없을 때 내부 클립보드로 폴백한다.
- `Ctrl+Shift+V`가 플로팅 없이 새 레이어에 붙여넣고 1회 undo로 제거된다.

#### 위험/주의

- OS 클립보드 이미지 형식은 플랫폼마다 다르다. `platform` 밖으로 형식 세부를 새지 않게 한다.
- 큰 이미지를 시스템 클립보드에 쓰면 지연이 생긴다. 복사는 UI 프레임을 막지 않도록 백그라운드로 넘기거나, 1차에서는 내부 클립보드만 동기로 처리한다.
- `Cut`을 `CompositeCommand`로 묶을 때 `Copy`가 히스토리에 들어가지 않도록 한다.
- 붙여넣기 위치가 캔버스 밖이면 픽셀이 사라질 수 있다. 클립 후 상태 메시지를 낸다.

---

### F07. 가이드·눈금자·스냅

- **우선순위/단계:** P1 · M2 · M
- **사용자 요청:** —

#### 현재 상태와 문제

- 눈금자/가이드가 없다. `snapUnit` 설정은 있지만 도형 드래그의 픽셀 단위 스냅에만 쓰인다.
- `crates/engine/src/raster/snap.rs`는 각도/bbox/핸들 스냅만 제공하고, 캔버스/가이드 스냅은 없다.
- 캔버스 가장자리/중앙에 정렬할 방법이 없다.

#### 목표

캔버스 위/왼쪽에 눈금자를 표시하고, 눈금자에서 가이드를 끌어 만들며, 이동/도형/선/그라디언트/선택 변형이 가이드·캔버스 가장자리·중앙·그리드에 스냅된다. 가이드는 문서에 저장되고 undo 가능하다.

#### 데이터 모델

`crates/engine/src/model/guides.rs`(신규):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guide {
    pub id: String,
    pub axis: GuideAxis,
    pub pos_px: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Guides {
    pub items: Vec<Guide>,
    pub visible: bool,
    pub locked: bool,
}
```

- `Document`에 `guides: Guides`를 추가한다. 스키마 v2.1의 optional `guides` 키와 대응한다.
- `visible=false`면 가이드를 그리지 않고 스냅에도 사용하지 않는다. `locked=true`면 드래그/삭제를 막는다.
- 가이드 좌표는 캔버스 픽셀 좌표(0..width, 0..height)다. 캔버스 밖 가이드는 허용하지 않는다.

#### 스냅 API

`crates/engine/src/raster/guide_snap.rs`(신규):

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapOptions {
    pub threshold_px: f64, // 화면 px, 기본 6.0
    pub zoom: f64,
    pub canvas: (u32, u32),
    pub grid: Option<u32>, // None = off
    pub snap_guides: bool,
    pub snap_canvas: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapResult {
    pub x: i32,
    pub y: i32,
    pub snapped_x: bool,
    pub snapped_y: bool,
}

pub fn snap_point(
    x: f64,
    y: f64,
    guides: &Guides,
    opts: SnapOptions,
) -> SnapResult;

pub fn snap_rect(
    rect: (f64, f64, f64, f64),
    guides: &Guides,
    opts: SnapOptions,
) -> (f64, f64, f64, f64);
```

- 임계값은 **화면 픽셀** 기준을 캔버스 픽셀로 환산한다: `threshold_canvas = threshold_px / zoom`.
- 후보: 각 가이드, 캔버스 0/중앙/끝, 그리드 배수. 가장 가까운 후보를 축별로 독립 선택한다.
- `snap_rect`는 이동/리사이즈 모두에서 가장자리와 중심을 후보로 본다. `Alt`를 누르면 스냅을 일시 해제한다.
- 기존 `snap_unit` 설정이 켜져 있으면 `grid=Some(1)`로 매핑한다. 새 설정 키를 만들지 않는다.

#### UX

- 눈금자: 캔버스 위/왼쪽 20px, 줌에 맞춰 tick/라벨. 클릭·드래그로 가이드 생성.
- 가이드 드래그: 커서가 가이드 근처(6px)일 때 이동. 눈금자 밖으로 끌면 삭제.
- 보기 메뉴: `눈금자`, `가이드 표시`, `가이드 잠금`, `가이드에 스냅`, `모든 가이드 지우기`.
- 상태바에 현재 스냅 좌표/가이드 수를 표시한다.

#### 명령/undo

- `SetGuidesCommand`(신규, `crates/engine/src/model/commands/guides.rs`)는 이전/이후 `Guides`를 저장한다.
- 가이드 추가/이동/삭제/전체 지우기는 각각 1스텝. 연속 드래그는 pointer-up에서 1스텝.
- `visible`/`locked` 토글도 같은 명령으로 기록한다.

#### 주요 모듈

- `crates/engine/src/model/guides.rs`(신규).
- `crates/engine/src/model/document/types.rs` — `Document.guides`.
- `crates/engine/src/io/serialize.rs`/`validate` — v2.1 `guides`.
- `crates/engine/src/raster/guide_snap.rs`(신규).
- `crates/engine/src/model/commands/guides.rs`(신규).
- `crates/app/src/canvas_view.rs` — 눈금자/가이드 렌더, 스냅 적용.
- `crates/app/src/features/{shape,pen,gradient,selection}` — 스냅 훅.
- `crates/app/src/ui/action_router.rs` — 보기/가이드 액션.

#### 수용 기준

- 눈금자에서 가이드를 만들면 정확한 캔버스 좌표에 저장된다.
- 가이드 6px 이내로 이동하면 스냅되고, `Alt`로 해제된다.
- 줌 1x/2x/4x에서 스냅 임계값이 화면 기준으로 일정하다.
- 사각형/선/그라디언트/선택 변형이 같은 스냅 규칙을 따른다.
- 가이드 추가/이동/삭제가 각각 1회 undo로 되돌아간다.
- 가이드가 저장/재열기 후 복원되고, v2.0.0 문서는 빈 가이드로 열린다.
- 가이드 숨김 시 렌더/스냅 모두 비활성화된다.
- 가이드 잠금 시 드래그/삭제가 거부된다.

#### 위험/주의

- 스냅 후보가 많아지면(가이드 100개 이상) 매 프레임 비용이 커진다. 정렬된 좌표 배열 + 이진 탐색으로 가장 가까운 후보를 찾는다.
- 좌표 반올림 규칙이 도구마다 다르면 1px 어긋난다. `snap_point`가 정수 좌표를 반환하고 모든 도구가 그 값을 쓰게 한다.
- 가이드를 문서 스키마에 넣으면 v2.1 마이그레이션 테스트가 필요하다. 빈 `Guides`는 JSON에서 생략한다.
- 눈금자는 캔버스 위젯 좌표와 뷰 변환을 공유해야 한다. `View`를 복제해 계산하고, 상태를 저장하지 않는다.

---

### F08. 심리스 타일 모드

- **우선순위/단계:** P1 · M2 · L
- **사용자 요청:** —

#### 현재 상태와 문제

- 캔버스 가장자리를 넘는 브러시/채우기가 클립된다.
- 타일 미리보기/타일 경계 표시가 없다.
- 아틀라스(F01)와 타일셋(F22)은 타일 경계를 전제로 하지만, 그리기 자체는 심리스하지 않다.

#### 목표

문서를 **하나의 타일**로 취급한다. 브러시/도형/그라디언트/채우기가 가장자리를 넘으면 반대편에 이어지고, 캔버스 뷰는 3×3 반복 미리보기를 보여준다. 내보내기는 단일 타일과 반복 시트를 모두 지원한다.

#### 데이터 모델

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileMode {
    pub wrap_x: bool,
    pub wrap_y: bool,
}
```

- `Document.tile_mode: Option<TileMode>`(v2.1 optional `tile_mode`). `None` = 꺼짐.
- `TileMode { wrap_x: true, wrap_y: true }`가 기본 켜짐 값. 한 축만 켜는 것도 허용한다.
- `Session::tile_mode()`/`set_tile_mode()`를 추가하고, 설정은 문서에 저장한다(전역 설정 아님).

#### 그리기 통합

`crates/engine/src/raster/tile.rs`(신규):

```rust
pub struct TiledSink<'a, S: PixelSink> {
    inner: &'a mut S,
    width: i32,
    height: i32,
    wrap_x: bool,
    wrap_y: bool,
}

impl<S: PixelSink> PixelSink for TiledSink<'_, S> {
    fn set(&mut self, x: i32, y: i32, packed: u32);
    fn get(&self, x: i32, y: i32) -> u32;
    fn set_run(&mut self, x: i32, y: i32, len: usize, color: &[u8; 4]);
}
```

- `set`: 좌표를 modulo로 감싼다. 스탬프가 경계에 걸치면 **경계 양쪽에 같은 픽셀을 쓴다**. 구현은 `x`를 감싼 뒤, 원래 좌표가 경계 밖이면 반대편에도 쓴다.
- `set_run`: run을 경계에서 분할해 양쪽에 쓴다. 예: `x=w-2, len=4` → `w-2..w`와 `0..2`.
- `fill_span`: 기본 구현을 쓰지 말고, 타일 경계를 넘는 변형을 별도 제공한다.
- 브러시 스탬프는 `TiledSink`로 감싸면 경계 근처에서 자동으로 반대편에 나타난다. 단, 스탬프 bbox가 캔버스보다 크면(펜 64px, 캔버스 32px) 무한 반복을 막기 위해 최대 1회 반복만 한다.

**채우기**: `crates/engine/src/kernels.rs`에 `flood_fill_wrapped`를 추가한다.

```rust
pub fn flood_fill_wrapped(
    get: impl Fn(i32, i32) -> u32,
    set: impl FnMut(i32, i32, u32),
    w: i32,
    h: i32,
    seed: (i32, i32),
    target: u32,
    fill: u32,
    wrap_x: bool,
    wrap_y: bool,
) -> usize;
```

- 방문 마스크는 캔버스 크기. 이웃 좌표를 modulo로 감싼다.
- 채우기 결과는 한 번의 `stroke_*`로 커밋한다.

**그라디언트/도형**: `TiledSink`로 감싸면 픽셀 쓰기 단계에서 자동으로 반복된다. 단, 도형의 bbox는 원본 좌표로 유지한다.

#### 표시/내보내기

- 캔버스 뷰: `tile_preview` 토글. 켜면 중앙 타일 주변 8개를 반복 렌더한다. 중앙 타일만 편집 가능하고, 주변은 35% 밝기로 표시한다.
- 구현: `CanvasTexture`를 9개 quad로 그리는 `egui::Mesh`를 만든다. UV는 각 quad마다 0..1. 엔진 프레임버퍼는 그대로 1타일만 유지한다.
- 타일 경계: 중앙 타일 주변에 점선. `OverlayOp::StrokeRect`.
- 내보내기(F17): `ExportOptions.tile_repeat: Option<(u32, u32)>`. `(1,1)` = 단일, `(3,3)` = 3×3 반복. 반복은 CPU에서 RGBA를 타일링한 뒤 PNG 인코딩.

#### 주요 모듈

- `crates/engine/src/raster/tile.rs`(신규).
- `crates/engine/src/kernels.rs` — `flood_fill_wrapped`.
- `crates/engine/src/model/document/types.rs` — `tile_mode`.
- `crates/engine/src/io/serialize.rs`/`validate` — v2.1 `tile_mode`.
- `crates/app/src/canvas_view.rs` — 3×3 메시/경계.
- `crates/app/src/features/{pen,fill,shape,gradient}` — `TiledSink`.
- `crates/app/src/ui/optionbar.rs`/`menubar.rs` — 타일 모드 토글.
- `crates/app/src/file_ops.rs`/`io/export.rs` — 반복 내보내기.

#### 수용 기준

- `wrap_x`에서 `x=0`에 쓴 픽셀이 `x=w-1`에도 나타나고, 브러시가 경계를 넘어 연속된다.
- `wrap_y`도 동일하다. 한 축만 켜면 다른 축은 클립된다.
- 채우기가 경계를 넘어 연결된 영역을 모두 채운다.
- 타일 모드를 끄면 기존 결과와 바이트 단위로 같다.
- 3×3 미리보기에서 중앙 타일만 편집되고, 주변 타일은 입력을 받지 않는다.
- 3×3 반복 내보내기의 각 타일이 원본과 픽셀 단위로 같다.
- 타일 모드가 저장/재열기 후 복원되고, v2.0.0 문서는 꺼진 상태로 열린다.
- 펜 64px/캔버스 32px 같은 극단에서도 무한 루프 없이 1회 반복으로 끝난다.

#### 위험/주의

- `TiledSink`와 F03 `SymmetrySink`를 함께 쓰면 감싸는 순서가 중요하다. **대칭 먼저, 타일 나중**이면 대칭 복제본이 경계에서 잘릴 수 있고, **타일 먼저, 대칭 나중**이면 대칭축이 반복 타일마다 적용된다. 1차에서는 `TiledSink(SymmetrySink(inner))`로 정의하고, 대칭+타일 조합은 별도 테스트로 고정한다.
- 채우기의 modulo 이웃 탐색은 방문 마스크가 없으면 무한 루프에 빠진다. 반드시 캔버스 크기 방문 마스크를 둔다.
- 3×3 메시는 GPU 텍스처를 9번 샘플링한다. 큰 캔버스에서도 프레임 예산을 지키는지 측정한다.
- 타일 모드는 문서 의미를 바꾼다. F01 아틀라스/F22 타일셋과의 관계를 문서에 명시하고, 기본값은 꺼짐으로 둔다.

---

### F09. 참조 이미지 오버레이

- **우선순위/단계:** P1 · M2 · S
- **사용자 요청:** —

#### 현재 상태와 문제

- PNG를 열면 새 문서가 된다. 기존 문서 위에 참조용 이미지를 얹을 수 없다.
- `decode_png`는 이미 있지만, 앱에서 이미지를 캔버스와 독립적으로 배치/변형하는 상태가 없다.

#### 목표

PNG를 **참조 레이어**로 불러와 캔버스 위/아래에 표시하고, 이동/확대/회전/투명도/잠금을 제공한다. 참조 이미지는 문서 픽셀이 아니므로 저장/내보내기/스포이트/합성에 포함되지 않는다.

#### 데이터 모델

`crates/app/src/features/reference.rs`(신규):

```rust
pub struct ReferenceImage {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub transform: crate::features::selection::state::Transform2D,
    pub opacity: f64, // 0..=1
    pub visible: bool,
    pub locked: bool,
    pub behind: bool, // true = 캔버스 뒤, false = 위
}

pub struct ReferenceState {
    pub image: Option<ReferenceImage>,
    pub texture: Option<egui::TextureHandle>,
}
```

- `AppState` 또는 `UiState`에 `ReferenceState`를 둔다. 문서에 저장하지 않는다(재시작 시 다시 불러온다).
- `behind=true`면 캔버스 텍스처보다 먼저, `false`면 나중에 그린다. `opacity`는 알파에 곱한다.
- 참조 이미지는 `flatten_to_rgba`, `sample_pixel`, `export_png_bytes`에 포함하지 않는다.

#### UX/API

```rust
pub fn load_reference(state: &mut AppState, png: &[u8]) -> EngineResult<()>;
pub fn clear_reference(state: &mut AppState);
pub fn fit_reference_to_canvas(state: &mut AppState);
```

- 파일 메뉴 `참조 이미지 불러오기…`, `참조 이미지 제거`.
- 참조 패널: 이름, 크기, 불투명도 슬라이더, `앞/뒤`, `잠금`, `캔버스에 맞춤`, `원래 크기`, `초기화`.
- 캔버스에서 `Alt`+드래그 = 참조 이동, 핸들 = 크기/회전. 잠금 시 입력 무시.
- 텍스처는 egui `TextureHandle`로 업로드한다. 큰 이미지는 `MAX_TEXTURE_SIDE`(예: 4096)로 다운스케일하고 원본은 유지한다.

#### 주요 모듈

- `crates/app/src/features/reference.rs`(신규).
- `crates/app/src/canvas_view.rs` — 참조 이미지 페인트 순서.
- `crates/app/src/ui/panels/reference.rs`(신규) — 패널.
- `crates/app/src/ui/action_router.rs` — 불러오기/제거/맞춤.
- `crates/platform` — 파일 다이얼로그(기존).
- `crates/engine/src/io/png/decode.rs` — 기존 `decode_png`.

#### 수용 기준

- PNG를 불러오면 캔버스 중앙에 원본 크기로 표시된다.
- 이동/크기/회전/투명도가 즉시 반영되고, 캔버스 확대/축소와 함께 움직인다.
- `behind`/`front`가 페인트 순서를 바꾼다.
- 참조 이미지가 PNG 내보내기, 스포이트, 합성 결과에 포함되지 않는다.
- 참조 이미지 조작이 문서 히스토리에 기록되지 않는다.
- 잠금 시 캔버스 입력이 참조를 움직이지 않는다.
- 앱 재시작 후 참조 이미지가 사라지고, 문서는 그대로 열린다.
- 큰 PNG(4096×4096)가 텍스처 한계로 다운스케일되어도 앱이 죽지 않는다.

#### 위험/주의

- 참조 이미지를 문서에 저장하지 않는다는 결정을 UI에 명시한다("참조 이미지는 문서에 저장되지 않습니다").
- GPU 텍스처 한계는 플랫폼마다 다르다. `egui` 업로드 실패를 처리하고 상태 메시지를 낸다.
- 참조 이미지가 캔버스 위에 있으면 픽셀 확인을 방해한다. 기본값은 `behind=true`, 불투명도 50%.
- 색 공간: PNG 디코더는 straight sRGB를 반환한다. 별도 변환을 하지 않는다.

---

### F10. 사용자 브러시 팁·동역학

- **우선순위/단계:** P1 · M2 · L
- **사용자 요청:** —

#### 현재 상태와 문제

- 브러시는 `brush_footprint(n)`의 **원형** 하나뿐이다. `BrushFootprint`는 크기별 캐시이며 마스크/런을 갖는다.
- `PenTool`은 `stroke_point`/`stroke_segment`에 크기와 packed 색만 넘긴다.
- 압력/기울기/속도 입력이 `ToolEvent`에 없다.
- 브러시 패널은 크기 프리셋 16개만 제공한다.

#### 목표

PNG 마스크 기반 사용자 브러시 팁과 동역학(크기/불투명도/각도 지터, 압력/속도 반응, 간격)을 추가한다. 기본 원형 브러시는 기존 결과와 바이트 단위로 같아야 한다.

#### 데이터 모델

`crates/engine/src/raster/brush_tip.rs`(신규):

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct BrushTip {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub mask: Vec<u8>,   // 0..=255 coverage, row-major
    pub spacing: f64,    // 0.05..=4.0, stamp 간격(지름 대비)
    pub angle_deg: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrushDynamics {
    pub size_jitter: f64,      // 0..=1
    pub opacity_jitter: f64,   // 0..=1
    pub angle_jitter_deg: f64, // 0..=180
    pub pressure_size: f64,    // 0..=1
    pub pressure_opacity: f64, // 0..=1
    pub speed_size: f64,       // 0..=1
    pub scatter: f64,          // 0..=1
    pub seed: u64,             // 스트로크별 고정
}
```

- `BrushTip`은 앱 데이터 디렉터리 `brushes/<id>.png` + `brushes/index.json`에 저장한다. `platform`이 경로를 제공한다.
- `BrushDynamics`는 `SettingsStore`의 `"brushDynamics"` 객체로 저장한다. 9개 설정 키를 늘리지 않는다.
- 기본 팁은 `round`(기존 `brush_footprint`), `square`, `soft_round` 3종을 코드로 생성한다.

#### 입력 확장

`ToolEvent`에 다음 필드를 추가한다.

```rust
pub pressure: f64,          // 0..=1, 마우스는 1.0
pub tilt: Option<(f64, f64)>, // degrees
```

- `build_tool_event`가 egui 입력의 압력/틸트를 매핑한다. 없으면 `pressure=1.0`, `tilt=None`.
- `Default`도 갱신한다. `ToolEvent` 리터럴은 1곳뿐이므로 영향이 작다.
- 속도는 `ToolEvent.time_ms`와 좌표 차이로 계산한다. 별도 필드를 추가하지 않는다.

#### 스탬프 API

```rust
pub fn stamp_tip<S: PixelSink>(
    sink: &mut S,
    tip: &BrushTip,
    cx: f64,
    cy: f64,
    scale: f64,
    angle_deg: f64,
    opacity: f64,
    packed: u32,
);

pub fn stroke_tip_segment<S: PixelSink>(
    sink: &mut S,
    tip: &BrushTip,
    from: (f64, f64),
    to: (f64, f64),
    dynamics: &BrushDynamics,
    pressure: (f64, f64),
    packed: u32,
);
```

- `stamp_tip`은 역변환으로 각 대상 픽셀의 팁 좌표를 계산하고, coverage × opacity를 알파에 곱한다. `packed`의 알파와 곱한다.
- `stroke_tip_segment`는 `spacing × tip_size` 간격으로 스탬프를 배치한다. 잔여 거리는 다음 세그먼트로 이월해 gaps를 막는다.
- 지터는 `DynamicsRng`(xorshift)로 결정한다. `seed`는 스트로크 시작 시 `time_ms`/pointer id로 만들고, 테스트는 고정 seed를 주입한다.
- 원형 기본 팁은 기존 `stroke_segment` 경로를 그대로 사용한다. `tip.id == "round" && dynamics == default`면 바이트 동일.

#### UI

- 브러시 패널: 팁 그리드(썸네일), `PNG에서 가져오기…`, 이름 변경/삭제, 간격/각도.
- 동역학 섹션: 크기 지터, 불투명도 지터, 각도 지터, 압력→크기/불투명도, 속도→크기, scatter.
- 미리보기: 96px 박스에 현재 팁/동역학으로 샘플 스트로크를 그린다.
- `PenTool` 옵션바에 `픽셀 퍼펙트`(F20)와 함께 `브러시` 선택을 노출한다.

#### 주요 모듈

- `crates/engine/src/raster/brush_tip.rs`(신규).
- `crates/engine/src/raster/brush.rs` — 기존 원형 경로 유지, 새 팁 경로 추가.
- `crates/app/src/features/pen/{core,optionbar}.rs` — 팁/동역학 사용.
- `crates/app/src/input/{event,build_event}.rs` — pressure/tilt.
- `crates/app/src/ui/panels/brush.rs` — 팁/동역학 UI.
- `crates/app/src/ui/settings_store.rs` — `brushDynamics`.
- `crates/platform/src` — 브러시 파일 경로/저장.

#### 수용 기준

- 기본 원형 팁/동역학 없음이 기존 펜 결과와 바이트 단위로 같다.
- 사용자 PNG 팁이 마스크 모양대로 스탬프되고, 알파가 coverage로 반영된다.
- 간격 0.25에서 스탬프가 겹쳐 끊김이 없고, 간격 2.0에서 의도적으로 떨어진다.
- 압력 0.5가 크기/불투명도에 비례해 반영된다(마우스는 1.0).
- 지터가 고정 seed에서 재현 가능하다.
- 브러시 팁/동역학이 재시작 후 복원된다.
- 사용자 팁 삭제 시 현재 팁이 `round`로 폴백된다.
- 스트로크 전체가 1회 undo로 사라진다.

#### 위험/주의

- 커스텀 팁은 스탬프마다 역변환/샘플링을 하므로 원형 런 최적화보다 느리다. 64px 팁/빈번한 스탬프에서 프레임 예산을 측정한다.
- `opacity`를 픽셀 알파에 곱하면 겹치는 스탬프가 누적되어 진해진다. 브러시 동역학의 불투명도는 **스트로크 전체**에 적용할지, 스탬프별로 적용할지 결정해야 한다. 1차는 스탬프별 누적(포토샵식)으로 하고, `flow`와 `opacity`를 분리하지 않는다.
- PNG 팁의 알파를 coverage로 쓸 때 straight alpha와 premultiplied alpha를 혼동하지 않는다. 디코더는 straight를 반환한다.
- 압력 지원이 없는 장치에서 `pressure=0.0`이 들어오면 브러시가 사라진다. 입력 어댑터에서 `0.0`을 `1.0`으로 보정한다.

---
