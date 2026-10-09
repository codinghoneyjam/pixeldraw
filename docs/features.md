# features — 툴 기능 단위 피처 패키지 (37파일, 4015줄)

`src/features/<피처>/`는 **툴 기능 하나를 자기 도구·패널·모델과 함께** 묶은 패키지다.
2026-10-08 재편으로 `src/model/`이 사라지고 도구 구현체·패널·뷰가 여기로 옮겨왔다.
이 문서는 그 패키지들의 **공개 표면**을 코드에서 그대로 옮긴 규범 문서다.

## 0. 읽는 순서와 규칙

| 질문 | 답은 어디에 |
|---|---|
| 이 피처가 뭘 파는가 | §1 지도 |
| 새 도구를 추가하려면 | §2 체크리스트 |
| DOM을 만져도 되는 파일은 | §3 (contract §4와 같은 규칙) |
| 심볰 이름으로 파일 찾기 | §4~§12 각 피처의 공개 API 표 |
| 피처 간 의존은 | §13 |

**공통 규약**

- 상대 경로 ESM `import`만. `export default`·동적 `import()`·`import.meta` 금지 (contract §5).
- 모든 새 파일 첫 줄은 `// <META - FILE SUMMARY - …>`. 함수 위에는 `// <META - ROLE : … | La-b>`
  표식. 이건 장식이 아니라 `tests/tool_integrity_check.mjs`가 **강제**한다.
- 파일 300줄 상한. 현재 `src/` 전체에서 초과는 `core/raster/polygon.js`(301) 한 개뿐이다.
- 에러는 `DrawToolError` 하나만. 색·좌표·합성 규칙은 `docs/contract.md` §1~§3.
- **도구 본체는 DOM-free.** `pixelToDevice`·`view`·`dpr`은 `overlay(ctx, info)` 인자로만 주입한다.

## 1. 지도

```
src/features/
├── viewport/    view.js view_store.js panel_view_options.js view_actions.js
├── document/    document.js history.js session.js settings_validator.js
├── layers/      layer.js layer_operations.js commands.js commands_pixel.js
│                edit_session.js layer_actions.js panel_layers.js
├── pen/         pen.js brush.js panel_brush.js brush_actions.js
├── shape/       shape.js shape_geom.js shape_overlay.js shape_render.js
│                shape_pending.js shape_keys.js panel_shape_options.js
├── fill/        fill.js
├── eyedropper/  eyedropper.js
├── hand/        hand.js
└── color/       panel_color.js panel_color_fields.js panel_color_wheel.js
                 color_palette.js color_recent.js color_slots.js
                 color_events.js color_actions.js
```

`src/ui/`는 **조립 셸**이다. 도구별 동작은 여기 없다(`docs/ui.md` 참고).
`src/tools/`는 입력 플럼빙만 남았다.

## 2. 새 도구 추가 체크리스트

1. `src/features/<name>/` 에 도구 본체(`<name>.js`)를 만든다. `src/tools/tool_base.js`의
   `Tool`을 상속하고 `id`·`pointerDown/Move/Up`·`cancel`·`overlay`를 구현한다.
2. 지형은 `core/`의 순수 함수로 분리한다. 도구는 DOM·ChunkStore 직접 변조 금지 —
   픽셀은 `session.beginEdit → writer → flush/commit` 경로만.
3. 옵션바 컨트롤이 필요하면 `panel_<name>_options.js`를 만든다. 셸은
   `index.html`의 `data-section="<name>"` 로 찾는다 (`docs/ui.md` §2).
4. 메뉴·단축키 동작이 필요하면 `<name>_actions.js` — 함수는 `session`/`viewStore`만
   받는다. 등록은 `src/ui/actions/action_map.js` 한 곳.
5. 아이콘은 `src/ui/toolbar/icons.js`의 `ICONS`, 한글 문구는 `src/ui/shared/strings.js`.
6. `toolManager.register(new XTool(env))` 는 `src/ui/app.js` boot 3단계.
7. 테스트: 순수 로직은 도메인 단정, DOM 로직은 `tests/helpers/fake_optionbar.js` fake.

## 3. DOM 규칙 (contract §4와 동일)

| 구분 | 파일 접미사 | 예 |
|---|---|---|
| DOM-free (판단·상태) | 그 외 전부 | `pen.js`·`fill.js`·`shape.js`·`shape_geom.js`·`view.js`·`document.js` |
| DOM 허용 (마운트·오버레이) | `panel_*` | `panel_color*.js`·`panel_layers.js`·`panel_brush.js`·`panel_view_options.js`·`panel_shape_options.js` |

`src/features/document/**`는 전예외 없이 DOM-free. `features/layers/**`·`features/color/**`는
패널 파일만 예외.

## 4. viewport — 뷰 수학·뷰 스토어·뷰 옵션

