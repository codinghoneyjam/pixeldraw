# core — 전역 계약·픽셀 수학·희소 저장의 유일 정의처

## 1. 모듈 개요와 책임

`src/core/`는 도구 전체의 바닥층이다. 상수·에러·이벤트 카탈로그의 단일 정의처(SSOT)이며,
모든 픽셀 연산에 정수·결정적 수학을 제공한다. 희소 청크 저장(`ChunkStore`)과 변경 추적
기록기(`PixelWriter`)도 여기서 제공한다. 어떤 상위 모듈(`model/io/render/tools/ui`)도
`core`가 역으로 의존하는 일은 없으며, 의존 방향은 항상 바깥에서 안쪽(`→ core`)이다.
값 복사를 금지하고 반드시 `import`로 참조한다(`contract.md` §1).

## 2. 의존 방향과 DOM 금지

`core`는 의존성이 0이다(npm·CDN·빌드 없음). `src/core/**` 전부는 DOM-free이며
`document`·`window`를 참조하지 않고 Node ≥ 20에서 그대로 import 가능해야 한다
(`contract.md` §4). 모듈 규약은 상대 경로 ESM `import`만 허용하고,
`export default`·동적 `import()`·`import.meta`를 쓰지 않는다(`contract.md` §5).

## 3. constants.js — 고정 상수

모든 값의 정의처는 `src/core/constants.js` 하나다. 캔버스 상한은 32px 배수 조건과
함께 검사되며, 펜 크기는 정수 1~64, 레이어 상한은 64, Undo 예산은 걸음 수와 바이트
중 먼저 도달하는 쪽이 적용된다. 줌 배율은 CSS px 기준 12단계이며, 기본 문서는
512×512 투명, 스키마 버전은 `"2.0.0"`이다.

- `TILE_PX = 64` — 1타일은 64px이므로 타일/픽셀 변환 기준이 된다. 예: `x / TILE_PX`.
- `UNIT_PX = 32` — 0.5타일이며 스냅 그리드 단위다.
- `CHUNK_PX = 32` — 청크 한 변으로 최소 수정 단위와 같다.
- `CHUNK_LEN = 4096` — `32*32*4` 바이트, 청크 한 개의 RGBA 바이트 수다.
- `MIN_SIZE_PX = 32` — 캔버스 최소 한 변이다.
- `MAX_W_PX = 2048` — 32의 배수인 가로 상한이다. `MAX_CHUNKS_X * CHUNK_PX`와 같다.
- `MAX_H_PX = 1088` — 32의 배수인 세로 상한이다(1080은 배수가 아니라 제외).
- `MAX_CHUNKS_X = 64` — 청크 x좌표 `cx`는 0~63 범위를 벗어날 수 없다.
- `MAX_CHUNKS_Y = 34` — 청크 y좌표 `cy`는 0~33 범위를 벗어날 수 없다. 한 레이어의 청크 항목은 `MAX_CHUNKS_X * MAX_CHUNKS_Y`(2176)개를 넘을 수 없다.
- `PEN_MIN = 1` — 펜 최소 크기이며 정수다.
- `PEN_MAX = 64` — 펜 최대 크기이며 `brushFootprint` 상한과 일치한다.
- `PEN_DEFAULT = 1` — 기본 펜 크기다.
- `MAX_LAYERS = 64` — 레이어 수 상한이다.
- `UNDO_MAX_STEPS = 200` — Undo 걸음 수 예산이다.
- `UNDO_MAX_BYTES = 96 MiB` — Undo 바이트 예산이며 걸음 수와 함께 적용된다.
- `ZOOM_LEVELS = [0.25,0.5,1,2,3,4,6,8,12,16,24,32]` — frozen 12단계 배율표다.
- `DEFAULT_DOC = { widthPx: 512, heightPx: 512, background: "transparent" }` — frozen 기본 문서다.
- `SCHEMA_VERSION = "2.0.0"` — 저장 포맷 버전 문자열이다.
- `chunkKey(cx, cy) -> number` — `cy*64+cx` Map 키를 만들며 희소 저장의 키 규칙이다. 예: `chunkKey(1,2) === 129`.
- `chunkCoords(key) -> { cx, cy }` — 키를 좌표로 되돌리며 `chunkKey`의 역함수다.
- `isValidCanvasSize(w, h) -> boolean` — 32 이상·상한 이하·32 배수·정수를 모두 검사한다. 예: `isValidCanvasSize(512,512) === true`.

