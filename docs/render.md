# render — 뷰·합성·표시

## 1. 개요·경계

- `view.js`·`composite.js`·`grid.js(계산부)`는 DOM-free 순수 모듈. Node import 가능, 테스트 직접 대상.
- `renderer.js`·`renderer_composite.js`·`renderer_display.js`·`background.js`·`grid.js(페인팅부)`는 브라우저 전용. `document`·`OffscreenCanvas`·`ResizeObserver`·`requestAnimationFrame` 접근 허용.
- 하위(`core/`)만 import. `model/`·상위 도메인·UI 역참조 금지. ESM 상대경로만, `export default`·동적`import()`·`import.meta` 금지.
- `view`는 줌·팬 수학 SSOT, `composite`는 표시 합성 SSOT(배경 제외), `CanvasRenderer`는 구독→dirty→rAF 오케스트레이션만.

## 2. view.js — 12 exports

좌표계: `View = { zoom, offsetX, offsetY }`. `zoom` = CSS px per canvas px. `offset` = 뷰포트 기준 캔버스 (0,0) 모서리의 CSS px 위치.

| # | 이름 | 시그니처 | 상세 |
|---|---|---|---|
| 1 | `ZOOM_LEVELS` | re-export (12단계, 0.25–32) | `core/constants` 재노출. `stepZoom`·`fitView` 기준 계단 |
| 2 | `createView` | `createView(o={}) → View` | `zoom/offsetX/offsetY` 검증. 비유한수·`zoom<=0` → `OUT_OF_RANGE` |
| 3 | `screenToCanvas` | `(v, sx, sy) → {x,y}` | `(sx-offsetX)/zoom`. 부동소수 그대로 반환 |
| 4 | `canvasToScreen` | `(v, x, y) → {x,y}` | `offset + v*zoom`. 역변환, 반올림 없음 |
| 5 | `pixelAt` | `(v, sx, sy) → {x,y}` | `screenToCanvas` + `Math.floor`. 정수 픽셀 인덱스 |
| 6 | `zoomAt` | `(v, ax, ay, newZoom) → View` | 앵커 `(ax,ay)`(CSS px) 고정 줌. `k=new/old`, `offset=ax-(ax-off)*k`. `newZoom<=0` → `OUT_OF_RANGE` |
| 7 | `stepZoom` | `(zoom, dir) → number` | `dir>0` 위·`<0` 아래·`0` 유지. 끝값 고착(범위 밖이면 양끝 반환) |
| 8 | `panBy` | `(v, dx, dy) → View` | `offset += (dx,dy)`. 불변 반환(스프레드) |
| 9 | `clampView` | `(v, canvasW, canvasH, vw, vh, margin=32) → View` | `offsetX ∈ [margin-canvasW*zoom, vw-margin]`, Y 동일. 캔버스가 완전히 사라지지 않게 32px 여유 |
| 10 | `fitView` | `(canvasW, canvasH, vw, vh, padding=16) → View` | `want=min((vw-2p)/cw,(vh-2p)/ch)`, 이하 최대 레벨 선택(없으면 최소). 중앙 정렬(`Math.round`) |
| 11 | `actualSizeView` | `(canvasW, canvasH, vw, vh) → View` | `zoom=1` 중앙 정렬 |
| 12 | `visibleChunkRange` | `(v, canvasW, canvasH, vw, vh) → {cx0,cy0,cx1,cy1} \| null` | 화면 (0,0)→(vw,vh)를 캔버스 좌표로 역투영 후 `CHUNK_PX`로 양자화. 클램프 후 역전 시 null(화면 밖) |

## 3. composite.js — 2 exports (DOM-free, 배경 제외)

| 이름 | 시그니처 | 상세 |
|---|---|---|
| `compositeChunk` | `(doc, cx, cy, out: Uint8ClampedArray(4096)) → bool` | 가시층 bottom-up 합성. `visible!==true`·`opacity<=0`·청크없음 skip. `over(dst, src, opacity)` 순차 누적. 불투명+opacity1 고속경로(덮어쓰기). 투명 결과 RGB 0 정규화. 가시 픽셀 유무 반환. `out` 규격 위반 → `INVALID_STATE` |
| `samplePixel` | `(doc, x, y) → [r,g,b,a]` | 1픽셀 표시 합성색. 범위 밖·비정수 → `[0,0,0,0]`. 스포이트·Alt-스포이트 기준값. 청크 오프셋 `(y-cy*32)*32+(x-cx*32)` |

배경은 여기서 처리하지 않음. `renderer_display`의 `paintBackground`가 담당.

## 4. renderer.js — CanvasRenderer (브라우저 전용, 210줄)