| 파일 | 줄 | 공개 API |
|---|---:|---|
| `view.js` | 86 | `createView`·`screenToCanvas`·`canvasToScreen`·`pixelAt`·`zoomAt`·`stepZoom`·`panBy`·`clampView`·`fitView`·`actualSizeView`·`visibleChunkRange` (+`ZOOM_LEVELS` 재수출) |
| `view_store.js` | 71 | `createViewStore(host,session)` → `{get,set,subscribe,center,zoomTo,zoomIn,zoomOut,fit,actual}` |
| `panel_view_options.js` | 64 | `mountViewOptions(root,{session,view,safe})` → `{dispose}\|null` |
| `view_actions.js` | 19 | `zoomIn`·`zoomOut`·`fit`·`actual`(viewStore 위임)·`gridCycle`(`GRID_MODES` 순환) |

- **DOM-free**: `view.js` 전부. 좌표는 정수 픽셀, `Math.floor`.
- `view_store`는 `host.clientWidth/Height`로 뷰포트를 읽는다(실패시 800×600).
  Session 직접 구독 없음 — 앱이 `subscribe(render)`로 소비한다.
- `panel_view_options.js`는 `#dt-grid-mode`·`#dt-zoom-select`(ZOOM_LEVELS로 채움)·
  `#dt-zoom-fit`·`#dt-zoom-actual`을 소유한다. 그래도 `gridCycle`의 `GRID_MODES`는
  `core/constants.js`가 SSOT.

## 5. document — 문서·히스토리·세션·설정 검증 (DOM-free 전부)

| 파일 | 줄 | 공개 API |
|---|---:|---|
| `document.js` | 220 | `Document` 클래스·`newDocument(opts)`·`IdGen`·`validateBackground`·`validateViewport` |
| `history.js` | 139 | `UndoManager` (`setDocument`·`commit`·`undo`·`redo`·`canUndo`·`canRedo`·`undoLabel`·`redoLabel`·`breakMerge`·`markSaved`·`isDirty`·`clear`·`subscribe`) |
| `session.js` | 157 | `Session extends EventTarget` (+`TOOL_IDS`·`validateSetting` 재수출) |
| `settings_validator.js` | 57 | `validateSetting(key,value)`·`TOOL_IDS` |

- `Session`이 **유일한 진입점**: `settings`는 호출마다 frozen 복사본이라 한 번 읽은 값은
  스냅샷(두 번 쓰는 swap이 안전한 이유). `setSetting`은 정규화 후 불변이면 이벤트 없이 종료.
- 레이어 작업 11종(`addLayer`…`setActiveLayer`)과 `resizeCanvas`, 편집 세션
  `beginEdit`는 여기 **선언**되고 구현은 `features/layers/`에 있다.
- 생성 색은 불투명(255) 또는 투명(0)만 (`validateColor`). 알파 0은 거부된다.

## 6. layers — 레이어·Command·편집 세션·패널

| 파일 | 줄 | 공개 API |
|---|---:|---|
| `layer.js` | 60 | `Layer` 클래스·`BLEND_MODES`(core에서 재수출)·`LAYER_PROP_FIELDS`·`validateLayer{Name,Opacity,Blend}` |
| `layer_operations.js` | 110 | `addLayer`·`duplicateLayer`·`removeLayer`·`moveLayer`·`mergeDown`·`insertLayer`·`resizeCanvas`·`renameLayer`·`setLayer{Visible,Locked,Opacity,Blend}`·`setActiveLayer` |
| `commands.js` | 140 | `PaintCommand`·`AddLayerCommand`·`RemoveLayerCommand`·`MoveLayerCommand`·`SetLayerPropCommand` (+`MergeDownCommand`·`ResizeCanvasCommand` 재수출) |
| `commands_pixel.js` | 159 | `MergeDownCommand`·`ResizeCanvasCommand` |
| `edit_session.js` | 59 | `beginEdit(session,{layerId,label})` → `{writer,label,flush,commit,cancel}` |
| `layer_actions.js` | 20 | `layerAdd`·`layerDuplicate`·`layerRemove`·`layerMergeDown`·`layerUp`·`layerDown` |
| `panel_layers.js` | 264 | `mountLayers(root,{session})`·`paintThumb(canvas,layer)` |

- **`BLEND_MODES` 여섯 종 모두 실제로 합성된다.** 정의처는 `core/blend.js` 하나이며
  `layer.js`는 재수출만 한다. 수식·알파 모델·반올림은 `docs/contract.md` §1-1.
  화면(`render/composite.js`)·PNG(`io/export_png.js`)·스포이트(`samplePixel`)가
  **같은 `overBlend()`**를 읽으므로 세 결과가 어긠날 수 없고,
  `tests/blend_modes.test.mjs`가 모드별로 그 동치를 단언한다.
  불투명 fast path는 `mode === "normal"`일 때만 유효하므로 세 경로 모두 게이팅했다.