## 4. errors.js — 단일 예외형

도구 전체의 예외는 `DrawToolError` 하나뿐이며 코드는 19종 frozen 카탈로그와
`contract.md` §3이 동일해야 한다. 그룹은 스키마·레이어·청크·PNG·저장소·값 범위로
나뉘며, 파일 크기 64 MiB 기준은 `FILE_TOO_LARGE`에 속한다.

- `ERROR_CODES` — `SCHEMA, DUP_LAYER_ID, ACTIVE_LAYER_MISSING, CHUNK_DUPLICATE, CHUNK_OUT_OF_CANVAS, CHUNK_BAD_PNG, UNSUPPORTED_LAYER_TYPE, PNG_UNSUPPORTED, PNG_CRC, PNG_SIGNATURE, FILE_TOO_LARGE, LAYER_NOT_FOUND, LAYER_LOCKED, LAYER_HIDDEN, LAYER_LIMIT, CANVAS_SIZE_INVALID, OUT_OF_RANGE, INVALID_STATE, STORAGE_QUOTA` 19종이다.
- `DrawToolError(code, message, details?) -> DrawToolError` — `code`는 카탈로그 값이어야 하며 `name`은 `"DrawToolError"`로 고정된다. 예: `throw new DrawToolError("OUT_OF_RANGE", "pen size 99 out of range")`.

## 5. events.js — 신호 카탈로그

이벤트는 `EventTarget` + 이 카탈로그만 사용하며 문자열을 흩뿌리지 않는다. UI는
Session 메서드 호출과 이벤트 구독만 하고 모델을 직접 변조하지 않는다.

- `EVENTS` — frozen 7종으로 `DOCUMENT_REPLACED=document-replaced, LAYERS_CHANGED=layers-changed, PIXELS_CHANGED=pixels-changed, HISTORY_CHANGED=history-changed, SETTINGS_CHANGED=settings-changed, STATUS_MESSAGE=status-message, TOOL_STATE=tool-state`다.
- `SETTING_KEYS` — frozen 9종으로 `primaryColor, secondaryColor, penSize, activeTool, gridMode, snapUnit, shapeFill, shapeRadius, shapeLockAspect`이며 설정 변경 통지의 키 집합이다.

## 6. pixel.js — 패킹과 색 파싱

픽셀은 u32 packed 정수로 다루며 투명은 항상 `(0,0,0,0)`으로 정규화되어 RGB 잔여값을
남기지 않는다(`contract.md` §2). 생성 색은 불투명(alpha 255) 또는 투명(0)만 만들고,
가져온 PNG의 부분 알파는 보존만 한다. 내부 스크래치 버퍼를 재사용하므로 반환 배열은
호출 측이 복사해서 써야 한다.

- `TRANSPARENT = 0` — 투명 packed 값이며 `packed === 0` 검사의 기준이다.
- `packRGBA(r, g, b, a) -> number` — 바이트 클램프 후 u32로 패킹하며 `a === 0`이면 RGB를 버리고 0을 반환한다. 예: `packRGBA(255,0,0,255) >>> 0`.
- `unpackRGBA(p) -> [r, g, b, a]` — packed 값을 4채널 배열로 풀며 `packRGBA`의 역연산이다.
- `parseHex(s) -> { r, g, b, a } | null` — `#rgb/#rrggbb/#rrggbbaa` 형태를 파싱하며 대소문자·앞뒤 공백을 허용하고 실패하면 `null`을 반환한다. 예: `parseHex("#ff0000")`.
- `toHex(r, g, b) -> string` — 채널을 소문자 `#rrggbb`로 직렬화하며 알파는 포함하지 않는다. 예: `toHex(255,0,0) === "#ff0000"`.

## 7. blend.js — 합성 수학

합성은 source-over 한 가지며 연산 순서를 바꾸면 결과가 달라지므로 순서를 고정한다.
반올림은 전역 규칙 `rnd` 하나로 통일한다(`contract.md` §2).

- `rnd(v) -> number` — `Math.floor(v + 0.5)`이며 포인터→픽셀의 `Math.floor`와 짝을 이룬다. 예: `rnd(1.5) === 2`.
- `over(dst, src, opacity) -> [r, g, b, a]` — `dst/src`는 `[r,g,b,a]` 배열이며 `opacity`는 0~1 실수다. 소스 알파가 0이면 dst를 그대로 돌려주고, 결과 알파가 0이면 `[0,0,0,0]`으로 정규화한다. 예: `over([0,0,0,0],[255,0,0,255],1)`.

