# core — 상수·에러·픽셀 기반

## 역할 요약

- 전역 계약의 유일한 정의처(상수·에러코드·이벤트명). 다른 파일에 값 복사 금지, 반드시 import.
- 모든 픽셀 연산의 정수·결정적 수학 제공(pack·합성·브러시·도형 마스크).
- 32px 청크 희소 저장(`ChunkStore`) + 변경 추적 기록기(`PixelWriter`) 제공.

## 파일별 API

### constants.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| 상수 | `TILE_PX=64, UNIT_PX=32, CHUNK_PX=32, CHUNK_LEN=4096` 등 | `contract.md` §1 고정값, `DEFAULT_DOC` 512×512 transparent |
| `chunkKey` | `chunkKey(cx, cy)` | `cy*64+cx` Map 키 |
| `chunkCoords` | `chunkCoords(key)` | `{cx, cy}` 역변환 |
| `isValidCanvasSize` | `isValidCanvasSize(w, h)` | 32 이상·상한 이하·32 배수 검사 |

상한·펜·레이어·Undo 예산의 정의는 `contract.md` §1 참조(중복 정의 금지).

### errors.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `ERROR_CODES` | frozen 객체(19종) | `contract.md` §3 카탈로그와 동일해야 함 |
| `DrawToolError` | `constructor(code, message, details?)` | 전체 도구 유일 예외형 |

### events.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `EVENTS` | frozen 7종 | `document-replaced, layers-changed, pixels-changed, history-changed, settings-changed, status-message, tool-state` |
| `SETTING_KEYS` | frozen 9종 | primaryColor, secondaryColor, penSize, activeTool, gridMode, snapUnit, shapeFill, shapeRadius, shapeLockAspect |

### pixel.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `TRANSPARENT` | `= 0` | 투명 packed 값 |
| `packRGBA` | `packRGBA(r, g, b, a)` | a=0이면 0 반환(RGB 잔여 제거) |
| `unpackRGBA` | `unpackRGBA(p)` | `[r,g,b,a]` 배열 |
| `parseHex` | `parseHex(s)` | `#rgb/#rrggbb/#rrggbbaa` → `{r,g,b,a}` 또는 null |
| `toHex` | `toHex(r, g, b)` | `#rrggbb` 소문자 |

### blend.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `rnd` | `rnd(v)` | `Math.floor(v+0.5)` |
| `over` | `over(dst, src, opacity)` | source-over 합성, 연산 순서 고정, 결과 `[r,g,b,a]` |

### chunkstore.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `isAllZero` | `isAllZero(data)` | 전 바이트 0 검사 |
| `ChunkStore` | `constructor(widthPx, heightPx)` | 희소 청크 Map, 빈 청크 미생성 |
| — | `getPixel(x, y)` / `_setPixel(x, y, packed)` | 범위 밖 get→0, set→무시 |
| — | `getChunk(cx, cy)` / `copyChunk(cx, cy)` | 없으면 null |
| — | `putChunk(cx, cy, data)` | null이면 삭제, 범위 밖이면 `OUT_OF_RANGE` |
| — | `forEachChunk(fn)` / `chunkCount()` / `pruneEmpty()` / `clone()` / `resizeTo(newW, newH)` | 정렬 순회·복제·리사이즈(잘린 청크 반환) |
| `PixelWriter` | `constructor(store, layerId)` | 스트로크 단위 before/dirty 추적 |
| — | `set(x, y, packed)` / `get(x, y)` | 범위 밖 set 무시, 종료 후 set은 `INVALID_STATE` |
| — | `takeDirty()` | `{cx,cy}` 목록 정렬 반환 후 비움 |
| — | `finish()` | 변경분 `{layerId, chunks:[{cx,cy,before,after}]}` 또는 null(빈 청크 자동 정리) |
| — | `discard()` | before로 복원 + dirty 반환 |
| `changeSetBytes` | `changeSetBytes(cs)` | before+after 바이트 합 |

### brush.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `brushFootprint` | `brushFootprint(n)` | 원형 마스크 `{n, offset, mask}` 캐시, 범위 밖 `OUT_OF_RANGE` |
| `forEachBresenham` | `forEachBresenham(x0, y0, x1, y1, cb)` | 정수 브레젠험 열거 |
| `stampBrush` | `stampBrush(writer, x, y, n, packed)` | footprint 찍기 |
| `strokeSegment` | `strokeSegment(writer, x0, y0, x1, y1, n, packed)` | 선분 스트로크 |
| `strokePoint` | `strokePoint(writer, x, y, n, packed)` | 한 점 찍기 |

1·2·3px은 정사각에 수렴하는 원형 규칙을 사용한다.

### shape_raster.js (순수, Node 가능)

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `rnd` | `(v) => Math.floor(v+0.5)` | 전역 반올림 규칙 |
| `brushFootprint` / `forEachBresenham` | 상동(단 `RangeError` 사용) | brush.js와 동일 수학, 별도 캐시 |
| `ellipseMask` | `ellipseMask(w, h)` | 빈 행/열 보정 포함 마스크 |
| `rrectMask` | `rrectMask(w, h, r)` | `X=2x+1` 코너 규칙 |
| `outlineRing` | `outlineRing(kind, w, h, r=0, n=1)` | 안쪽 정렬 테두리, 작으면 solid |
| `lineMask` | `lineMask(p0, p1, n)` | 최소 bbox+마스크 `{x,y,w,h,data}` |
| `angleSnap` | `angleSnap(x0, y0, x1, y1, step=15)` | 각도 스냅 `[x,y]` |
| `dragBBox` | `dragBBox(p0, p1, lock, center)` | 드래그 bbox `[x,y,w,h]` |
| `resizeBBox` | `resizeBBox(b, handle, p, lock, center, minSize=1)` | 핸들 리사이즈 |
| `unitSnap` | `unitSnap(b)` | 32px 그리드 스냅 |
| `applyShape` | `applyShape(writer, spec, primaryPacked, secondaryPacked)` | spec 래스터화, 칠한 픽셀 수 반환 |

`spec`: line은 `{kind,p0,p1,strokeWidth}`, 도형은 `{kind,bbox,radius,strokeWidth,fillMode}`. `both`는 바깥=보조색, 테두리=주색.

## 핵심 흐름

- 스트로크: `PixelWriter.set` 반복 → `takeDirty`(중간 렌더) → `finish`(청크 diff) → `PaintCommand`.
- 빈 청크는 저장하지 않음(`finish`·`pruneEmpty`가 `isAllZero` 정리).

## 주의점

- `brush.js` 위반은 `DrawToolError(OUT_OF_RANGE)`, `shape_raster.js` 단독 함수는 `RangeError`. 혼동 주의.
- `ChunkStore._setPixel` 직접 호출 금지(불변 조건 `contract.md` §2-3). 외부에서는 `PixelWriter`만 사용.
- viewport 밖 `set`은 조용히 무시 → 호출 측 클리핑 불필요.
- 모듈 규약: 상대 경로 ESM import만, `export default`·동적 `import()`·`import.meta` 없음 (`contract.md` §5).
