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
| `MAX_W_PX` | 1920 | 32의 배수 상한 |
| `MAX_H_PX` | 1088 | 32의 배수 상한 (1080은 32의 배수가 아님) |
| `MAX_CHUNKS_X` / `MAX_CHUNKS_Y` | 60 / 34 | 청크 좌표 `cx∈[0,59]`, `cy∈[0,33]` |
| `PEN_MIN` / `PEN_MAX` / `PEN_DEFAULT` | 1 / 64 / 1 | 정수 |
| `MAX_LAYERS` | 64 | |
| `UNDO_MAX_STEPS` | 200 | |
| `UNDO_MAX_BYTES` | 96 MiB | 둘 중 먼저 도달하는 쪽 |
| `ZOOM_LEVELS` | `[0.25,0.5,1,2,3,4,6,8,12,16,24,32]` | 배율 = 캔버스 1px당 CSS px |
| `DEFAULT_DOC` | 512×512, 배경 `transparent` | |
| `SCHEMA_VERSION` | `"2.0.0"` | |

`chunkKey(cx, cy) = cy*64 + cx` (Map 키). 역변환은 `chunkCoords(key)`.

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
| `src/core/**` | 없음 (전부 DOM-free) |
| `src/model/**` | 없음 (전부 DOM-free) |
| `src/io/**` | `file_io.js`, `store_idb.js` 만 브라우저 API 접촉(지연 접근) |
| `src/render/view.js`, `src/render/composite.js` | 없음 |
| `src/tools/**` | 순수 로직 전부 DOM-free |
| `src/ui/**` | 전체가 DOM 허용 |

`src/render/renderer.js`·`grid.js`·`background.js`는 캔버스 표시용이라 DOM이 허용된다.

## 5. 모듈 규약

- `src/`는 상대 경로 ESM `import`만 쓴다. `export default`, 동적 `import()`, `import.meta`를
  쓰지 않는다(브라우저가 모듈을 그대로 로드하므로 번들러가 개입하지 않는다).
- 최상위 선언 이름은 모듈 간 충돌이 없도록 고유해야 한다.
- 실행 진입점은 `run.py` 하나다. 구버전 단일 HTML 번들러는 제거되었다.

## 6. 의존 DAG

```
core → model → io ─┐
core → model → render ─┤→ tools → ui
core → shape_raster → shape 도구 ─┘
```

`core`는 어떤 모듈에도 의존하지 않는다. `ui`는 최상위 소비자로서 하위 모듈을 호출만 한다.