## 8. chunkstore.js — 희소 저장과 기록기

`ChunkStore`는 32px 청크를 `Map<key, Uint8ClampedArray(4096)>` 희소 저장하며 빈
청크는 만들지 않는다. 범위 밖 `getPixel`은 0을 돌려주고 `_setPixel`은 조용히
무시하므로 호출 측 클리핑이 불필요하다. 직접 변조는 금지되며 픽셀 변경은
`PixelWriter` 또는 Command의 `do/undo`만 가능하다(`contract.md` §2).

- `isAllZero(data) -> boolean` — 전 바이트가 0인지 검사하며 빈 청크 판정의 기준이다.
- `ChunkStore(widthPx, heightPx)` — 크기를 검증하고 `chunksX/Y`를 파생시키며, 무효하면 `CANVAS_SIZE_INVALID`를 던진다.
- `ChunkStore._offset(x, y) -> number` — 청크 내 바이트 오프셋을 계산하는 내부 헬퍼다.
- `ChunkStore.getPixel(x, y) -> number` — packed u32를 돌려주며 범위 밖이면 0, 빈 청크도 0이다. 예: `store.getPixel(10,20) >>> 0`.
- `ChunkStore._setPixel(x, y, packed)` — Command·`PixelWriter` 전용이며 외부 직접 호출을 금지한다. `packed === 0`이면 해당 픽셀만 지우고 청크 자체는 남긴다.
- `ChunkStore.getChunk(cx, cy) -> Uint8ClampedArray | null` — 저장된 버퍼의 참조를 돌려주며 없으면 `null`이다.
- `ChunkStore.copyChunk(cx, cy) -> Uint8ClampedArray | null` — 복사본을 돌려주므로 스냅샷·undo 재료로 쓴다.
- `ChunkStore.putChunk(cx, cy, data)` — `null`이면 삭제하고, 범위 밖이면 `OUT_OF_RANGE`, 길이가 4096이 아니면 `OUT_OF_RANGE`를 던진다. 입력은 방어 복사된다.
- `ChunkStore.forEachChunk(fn)` — 키 오름차순으로 `(cx, cy, data)`를 순회하므로 직렬화가 결정적이다.
- `ChunkStore.chunkCount() -> number` — 저장된 청크 수이며 빈 청크는 세지 않는다.
- `ChunkStore.pruneEmpty() -> number` — `isAllZero` 청크를 삭제하고 제거 수를 돌려준다.
- `ChunkStore.clone() -> ChunkStore` — 같은 크기·깊은 복사를 만들며 원본과 버퍼를 공유하지 않는다.
- `ChunkStore.resizeTo(newW, newH) -> [{ cx, cy, data }]` — 크기를 검증한 뒤 잘려나간 청크의 복사본 목록을 돌려주며 redo 재료가 된다.
- `PixelWriter(store, layerId)` — 한 스트로크의 before 스냅샷과 dirty 집합을 추적하며 `finish`·`discard` 중 하나로 반드시 닫는다.
- `PixelWriter._touch(x, y)` — 최초 접촉 청크의 before를 복사하고 dirty 집합에 `(cx,cy)`를 등록하는 내부 헬퍼다.
- `PixelWriter.set(x, y, packed)` — 범위 밖은 무시하고 종료 후 호출은 `INVALID_STATE`를 던진다. 예: `writer.set(10,20,packRGBA(255,0,0,255))`.
- `PixelWriter.get(x, y) -> number` — 현재 저장소 값을 읽으며 범위 밖이면 0이다.
- `PixelWriter.takeDirty() -> [{ cx, cy }]` — `(cy,cx)` 정렬 목록을 반환한 뒤 내부를 비우므로 중간 렌더 루프에 쓴다.
- `PixelWriter.finish() -> { layerId, chunks: [{ cx, cy, before, after }] } | null` — 전역 청크가 비게 되면 저장소에서 삭제하고 `after=null`로 기록하며, 변경이 없으면 `null`을 돌려준다.
- `PixelWriter.discard() -> [{ cx, cy }]` — before로 전량 복원하고 dirty 목록을 돌려주며 취소 경로에 쓴다.
- `changeSetBytes(cs) -> number` — changeset의 before+after 바이트 합이며 `null`이면 0이다. Undo 예산 계산에 쓴다.

## 9. brush.js — 스트로크 기하