구독→dirty→rAF 파이프:

- `constructor({host, session, getView, getOverlay})`: `host.appendChild`·`getView`·`session.addEventListener` 없으면 `INVALID_STATE`.
- `attach()`: `canvas.dt-display` 생성·`absolute/inset:0`, host가 `static`이면 `relative`로 승격. `DOCUMENT_REPLACED`→`_setDocument`, `PIXELS_CHANGED`→`invalidateChunks`(또는 `all`→`invalidateAll`), `LAYERS_CHANGED`→`active`·`prop.name` 제외 전체 무효화, `SETTINGS_CHANGED(gridMode)`→재렌더. `ResizeObserver`→`resize()`.
- `resize()`: `clientWidth/Height`→(실패 시 `getBoundingClientRect`)→`_cssW/_cssH`, `_dpr=window.devicePixelRatio||1`. `display.width=round(cssW*dpr)` — 값 동일 시 대입 금지(비트맵 클리어→깜빡임 방지). `style.width`는 CSS px 유지.
- `requestRender()`: rAF 단일 슬롯(`_cancelFrame` 가드). rAF 없으면 `setTimeout 16ms` 폴백.
- `invalidateAll()` / `invalidateChunks(chunks)`: `chunkKey(cx,cy)` 집합 적재. 비정상 청크 skip.
- `_setDocument(doc)`: `ensureComp` + 전체 무효화 + 렌더 예약.
- `_update()`: 크기 변경 시 `ensureComp`. `dirtyAll`이면 전 청크 키 전개. `paintDirty`(합성 캐시) → `drawDisplay`(화면). 잔여 dirty 있으면 다음 프레임 예약.
- `pixelToDevice(x,y)`: `(offset+x*zoom)*dpr`. 오버레이 콜백에 주입.
- `dispose()`: rAF 취소·리스너 해제·RO 해제·캔버스 제거.

## 5. renderer_composite.js — 합성 캐시

| 이름 | 상세 |
|---|---|
| `ensureComp(r)` | `doc.canvas` 크기 오프스크린 1개 + `ImageData(32,32)` 재사용 버퍼 확보. `OffscreenCanvas` 우선, 없으면 `document.createElement("canvas")`. 동일 크기면 no-op. 생성 후 `_dirtyAll=true` |
| `paintDirty(r, doc, view, w, h)` | 더티 청크를 `_comp`에 `putImageData`. 1순위: `visibleChunkRange` 내 가시 청크 전량. 2순위: 나머지 키를 `FRAME_BUDGET_MS=6ms` 예산 내 순회, 초과 시 중단(다음 프레임 계속) |

`nowMs()`는 `performance.now()` 우선, 없으면 `Date.now()`.

## 6. renderer_display.js — 화면 blit

`drawDisplay(r, doc, view, w, h)` 순서:

1. `setTransform(1,0,0,1,0,0)` 리셋 → `clearRect(devW,devH)`. `devW=round(cssW*dpr)`.
2. `dx=round(offsetX*dpr)`, `dy`, `dw=round(w*zoom*dpr)`, `dh` 계산.
3. `save → rect(dx,dy,dw,dh) → clip` (캔버스 영역 밖 그리기 차단).
4. `paintBackground(...)` — 체커/단색 (클립 내부).
5. `imageSmoothingEnabled=false` → `blitVisible(...)` — 합성 캐시→표시.
6. `restore` → `paintGrid(...)` — 클립 밖에서 별도 클립으로 그림.
7. 테두리 1px 4변(`rgba(0,0,0,0.6)`) → 오버레이 콜백 `(ctx,{view,dpr,pixelToDevice})`.

`blitVisible(r, ctx, view, w, h, dpr)`:

- 소스: `x0=max(0,floor((0-offsetX)/zoom))`, `sw=min(w,ceil((cssW-offsetX)/zoom))-x0` (Y 동일). `sw/sh<=0`이면 리턴(화면 밖).
- 목적지: `drawImage(comp, x0,y0,sw,sh, round((offsetX+x0*zoom)*dpr), ..., round(sw*zoom*dpr), ...)`. `ddw/ddh<=0`이면 리턴.

## 7. background.js — 2 exports

| 이름 | 시그니처 | 상세 |
|---|---|---|
| `paintChecker` | `(ctx, dx, dy, dw, dh, dpr, checkerCssPx=8, viewport=null) → void` | 화면 고정 체커. `cell=round(8*dpr)`. `viewport={vw,vh}` 교집합만 칠해 비용을 뷰포트에 묶음(줌 무관). 위상은 디바이스 원점 고정. `A=#ffffff` 바탕 + `(ix+iy)%2`에 `B=#d9d9d9` |
| `paintBackground` | `(ctx, background, dx, dy, dw, dh, dpr, viewport=null) → void` | `transparent`/비문자열 → 체커만. `parseHex` 알파0 → 체커만. 부분알파 → 체커+`rgba()`. 불투명 → `rgb()` 직칠 |

