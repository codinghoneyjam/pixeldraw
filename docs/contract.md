# contract — 전역 계약(고정 상수·불변 조건·에러·구역 규칙)

모듈 문서(`core.md`·`model.md`·`io.md`·`render.md`·`tools.md`·`ui.md`)가 공통으로 인용하는
전역 규칙의 **유일한 규범 문서**다. 이 문서는 도구 자체에 완전히 자기포함(self-contained)하며
외부 저장소·외부 명세를 참조하지 않는다.

코드 주석의 `contract §N` 표기는 모두 이 문서를 가리킨다.

## 1. 고정 상수

정의처는 오직 `src/core/constants.js` 하나다. 다른 파일에 값을 **복사 금지**, 반드시 import한다.

| 이름 | 값 | 비고 |
|---|---|---|
| `TILE_PX` | 64 | 1타일 = 64px |
| `UNIT_PX` = `CHUNK_PX` | 32 | 0.5타일 = 최소 수정 단위 = 청크 한 변 |
| `CHUNK_LEN` | 4096 | `32*32*4` 바이트 |
| `MIN_SIZE_PX` | 32 | 캔버스 최소 크기 |
| `MAX_W_PX` | 2048 | 32의 배수 상한. = `MAX_CHUNKS_X * CHUNK_PX` |
| `MAX_H_PX` | 1088 | 32의 배수 상한 (1080은 32의 배수가 아님) |
| `MAX_CHUNKS_X` / `MAX_CHUNKS_Y` | 64 / 34 | 청크 좌표 `cx∈[0,63]`, `cy∈[0,33]`. 한 레이어 청크 항목 상한 = 64*34 = 2176 |
| `PEN_MIN` / `PEN_MAX` / `PEN_DEFAULT` | 1 / 64 / 1 | 정수 |
| `MAX_LAYERS` | 64 | |
| `UNDO_MAX_STEPS` | 200 | |
| `UNDO_MAX_BYTES` | 96 MiB | 둘 중 먼저 도달하는 쪽 |
| `ZOOM_LEVELS` | `[0.25,0.5,1,2,3,4,6,8,12,16,24,32]` | 배율 = 캔버스 1px당 CSS px |
| `VIEW_CLAMP_MARGIN` | 32 (CSS px) | `clampView` 기본 마진. 캔버스가 완전히 화면 밖으로 나가지 않도록 항상 이만큼 남긴다 |
| `VIEW_FIT_PADDING` | 16 (CSS px) | `fitView` 기본 여백. 양쪽 축에 `2*padding`를 뺀 뒤 축소 |
| `VIEW_CHUNK_EDGE_EPS` | `1e-9` | `visibleChunkRange`의 먼 가장자리에서 빼는 값. 캔버스 경계가 청크 경계와 정확히 만날 때 청크를 하나 과다 보고하는 것을 막는다 |
| `DEFAULT_DOC` | 512×512, 배경 `transparent` | |
| `SCHEMA_VERSION` | `"2.0.0"` | |

`chunkKey(cx, cy) = cy*64 + cx` (Map 키). 역변환은 `chunkCoords(key)`.

### 1-2. 표시 상수 — Phase 2 (2026-10-12 확정)

정의처는 `src/render/*.js`의 모듈 상수였는데, 이 파일들은 `src/core/**` 밖이라
`constants.js`에 닿지 못한다(§4가 DOM을 허용하는 구역). Rust 이식에서 값이
희족어지는 것을 막기 위해 **계약 표로 등록**하고 정의처는
`crates/engine/src/constants.rs` 하나로 옮겼다. 값은 JS 원본에서 복사했다.

| 이름 | 값 | JS 원본 |
|---|---|---|
| `FRAME_BUDGET_MS` | 6 | `renderer_composite.js` L5 |
| `CHECKER_A` / `CHECKER_B` | `#ffffff` / `#d9d9d9` | `background.js` L6-7 |
| `CHECKER_CSS_PX` | 8 | `background.js` L9 기본매개변수 |
| `BORDER_RGBA` | `(0,0,0,0.6)` → `[0,0,0,153]` | `renderer_display.js` L4 |
| `GRID_ALPHA_THIN` / `GRID_ALPHA_THICK` / `GRID_ALPHA_PIXEL` | 0.3 / 0.55 / 0.15 | `grid.js` L14-16 |
| `GRID_THIN_CSS_PX` / `GRID_THICK_CSS_PX` | 1 / 2 | `grid.js` L19-20 |
| `GRID_BEVEL_DEV_PX` | 1 | `grid.js` L21 |
| bevel pass 알파 | thick 밝 0.44·어둠 0.5, thin 밝 0.3·어둠 0.32 | `grid.js` L48-51 |