- 위 규칙: `moveLayer`에서 위로 = index+1 (뒤가 위). 패널은 이 불변을 그대로 쓴다.
- `beginEdit`는 잠금·숨김·없는 층이면 `DrawToolError`. 도구는 그 예외를 조용히 무시한다.
  `commit()`은 실제로 바뀐 픽셀이 있어야 `PaintCommand`를 남기고 `true`를 돌린다.
- `panel_layers.js`만 DOM 예외. 나머지는 전부 DOM-free.

## 7. pen — 펜·지우개·브러시·브러시 패널

| 파일 | 줄 | 공개 API |
|---|---:|---|
| `pen.js` | 261 | `PenTool extends Tool` (`mode: "draw"` → id `pen`, `"erase"` → id `eraser`) |
| `brush.js` | 55 | `brushFootprint(n)`·`forEachBresenham`·`stampBrush`·`strokeSegment`·`strokePoint` |
| `panel_brush.js` | 176 | `mountBrush(root,{session})`·`SIZE_PRESETS`(16단계)·`PRESET_COLUMNS`·`presetColumns`·`presetRows`·`paintPreview`·`stepPenSize` |
| `brush_actions.js` | 9 | `brushStep(session,delta)` (`PEN_MIN`/`PEN_MAX` 클램프) |

- **`brushFootprint`의 SSOT는 여기다.** `core/raster/raster_brush.js`에 같은 이름의
  죽은 복제본이 남아 있다(import되지 않음) — README 알려진 문제 항목.
- 진행: `pointerDown`=`strokePoint+flush` → `pointerMove`=coalesced 순회
  `strokeSegment+flush` → `pointerUp`=잔여 연결 후 `commit`.
- `PEN_MIN=1`/`PEN_MAX=64`. 프리셋은 16단계(1,2,3,4,5,6,8,10,12,16,20,24,32,40,48,64)이며
  CSS는 `--dt-preset-cols/rows`로 4×4 그리드를 만든다.

## 8. shape — 도형 도구 5종 (line·rect·rrect·ellipse·polygon)

| 파일 | 줄 | 공개 API |
|---|---:|---|
| `shape.js` | 274 | `ShapeTool` (생성자 `(env, kind)`) — 포인터 동사·상태 머신 |
| `shape_geom.js` | 193 | `HANDLE_IDS`·`distSeg`·`layoutHandles`·`hitHandle`·`pointInPolygon`·`insideShape`·`dragGeom`·`resizeGeom`·`moveGeom`·`parseLineGeom`·`parsePolygonGeom`·`snapPlacePoint`·`parseBoxGeom`·`buildSpec`·`drawSpec`·`geomFromSpec` |
| `shape_overlay.js` | 114 | `packOf`·`computePaintLayers`·`deviceOf`·`renderOverlay` |
| `shape_render.js` | 114 | `buildColor`·`buildPendingSpec`·`parsePendingValue`·`commitPending`·`previewSpec`·`previewPaintLayers`·`paintPreview`·`renderToolOverlay` |
| `shape_pending.js` | 69 | `placePoint`·`finishPlacing`·`readPending`·`writePending` |
| `shape_keys.js` | 57 | `handleKeyDown(tool,ev)` |
| `panel_shape_options.js` | 147 | `mountShapeOptions(root,{session,toolManager,safe})` → `{sync,fill,dispose}\|null` |

- `Tool`을 상속하지 않고 **덕타이핑**한다(`acceptsButton` 없음 = 좌클릭만).
- 상태 머신: `idle → drawing → pending → resizing/moving`.
- **색은 스냅샷.** `pending.color`는 그려진 시점의 주색이며 이후 주색을 바꿔도
  기존 도형이 변색되지 않는다(`buildColor`).
- `keyDown`: Enter=확정·Esc=진행 취소·Delete/Backspace=보류 파기·방향키=nudge
  (Shift=32px). 오버레이는 폴리곤 다변 누적 후 bbox+핸들.
- `panel_shape_options.js`는 옵션바의 `#dt-shape-*` 컨트롤과 그 활성 행렬을 소유한다.
  bbox 수정은 `writePending` 경유라 기존 pending의 색을 보존한다.

## 9. fill — 페인트통

| 파일 | 줄 | 공개 API |
|---|---:|---|
| `fill.js` | 77 | `floodFillScanline(writer,get,w,h,sx,sy,seed,fill)`·`FillTool` |

- 4방향 스캔라인, 허용오차 0, `seed===fill`이면 즉시 복귀. 1회성 `commit`.
- 이동·업·키 동작 없음. 같은 색 클릭은 알림 후 `edit.cancel()`.

## 10. eyedropper / hand — 단일 도구

