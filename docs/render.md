# render — 뷰·합성·표시

## 역할 요약

- `view`·`composite`는 DOM-free 순수 모듈(Node import 가능). 줌·팬 수학과 표시 합성 담당.
- `renderer`는 브라우저 전용 표시기. 합성 캐시+더티 청크+viewport 클리핑으로 그린다.
- 배경(체커·단색)과 그리드(32·64·픽셀)는 별도 모듈.

## 파일별 API

### view.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `ZOOM_LEVELS` | re-export (12단계) | 0.25–32 |
| `createView` | `createView(o={})` | `{zoom, offsetX, offsetY}` 생성, zoom≤0 `OUT_OF_RANGE` |
| `screenToCanvas` | `screenToCanvas(v, sx, sy)` | 화면→캔버스 좌표 |
| `canvasToScreen` | `canvasToScreen(v, x, y)` | 역변환 |
| `pixelAt` | `pixelAt(v, sx, sy)` | `Math.floor` 정수 픽셀 |
| `zoomAt` | `zoomAt(v, ax, ay, newZoom)` | 앵커 고정 줌 |
| `stepZoom` | `stepZoom(zoom, dir)` | 단계 이동(끝값 고착) |
| `panBy` | `panBy(v, dx, dy)` | 평행 이동 |
| `clampView` | `clampView(v, canvasW, canvasH, vw, vh, margin=32)` | 32px 여유 클램프 |
| `fitView` | `fitView(canvasW, canvasH, vw, vh, padding=16)` | 맞춤(레벨 이하 최대값) |
| `actualSizeView` | `actualSizeView(canvasW, canvasH, vw, vh)` | 100% 중앙 |
| `visibleChunkRange` | `visibleChunkRange(v, canvasW, canvasH, vw, vh)` | 가시 청크 범위 또는 null |

offset은 뷰포트 기준 캔버스 (0,0) 모서리의 CSS px 위치.

### composite.js (DOM-free)

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `compositeChunk` | `compositeChunk(doc, cx, cy, out)` | 가시층 bottom-up 합성→4096바이트, 가시 픽셀 유무 bool |
| `samplePixel` | `samplePixel(doc, x, y)` | 표시 합성색 `[r,g,b,a]`, 범위 밖 투명 |

불투명+opacity1 고속 경로あり. 투명 결과는 RGB 0 정규화. 배경은 처리하지 않음.

### renderer.js (브라우저 전용)

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `CanvasRenderer` | `constructor({host, session, getView, getOverlay})` | host·세션·뷰 getter 필수 |
| — | `attach()` / `dispose()` / `resize()` | 캔버스 생성·해제·DPR 리사이즈 |
| — | `requestRender()` / `invalidateAll()` / `invalidateChunks(chunks)` | rAF 스케줄·전체/부분 무효화 |
| — | `pixelToDevice(x, y)` | 캔버스→디바이스 px |

내부: 오프스크린 합성 캔버스 1개 + 청크 ImageData 재사용. `pixels-changed`는 해당 청크만, `layers-changed`(active·이름 제외)·`document-replaced`는 전체 무효화. 가시 청크 우선 페인트 후 프레임 예산 6ms 내 나머지 처리. `imageSmoothingEnabled=false` 고정, 표시 영역 clip 후 blit, 오버레이(도구 프리뷰) 콜백 실행.

### grid.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| 상수 | `GRID_ALPHA_THIN=0.3, THICK=0.55, PIXEL=0.15` | 선 투명도 |
| `gridLines` | `gridLines(view, w, h, vw, vh, mode)` | `off`=null, `unit`=**32px만**, `tile`=**64px만**, `pixel`=1px+32px+64px 선 목록 또는 null |
| `paintGrid` | `paintGrid(ctx, lines, view, dx, dy, dw, dh, devW, devH, dpr)` | 32/64px는 2톤 베벨, 1px만 `difference` 헤어라인 |

두 규칙이 중요하다.

1. **`unit`과 `tile`은 rung을 공유하지 않는다.** 예전 `tile`은 64px와 함께 32px 라인도 그려서 32/64 선택이 화면상 구분이 안 됐었다(사용자 보고 "그리드 32,64 미작동"). 지금은 `unit`=32px만, `tile`=64px만이고 `tile`이 더 굵고 진하게 인킹된다.
2. **선 두께는 CSS px로 선언하고 `ceil(cssPx*dpr)` 디바이스 px로 그린다.** 예전 `fillRect(...,1,...)`은 dpr를 무시해 1.5× 화면에서 0.67 CSS px(사실상 보이지 않음)로 뭉개졌다. `round`가 아니라 `ceil`인 이유는 `round`는 dpr 1.25에서 다시 1 디바이스 px로 내려가 같은 버그를 만든다.

**`difference` 합성을 32/64px에서 버린 이유**: `difference`는 백드롭을 역상으로 밀기 때문에 **중간톤 픽셀(체커보드 이음새)과 상쇄되어 변화량이 0**이 된다. 선이 사라지던 바로 그 위치다. 그래서 32/64px는 `source-over` 2톤 베벨(밝은 run + 후행 가장자리의 1 디바이스 px 어두운 run)로 그린다. 어떤 색 위에서도 둘 중 하나가 반드시 대비를 만든다. 1px 검사용 헤어라인만 `difference`를 유지한다.

확대율이 낮으면(32px×zoom<6 등) 자동 생략.

### background.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `CHECKER_A/B` | `#ffffff/#d9d9d9` | 체커 색 |
| `paintChecker` | `paintChecker(ctx, dx, dy, dw, dh, dpr, checkerCssPx=8, viewport=null)` | 뷰포트 교집합만 그림(줌 무관 비용) |
| `paintBackground` | `paintBackground(ctx, background, dx, dy, dw, dh, dpr, viewport=null)` | transparent→체커, 부분알파→체커+색, 불투명→단색 |

## 핵심 흐름

- 렌더 루프: Session 이벤트→dirty 집합→rAF `_update`→`_paintDirty`(합성 캐시 갱신)→`_drawDisplay`(배경→blit→그리드→테두리→오버레이).
- 스포이트·펜 Alt-스포이트는 `samplePixel`(표시 합성색) 기준.

## 주의점

- viewport 클리핑: `_paintDirty`는 가시 청크 우선, `paintChecker`는 뷰포트 교집합만. 고배율에서도 비용이 뷰포트에 묶임.
- `compositeChunk`의 `out`은 반드시 `Uint8ClampedArray(4096)` 아니면 `INVALID_STATE`.
- `attach()`는 DOM 없으면 `INVALID_STATE`. `core`·`model`·`view`·`composite`에는 DOM 코드를 넣지 말 것.
- 모듈 규약: 상대 경로 ESM import만, `export default`·동적 `import()`·`import.meta` 없음 (`contract.md` §5).