브러시는 원형 풋프인트와 Bresenham 선분 열거로 구성된다. 1·2·3px은 정사각에
수렴하는 원형 규칙을 쓰며 풋프인트는 크기별 캐시되어 재사용된다. 범위 밖 펜
크기는 `DrawToolError(OUT_OF_RANGE)`를 던진다.

- `brushFootprint(n) -> { n, offset, mask }` — 지름 `n`의 원형 마스크를 만들며 `offset=(n-1)>>1`이다. 예: `brushFootprint(3).mask.length === 9`.
- `forEachBresenham(x0, y0, x1, y1, cb)` — 정수 격자를 한 칸씩 열거하며 `cb(x,y)`를 호출하고 끝점에 도달하면 종료한다.
- `stampBrush(writer, x, y, n, packed)` — `(x-offset, y-offset)` 기준 마스크 셀이 1인 위치에만 `writer.set` 하며 한 점 찍기의 원자 연산이다.
- `strokeSegment(writer, x0, y0, x1, y1, n, packed)` — 선분 위 각 격자점에 `stampBrush`를 찍어 드래그 한 구간을 채운다. 예: `strokeSegment(w,0,0,10,0,3,red)`.
- `strokePoint(writer, x, y, n, packed)` — `stampBrush`의 별칭이며 클릭 한 점 경로에 쓴다.

## 10. raster/ — 도형 래스터 4파일

`src/core/raster/`는 도형 래스터 전용 서브패키지다. 진입점 `shape_raster.js`와
3개 순수 하위 모듈로 구성되며, 전부 DOM-free이고 Node에서 그대로 import된다
(`contract.md` §4 `src/core/**`에 속한다).

### 10.1 shape_raster.js — 적용 진입점

- `applyShape(writer, spec, primaryPacked, secondaryPacked) -> number` — `ShapeSpec`을 `PixelWriter` 경유로 래스터화하고 기록한 픽셀 수를 돌려준다. `spec.kind=line`은 `lineMask`로 곧게 그리고, 그 외(`rect`/`rrect`/`ellipse`)는 `bbox`를 `toBBox`로 정규화한 뒤 `fillMode`에 따라 분기한다. `fill`은 `ellipseMask`/`rrectMask` 전체를 1색으로, `both`는 내부를 `secondaryPacked`·외곽을 `primaryPacked`로, `outline`(기본)은 `outlineRing`만 `primaryPacked`로 쓴다. `strokeWidth` 미지정 시 1.
- 내부 헬퍼 3종: `toBBox(b)`는 배열·객체 bbox를 `[x,y,w,h]`로, `assertMaskSize(w,h)`는 정수 1 이상을, `assertBrushSize(n)`은 정수 1~64를 강제한다. 두 검증은 `RangeError`를 던진다.
- `writeMask(writer, mask, dx, dy, packed) -> number` — 마스크에서 값 1인 셀만 `writer.set` 하고 개수를 반환한다. 좌표 클리핑은 하지 않는다(`PixelWriter.set`이 범위 밖을 무시).

### 10.2 raster_brush.js — 풋프린트·Bresenham

- `brushFootprint(n) -> { n, offset, mask }` — 지름 `n` 원형 마스크를 크기별 캐시(frozen)해 재사용한다. `offset=(n-1)>>1`.
- `forEachBresenham(x0, y0, x1, y1, cb)` — 정수 격자를 한 칸씩 열거하며 끝점 도달 시 종료한다.
- **주의**: 동일 이름의 `brushFootprint`·`forEachBresenham`이 `core/brush.js`에도 존재한다. 현 트리에서 `raster_brush.js`를 import 하는模块은 없다(§13 참조).

### 10.3 raster_masks.js — 마스크 3종

- `ellipseMask(w, h)` — 타원 내부 마스크.
- `rrectMask(w, h, r)` — 모서리 반경 `r`인 사각 마스크.
- `outlineRing(kind, w, h, r = 0, n = 1)` — `kind`는 `"ellipse"`/`"rrect"`, 두께 `n`의 외곽 링.
- 반환 형태는 `{ x, y, w, h, data }`이며 `data`는 행 우선 `Uint8Array`(값 0/1). 소비자는 `applyShape`(진입점)와 `shape_overlay.js`(프리뷰)다.

### 10.3b segment.js — 선 마스크 1종