**그리드 선 폭은 `ceil(css*dpr)`다(round 아님)** — dpr 1.25의 1 CSS px 선이
0.8 device px로 내려가 선이 사는 것을 막는다. 단 `GRID_ALPHA_PIXEL` 헤어라인은
예외로 모든 dpr에서 정확히 1 device px다.
**알파 3종은 무게가 아니라 팔레트 마커**다(`strokeDevPx`가 `a`로 두께를,
`passAlpha`가 `a`로 잉크량을 고른다). 그래서 `GridStroke::a`는 가중치 이상의
의미를 가지며 다른 값으로 대체할 수 없다.

### 1-1. 레이어 블렌드 모드 (2026-10-09 확정)

`BLEND_MODES`의 정의처는 `src/core/blend.js` **하나**이며, 값은 아래 여섯 가지다.
다른 파일에 같은 목록을 복사 금지 — `features/layers/layer.js`는 이를 재수출만 한다.

| 모드 | 채널 수식 (`sc`=소스·`dc`=배경, 0~255) |
|---|---|
| `normal` | `sc` |
| `multiply` | `rnd(sc * dc / 255)` |
| `screen` | `rnd(sc + dc - sc * dc / 255)` |
| `overlay` | `dc < 128 ? rnd(2 * sc * dc / 255) : rnd(255 - 2 * (255 - sc) * (255 - dc) / 255)` |
| `darken` | `min(sc, dc)` |
| `lighten` | `max(sc, dc)` |

모든 수식은 **0~255 정수 공간**에서 계산하며 각 채널당 한 번만 `rnd()`로 반올림한다.
`overlay`의 `dc = 128` 경계는 이 수식 고유의 behaviour다: 128은 128보다 작지 않으므로
screen 분기를 택하고, 결과가 이상적 중간값보다 1 LSB 벗어난다(예: `overlay(0,128) = 1`).
이 편차는 수식의 성질이지 버그가 아니며, 골든 픽스처에 그대로 박힌다.

**알파 모델 — "색만 혼합, 알파는 source-over"**

1. 색은 **비premultiplied(straight)** 채널끼리 혼합한다. 누적기는 항상 straight 알파를
   보관하며(`composite.js`의 `dr,dg,db,da`), `over()`가 되돌리는 값도 straight다.
2. 혼합 결과색 `bc`를 **소스 색으로 대체**한 뒤, 알파·합성은 `over()`와 완전히 동일하게
   진행한다. 즉 `oa = sa + da*(1-sa)`, 채널은 `rnd((bc*sa + dc*da*(1-sa)) / oa)`.
3. **배경 알파 `da === 0`이면 혼합하지 않는다.** 배경색이 정의되지 않았으므로
   `multiply`가 검정으로 곱하는 참사를 막기 위해 이 경우 `normal`과 같게 취급한다.
4. 반올림은 위 수식의 모든 실수 연산에 `rnd(v) = Math.floor(v + 0.5)`를 쓴다.
   두 가지 반올림 규칙을 섞지 않는다.
5. **결정성.** 같은 입력 → 같은 출력. 무작위 금지.

이 모델은 W3C 합성 스펙의 isolated-backdrop 그룹 처리와 다르다. 스펙은 배경 알파에
가중을 둔 별도 수식을 요구한다. 여기서는 픽셀 에디터 관행(Aseprite·Krita의 레이어
취급)에 가까운 **간이 모델**을 택했고, 그 사실이 규범이다. 나중에 스펙 정합으로
바꾸려면 이 절의 수식과 골든 픽스처를 함께 갱아한다.

**동치 불변 (테스트가 강제)**: 같은 문서에 대해 화면 합성(`compositeChunk`)·
PNG
평면화(`export_png`)·점 샘플링(`samplePixel`, 스포이트가 읽는 값)은 **모든 모드에서
같은 픽셀값**을 내야 한다. 셋 중 하나라도 어긋나면 사용자가 집는 색과 저장되는 색이
달라진다.

## 2. 전역 불변 조건

모든 모듈이 지킨다. 위반은 버그이며 테스트가 잡아내야 한다.

1. **좌표는 정수 픽셀.** 포인터 → 픽셀은 `Math.floor`. 반올림은 항상
   `rnd(v) = Math.floor(v + 0.5)`.
2. **생성 색은 불투명(alpha 255) 또는 투명(0)만.** 투명 픽셀은 항상 `(0,0,0,0)`으로 정규화
   (RGB 잔여값 금지). 가져온 PNG의 부분 알파는 보존만 하고 도구는 만들지 않는다.
3. **`ChunkStore` 직접 변조 금지.** 픽셀 변경은 `PixelWriter`(도구) 또는 Command의 `do/undo`만
   가능. viewport 밖 `set`은 조용히 무시된다(호출 측 클리핑 불필요).
4. **`Document` 변경은 Command로만.** 생성자·`loadDocument` 제외. Command 밖에서
   `layers.push` 같은 직접 변경 금지. `Document._*` 밑줄 메서드는 Command 전용이다.
5. **DOM 금지 구역**은 §4를 따른다.
6. **이벤트는 `EventTarget` + `events.js` 카탈로그만.** UI는 Session 메서드 호출과 Session
   이벤트 구독만 하며 모델을 직접 변조하지 않는다.