## 8. grid.js — 4 exports

모드 사다리(상호배타): `off`=없음, `unit`=32px만, `tile`=64px만, `pixel`=1px+32px+64px.

| 이름 | 상세 |
|---|---|
| 상수 | `GRID_ALPHA_THIN=0.3, THICK=0.55, PIXEL=0.15`. `a`가 팔레트 마커(두께·합성 프로파일 결정) |
| `gridStrokeDevPx(a, dpr)` | `PIXEL→1`, 그 외 `ceil(cssPx*dpr)` (`THIN 1px`·`THICK 2px`). `ceil`인 이유: `round`는 dpr 1.25에서 1px로 떨어져 소실. 1px 검사용은 고정 헤어라인 |
| `gridLines(view, w, h, vw, vh, mode)` | DOM-free 순수 계산. `32*zoom>=6`·`64*zoom>=6`·`z>=8` 임계 미만 rung 생략. 화면→캔버스 역투영 후 `pushLines`로 `k*step` 수집, `[0,max]` 클립. `pixel` rung이 인덱스 0(얇은 순). 전부 숨기면 `null` |
| `paintGrid(ctx, lines, view, dx, dy, dw, dh, devW, devH, dpr)` | 디바이스 공간 페인터. 표시영역 교집합 없으면 리턴. `rect(dx,dy,dw,dh)+clip` 후 X·Y 런 stamp. 32/64px는 `source-over` 2톤 베벨(밝은 run + 후행 1px 어두운 run), 1px만 `difference` 헤어라인 |

`difference`를 32/64px에서 버린 이유: 중간톤(체커 이음새)과 상쇄되어 선이 사라짐. 베벨은 어느 배경 위에서도 한 패스는 반드시 대비를 만듦.

## 9. 렌더 파이프라인 순서도

```text
Session 이벤트 (pixels/layers/settings/document)
  → dirty Set (+dirtyAll) → requestRender (rAF 1슬롯)
  → _update: ensureComp(크기변경 시) → paintDirty(합성캐시)
  → drawDisplay:
     clip(캔버스 rect) → paintBackground(체커/단색)
     → blitVisible(comp→display, smoothing off)
     → paintGrid(별도 clip) → border 1px → overlay 콜백
```

## 10. 좌표·DPR 주의점

- CSS px(뷰·레이아웃) vs canvas px(픽셀 데이터) vs device px(실제 비트맵) 3층 구분. `zoom`=CSSpx/canvaspx, `dpr`=devicepx/CSSpx.
- 표시 좌표는 항상 `Math.round` 후 사용(`dx/dy/dw/dh`, blit 목적지, 그리드 런). 누적 오차·반픽셀 번짐 방지.
- `imageSmoothingEnabled=false` 고정. 고배율 픽셀 경계 선명 유지.
- `paintChecker`·`paintGrid`는 뷰포트 교집합만 그려 고배율에서도 비용 상수. `paintDirty`는 가시 청크 우선 + 6ms 예산으로 프레임 드롭 방지.
- `compositeChunk out`은 `Uint8ClampedArray(4096)` 고정. `attach`는 DOM 없으면 `INVALID_STATE`.

## 11. 파일 구성 (2026-10-01 확인)

`src/render/`는 7개 파일이다. 분리 커밋으로 `renderer.js`가 327줄에서 210줄로 줄었고
`renderer_composite.js`(58줄)·`renderer_display.js`(51줄)가 신설되었다. 두 하위 모듈은
모두 `CanvasRenderer` 인스턴스 `r`을 첫 인자로 받아 상태를 직접 접근하는 패턴이며,
`renderer/` 안에서만 호출된다. 분리 대상 파일은 더 이상 없다.

| 파일 | 줄 | DOM | 역할 |
|:---|:---:|:---:|:---|
| `renderer.js` | 210 | 허용 | 수명 주기·구독·dirty·rAF 오케스트레이션 |
| `grid.js` | 123 | 계산부만 | 그리드 계산(DOM-free) + 페인팅 |
| `composite.js` | 102 | 금지 | 표시 합성 SSOT |
| `renderer_composite.js` | 58 | 허용 | 합성 캐시 |
| `renderer_display.js` | 51 | 허용 | 화면 blit |
| `view.js` | 86 | 금지 | 줌·팬 수학 SSOT |
| `background.js` | 46 | 허용 | 체커·단색 배경 |