| 파일 | 줄 | 공개 API | 비고 |
|---|---:|---|---|
| `eyedropper.js` | 126 | `EyedropperTool` | 좌=주색·좌+Shift=보조색·우=보조색·Alt=활성 레이어 원시값·기본=표시 합성색. 우클릭도 수락하므로 `acceptsButton`가 2를 허용한다. 드래그 중 연속 채취. |
| `hand.js` | 9 | `HandTool` | 전부 베이스 상속. 실제 팬은 `InputController._shouldPan`이 하므로 도구는 그리기 없다. |

- 투명 픽셀 채취는 "투명 픽셀" 알림 후 미갱신(`a=0`은 생성 색 규칙 위반).
- 스포이트 커서·오버레이는 SVG data URL + 키워드 폴백.

## 11. color — 색상 패널 8파일

| 파일 | 줄 | 공개 API |
|---|---:|---|
| `panel_color.js` | 133 | `mountColor(root,{session,palette,getRecent,saveRecent})` (+`DEFAULT_PALETTE` 재수출) |
| `panel_color_fields.js` | 135 | `normHex`·`isFocused`·`resolveColorFields`·`paintColorFields`·`makeSwatchButton`·`mountColorFields`·`CHANNELS` |
| `panel_color_wheel.js` | 275 | `createColorWheel`·`mountWheel`·`rgbToHsv`·`hsvToRgb`·`angleToHue`·`hsvToHex`·`WHEEL_PX`/`RING_OUT`/`RING_IN`/`RING_MID`/`RING_CLEARANCE`/`SV_PX` |
| `color_palette.js` | 36 | `mountPalette`·`syncActiveSwatches`·`DEFAULT_PALETTE`(32색 frozen) |
| `color_recent.js` | 51 | `buildRecentGrid`·`paintRecent`·`pushRecent` |
| `color_slots.js` | 28 | `mountSlotButtons(ui)` |
| `color_events.js` | 31 | `mountSessionEvents(ui)` |
| `color_actions.js` | 20 | `colorSwap`·`colorReset` |

- **쓰기 SSOT는 한 곳**: `panel_color.js`의 `applyColor(ui,key,hex,{record})`.
  무효 입력이면 읽기만 하고 `false`. 성공 시 lock으로 재진입을 막고
  `setSetting → paint → (record) pushRecent`.
- 읽기 SSOT는 `canonicalHex(ui)` (활성 슬롯 → `normHex`, 실패시 `#000000`).
- `color_actions.js`(키보드)와 `color_slots.js`(버튼)는 같은 `setSetting` 경로를 쓰므로
  재칠·최근색 기록이 양쪽에 동일하게 걸린다(`color_events.js`).
- `DEFAULT_PALETTE` 32색의 **순서는 불변** — `color_recent`의 고정칸 수가 이 길이에서
  파생되므로 바꾸면 스냅샷 테스트가 깨진다.
- 휠 크기 상수는 `style_picker.css`의 176/98과 맞물린다. 한쪽을 고치면 다른 쪽도.
- **`WHEEL_PX=176`, `SV_PX=floor(2·(RING_IN−CLEARANCE)/√2)`=98.** 하드코딩하면
  모서리가 잘린다.

## 12. 피처별 한 줄 요약

| 피처 | 한 줄 |
|---|---|
| viewport | 줌·팬·클램프 수학과 뷰 옵션. **DOM-free 코어 + panel 한 개** |
| document | 문서·히스토리·세션·설정 검증. **전부 DOM-free** |
| layers | 레이어 모델·Command·편집 세션·패널 |
| pen | 펜/지우개·브러시 지형·브러시 패널 |
| shape | 도형 5종 + 기하 + 오버레이 + 옵션바 블록 |
| fill | 4방향 스캔라인 채우기 하나 |
| eyedropper | 버튼 3종을 모두 받는 채취기 |
| hand | 팬은 InputController가 하므로 껍데기 |
| color | 슬롯·필드·휠·팔레트·최근색 파이프라인 |

## 13. 피처 간 의존 (금지가 아닌 방향 기록)

현재 존재하는 크로스 피처 엣지는 세 방향뿐이다. 늘어나면 DAG 문서를 함께 고친다.

| 출발 | 도착 | 이유 |
|---|---|---|
| `features/layers/*` | `core/*` | 합성·픽셀·좌표 |
| `features/shape/*` | `core/raster/*` | 마스크·스냅·적용 |
| `io/*` | `features/document`·`features/layers` | `Document`·`Layer` 복원 |

- `features → ui` 역의존은 **없다**. 옵션바·액션을 피처로 옮길 때도 `safe`는 셸이
  주입하는 방식으로 지켰다(`panel_color.js`의 `ui.safe`와 같은 패턴).
- `core`는 어떤 모듈에도 의존하지 않는다.