7. **에러는 `DrawToolError(code, message, details?)` 하나만.** 카탈로그는 §3.
8. **결정성.** 같은 입력 → 같은 출력. 무작위·시간 의존 금지(레이어/도형 ID는 카운터 기반).
   예외: 문서 `document_id`만 UI 계층에서 `crypto.randomUUID()`로 발급.
9. **의존성 0.** npm 패키지 없음, CDN 없음, 빌드 없음. Node ≥ 20(테스트). 브라우저는
   `CompressionStream` 필요(Chrome 80+, Firefox 113+, Safari 16.4+).

## 3. 에러 코드 카탈로그

`src/core/errors.js`의 `ERROR_CODES`(frozen, 19종)에 정의된 값과 동일해야 한다.

| 그룹 | 코드 |
|---|---|
| 스키마 | `SCHEMA`, `UNSUPPORTED_LAYER_TYPE` |
| 레이어 | `DUP_LAYER_ID`, `ACTIVE_LAYER_MISSING`, `LAYER_NOT_FOUND`, `LAYER_LOCKED`, `LAYER_HIDDEN`, `LAYER_LIMIT` |
| 청크 | `CHUNK_DUPLICATE`, `CHUNK_OUT_OF_CANVAS`, `CHUNK_BAD_PNG` |
| PNG | `PNG_UNSUPPORTED`, `PNG_CRC`, `PNG_SIGNATURE` |
| 저장소 | `FILE_TOO_LARGE`, `STORAGE_QUOTA` |
| 값 범위 | `CANVAS_SIZE_INVALID`, `OUT_OF_RANGE`, `INVALID_STATE` |

`FILE_TOO_LARGE` 기준은 64 MiB다.

## 4. DOM 금지 구역

아래 경로는 `document`·`window`를 참조하지 않아야 하며 **Node에서 import 가능**해야 한다.

| 경로 | 예외 |
|---|---|
| `src/core/**` (incl. `src/core/raster/**`) | 없음 (전부 DOM-free) |
| `src/features/document/**` | 없음 (전부 DOM-free) |
| `src/features/layers/**` | `panel_layers.js` 등 **패널 모듈만** DOM 허용 |
| `src/features/color/**` | `panel_color*.js` 등 **패널 모듈만** DOM 허용 |
| `src/io/**` | `file_io.js`, `store_idb.js` 만 브라우저 API 접촉(지연 접근) |
| `src/features/viewport/view.js`, `src/render/composite.js` | 없음 |
| `src/tools/**` | 순수 로직 전부 DOM-free |
| `src/ui/**` | 전체가 DOM 허용 |

`src/render/renderer.js`·`grid.js`·`background.js`는 캔버스 표시용이라 DOM이 허용된다.
`src/features/**`의 패널·오버레이 모듈(`panel_color*.js`·`panel_layers.js`·
`panel_brush.js`·`panel_view_options.js`·`panel_shape_options.js`·`shape_overlay.js`)은
DOM을 만지므로 `src/ui/**`와 같은 예외를 적용받는다. 반면 각 피처의 도구 본체와
순수 모듈(`pen.js`·`fill.js`·`eyedropper.js`·`shape.js`·`shape_geom.js`·`view.js`)은
DOM-free 규칙을 그대로 지킨다. 구분선은 파일 접미사다: `panel_*`는 마운트·DOM,
그 외는 판단·상태.

## 5. 모듈 규약

- `src/`는 상대 경로 ESM `import`만 쓴다. `export default`, 동적 `import()`, `import.meta`를
  쓰지 않는다(브라우저가 모듈을 그대로 로드하므로 번들러가 개입하지 않는다).
- 최상위 선언 이름은 모듈 간 충돌이 없도록 고유해야 한다.
- 실행 진입점은 두 개다 — 브라우저는 `run.py`, 네이티브는 `src-tauri/`(Tauri).
  구버전 단일 HTML 번들러는 제거되었다. 두 경로가 만나는 지점은
  `src/io/file_io.js` 하나뿐이며, 규칙은 `docs/desktop.md`에 적었다.

## 6. 의존 DAG

```
core → features/document → io ─┐
core → features/document → render ─┤→ tools → ui
core → shape_raster → features/shape 도구 ─┘
```

`core`는 어떤 모듈에도 의존하지 않는다. `ui`는 최상위 소비자로서 하위 모듈을 호출만 한다.
`features/`는 툴 기능 단위로 묶인 피처 패키지며, 각 피처가 자기 도구·패널·모델을 함께 가진다.

`shape_raster`는 `src/core/raster/` 서브패키지다. 진입점 `shape_raster.js`(`applyShape`만
export)와 순수 하위 모듈 `raster_brush.js`·`raster_masks.js`·`raster_snap.js`로 구성되며,
4개 모두 DOM-free이고 `src/core/**` 예외 없음 규칙(§4)을 그대로 따른다. 소비자는
`features/shape/shape.js`(적용)와 `features/shape/shape_overlay.js`(프리뷰),
`features/shape/shape_geom.js`(`raster_snap.js`)다.