- `lineMask(p0, p1, n)` — 두 점을 잇는 두께 `n`의 선 마스크. 최소 bbox `{ x, y, w, h, data }`.
- 두 규칙으로 갈린다. `n == 1`은 Bresenham 경로(`line8`)이고, `n >= 2`는 PIL `ImagingDrawWideLine`이 선분을 **회전 사각형**으로 넓혀 다각형 스캔라인 엔진으로 채운 결과다. 후자는 `polygonOutlineMask`(폐곡선 외곽선)과 같은 `wideLineQuadEdges` + `polygonGeneric` 쌍을 재사용한다.
- n×n 정사각형을 Bresenham 경로에 찍고 끝캡을 투영으로 잘라내는 모델은 흔한 오해다. Pillow 12.1.0 기준 9 지오메트리 × 폭 2..7(54건) 중 정확히 1건만 맞는다.
- 주의: 에디터의 **라운드 펜**은 여기 있지 않다. `core/brush.js`의 `strokePoint`/`strokeSegment` + `brushFootprint`가 담당하며, 정황은 `tests/fixtures/raster_golden.json`의 `stroke` 그룹(펜 명세)과 `wide_line` 그룹(PIL `d.line` 원본)이 **분리**되어 있다. 섞으면 안 된다.

### 10.4 raster_snap.js — 스냅 기하

- `rnd(v)` — `Math.floor(v + 0.5)` 반올림. `core/blend.js`의 `rnd`와 이름·수학이 동일하다 별도 정의이며, 현 트리에서 `shape_geom.js`가 이쪽을 import 한다.
- `angleSnap(x0, y0, x1, y1, step = 15)` — 벡터를 `step`도 격자에 스냅한다. 원점이 같으면 끝점을 그대로 돌려준다.
- `dragBBox(p0, p1, lock = false, center = false)` — 드래그 지오메트리.
- `resizeBBox(b, handle, p, lock = false, center = false, minSize = 1)` — 핸들 리사이즈.
- `unitSnap(b)` — 32px(`UNIT_PX`) 그리드 스냅.
- bbox는 배열 `[x,y,w,h]` 또는 객체 `{x,y,w,h}`를 모두 받고, 반환은 항상 배열이다.

## 11. 대표 플로우 2개

스트로크 플로우는 `PixelWriter`를 열고 `strokeSegment`로 채운 뒤 중간 렌더와
확정을 분리한다. `set` 반복 → `takeDirty`(중간 렌더) → `finish`(청크 diff) →
`PaintCommand` 순이며, `finish`가 `null`이면 커맨드를 만들지 않는다.
취소 플로우는 `discard()`가 before 스냅샷으로 복원하고 dirty 목록을 돌려줘
화면만 갱신하면 된다. 빈 청크는 `finish`·`pruneEmpty`가 `isAllZero`로 정리하므로
저장소에는 흔적이 남지 않는다.

## 12. contract 연결

`contract.md` §1의 상수는 `constants.js`가 유일 정의처이며, §2의 정수 좌표·투명
정규화·`PixelWriter` 전용 변경·결정성 규칙을 이 모듈이 강제한다. §3의 19종 코드는
`errors.js`와 동일해야 하며, §4의 DOM 금지 구역에 `src/core/**` 전부가 속한다.
`chunkKey` 공식과 `CHUNK_LEN` 정의도 계약과 코드가 한 글자도 어긋나면 안 된다.

## 13. 미해결 사항 (2026-10-01 확인)

- **`raster_brush.js`는 죽은 모듈이다.** 동일 심볼을 가진 `core/brush.js`가 생존 버전이고
  실제 소비자는 `pen.js`·`panel_brush.js`가 `core/brush.js`를 import 한다. 분리 작업
  (커밋 `62f6d0ae`)에서 잔류한 중복 구현이며, `contract.md` §1 "값 복사 금지" 및
  저장소 헌법의 Anti-Duplication SSOT에 저촉된다. 정리 대상.
- **`rnd`가 두 곳에 정의된다.** `core/blend.js`와 `core/raster/raster_snap.js`에 같은
  수학의 `rnd`가 각각 존재한다. §1 단일 정의처 원칙의 예외로 취급 필요.
- `shape_raster.js`의 `assertMaskSize`/`assertBrushSize`는 `RangeError`를 던지는데,
  `contract.md` §2-7은 `DrawToolError` 하나만 허용한다. 위반 여부 판단 필요.

## Handoff

- Wrote: `draw_tool_v2/core.md`
- Result: `core/` 11개 파일(raster/ 4개 포함) 전 심볼 커버리지, 미해결 중복 3건 명시
- Next: Orchestrator — `raster_brush.js` 제거 및 `rnd` 단일화 판단
