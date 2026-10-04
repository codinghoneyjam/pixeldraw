# tools — 입력·도구·커밋

## 1. 개요·3원칙

- 입력→위임→커밋: `InputController`가 DOM 이벤트를 정규화 → `ToolManager`가 활성 도구 1개에 위임 → 도구가 `Session.beginEdit` 경유로만 기록.
- 원칙 1 (단일 활성): 활성 도구는 항상 1개. 전환 시 이전 도구 `cancel()` + `deactivate()`, 다음 도구 `activate()`.
- 원칙 2 (픽셀 직접 금지): 도구는 ChunkStore를 직접 만지지 않음. `beginEdit → writer → flush/commit(cancel)` 경로만 사용.
- 원칙 3 (Node 실행 가능): 도구 순수 로직은 DOM import 없이 Node에서 실행 가능. DOM은 `InputController`와 오버레이 렌더에만 격리.

## 2. Tool 베이스 (`tool_base.js`)

| 메서드 | 시그니처 | 설명 |
|---|---|---|
| `Tool` | `constructor(env={})` | `{session, getView, requestRender}` 보관 |
| — | `get id` / `get cursor` | 식별자·커서 (`tool`/`default` 기본값) |
| — | `activate()` / `deactivate()` | 진입·이탈 훅 (기본 no-op) |
| — | `pointerDown/Move/Up(ev)` | 스트로크·드래그 생명주기 (기본 no-op) |
| — | `cancel()` / `hover(ev)` | 중단·프리뷰 위치 갱신 (기본 no-op) |
| — | `keyDown(ev): boolean` | 처리 시 `true`, 기본 `false` |
| — | `hasPending(): boolean` / `discardPending()` | 보류 객체 유무·파기 |
| — | `overlay(ctx, info)` | 프리뷰 렌더 훅 |

## 3. ToolManager (`tool_manager.js`, 120줄)

- `constructor({session, getView, requestRender})`: `session` 필수, `SETTINGS_CHANGED.activeTool` 구독.
- `register(tool)` / `get(id)` / `get active` / `get activeId`: 등록·조회. 미등록 `active` 접근은 `INVALID_STATE`.
- 위임: `pointerDown/Move/Up(ev)`·`hover(ev)`·`keyDown(ev)` → `active`에 그대로 전달. `keyDown` 반환값 전파.
- `cancel()` 체인: `active.cancel()`을 try/catch로 감싸 삼킴 (이미 정리된 도구·입력 경로 보호).
- 전환 `_switchTo(nextId)`: 동일 ID면 복귀. `session.isEditing`이면 `prev.cancel()` 먼저 → `prev.deactivate()` → `_activeId` 갱신 → `next.activate()`. 각 단계 try/catch로 전환 차단 금지. 미등록 next면 ID만 갱신 후 종료.
- `get overlay`: `active.overlay` 바인딩 반환 (미등록·비함수면 `null`). `get cursor`: `active.cursor`, 실패 시 `default`. `dispose()`: 설정 구독 해제.

## 4. InputController + WheelAccumulator (`input_controller.js`, 432줄)

- `WheelAccumulator`: `constructor/reset/feed(deltaY, deltaMode, pageHeight)`. 60px당 1스텝, `deltaMode` 1=×16·2=×pageH. 방향 전환 시 잔량 교체, 반환 `-steps` (휠업=줌인).
- ToolEvent 정규화 (`buildToolEvent`): `{x,y(정수 픽셀),fx,fy(부동),sx,sy(스크린),button,shift,ctrl,alt,pointerId,pointerType,coalesced,timeStamp}`. `ctrl`는 metaKey 포함.
- `coalesced`: `getCoalescedEvents` 순회 → 정수 변환 → 연속 중복 제거 → 끝점(`pointer` 본체) 보장. 실패 시 본체 단일점 폴백.
- 팬 판정 `_shouldPan`: 중클릭(1)·스페이스 홀드·활성 `hand` 중 하나면 팬. 팬은 `panBy+clampView+setView+requestRender`, 커서 `grabbing`.
- 버튼 수락 `_toolAcceptsButton`: 활성 도구의 `acceptsButton(button)`에 질의, 메서드 없으면 좌클릭(0)만. 불허 버튼은 무시.
- pointerdown: 팬 우선(0/1번만) 후 도구 위임. 그리기 포인터 1개(`_drawPointerId`) 래치 + `setPointerCapture`.
- pointermove: 팬 중이면 팬, 그리기 중이면 `pointerMove` 위임, 평시이면 `hover({x,y,fx,fy})` + `onHover`.
- 종료: `pointerup`→`pointerUp`, `pointercancel`·`lostpointercapture`→`cancel()`. 종료 후 `_inside` 거짓이면 hover 정리, 커서 재적용.
- 휠: `preventDefault` 필수. Shift+휠=수평 팬(`panBy(-deltaY,0)`), 일반=앵커 줌(`stepZoom→zoomAt(sx,sy)→clampView`).
- blur·취소 안전: window `blur`·`visibilitychange(hidden)` 시 스페이스·팬·그리기 상태 전부 초기화 + `toolManager.cancel()`. Space keydown/up은 입력 포커스(input/textarea/contentEditable)에서는 무시. 커서 상태머신: outside > panning > space > tool.

## 5. Pen (`pen.js`, draw/erase)

- `constructor(env, {mode="draw"})`: draw→id `pen`, erase→`eraser`. 커서는 SVG 원(펜)/사각(지우개) + crosshair/cell.
- `pointerDown`: 좌클릭만. `Alt`=표시 합성색 스포이트(주색 갱신, 투명 알림). `beginEdit` 실패(잠금/숨김층)=조용히 무시. Shift+클릭=`_lastPoint`→현점 `strokeSegment` 후 즉시 `commit`.
- 진행: `pointerDown=strokePoint+flush` → `pointerMove=coalesced 순회 strokeSegment+flush` → `pointerUp=잔여 구간 연결 후 commit(라벨 연필/지우개)`.
- 색: 지우개 `packed=0`, 펜은 `primaryColor` 파싱 실패 시 검정 불투명 폴백.
- 오버레이: 스트로크 중에만 `_hover ?? _lastPoint` 폴백, 평시 `_hover`만 (stale 프리뷰 방지). `zoom≥2`면 `brushFootprint` 마스크+difference 테두리, 미만이면 십자선.

## 6. Eyedropper (`eyedropper.js`)

- `acceptsButton`: 0(좌)·2(우)만 수락, 중클릭 무시. `pointerDown`에서 `_btn` 래치 (move는 `button=-1`이므로 래치 필수).
- 규칙: 좌=주색, 좌+Shift=보조색, 우=보조색(Shift와 무관), Alt=활성 레이어 원시값, 기본=표시 합성색. 드래그 중 연속 채취, `pointerUp/cancel/deactivate`에서 래치 해제.
- 투명(`a=0`)이면 "투명 픽셀" 알림 후 미갱신. 오버레이는 1픽셀 difference 박스.

## 7. Fill (`fill.js`)

- `floodFillScanline(writer, get, w, h, sx, sy, seed, fill)`: 4방향 스캔라인, 허용오차 0, `seed===fill` 즉시 복귀.
- `FillTool` id `fill`: 좌클릭·범위 내만 처리. `beginEdit(페인트통)` → 활성층 seed vs 주색 비교 → 동일색이면 알림 후 `edit.cancel()` → 다르면 스캔라인 채우기 → 1회성 `commit`. 이동·업·키 동작 없음.

## 8. Hand (`hand.js`)

- id `hand`, cursor `grab`. 메서드 전부 베이스 상속. 실제 팬은 InputController가 `_shouldPan` 분기로 수행하므로 도구 자체는 그리기 없음.

## 9. Shape (`shape.js`, Tool 상속 아님·덕타이핑)

- `constructor(env, kind)`: kind=line/rect/rrect/ellipse. `SETTINGS_CHANGED(LIVE_KEYS: penSize/shapeFill/shapeRadius/snapUnit/shapeLockAspect)`·`LAYERS_CHANGED`·`DOCUMENT_REPLACED` 구독. 색상은 LIVE 키가 아님 (pending이 그려진 시점 색 스냅샷 소유).
- 상태머신: idle → drawing(드래그 중) → pending(확정 대기) → resizing/moving(핸들·내부 드래그). `hasPending/getPending/setPending/discardPending/cancel/commit` 제공.
- `pointerDown`: pending 위에서 핸들 히트→resizing(orig 복제), 내부→moving, 빈 곳→`commit()` 후 새 그리기 시작. `pointerUp`: 제자리 클릭은 파기, 드래그는 `dragGeom`→pending(주색 스냅샷) 저장.
- `keyDown`: Enter=확정(`true`), Esc=진행 중 취소·보류 파기, Delete/Backspace=보류 파기, 방향키=nudge(Shift=32px, 아니면 1px, pending 모드만).
- `commit()`: `beginEdit({layerId: pending.layerId})` → `applyShape(writer, spec, fg, fg)` → `commit`. 잠금/숨김/없음 층이면 보류 파기 후 `false`. 활성층 변경 시 자동 commit, 층 삭제·문서 교체 시 보류 파기.
- 프리뷰: `_previewSpec`(drawing=`drawSpec`, pending=`buildSpec`) → `paintPreview(writer)`·`overlay=renderOverlay`. 색상 포함 캐시 키로 stale 방지.

## 10. shape_geom (순수함수 12 + 상수)

| 함수 | 설명 |
|---|---|
| `HANDLE_IDS` | `nw/n/ne/e/se/s/sw/w` 고정 순서 |
| `distSeg` | 점-선분 거리 (라인 그랩용) |
| `layoutHandles` | 라인 끝점 2개 / bbox 8핸들 배치 |
| `hitHandle` | 허용오차 내 핸들 히트 |
| `insideShape` | bbox 포함 / 라인 그랩폭 판정 |
| `dragGeom` | 신규 드래그 지오메트리 (Shift=angleSnap, lock/center, snap) |
| `resizeGeom` | 핸들 리사이즈 (라인=고정점+angleSnap) |
| `moveGeom` | 정수 델타 이동 (snap 시 32px 그리드) |
| `parseLineGeom` | 옵션바 `{x0,y0,x1,y1}` 또는 bbox 검증 파싱 |
| `parseBoxGeom` | 옵션바 `{x,y,w,h,radius}` 검증 (w,h≥1, r≥0) |
| `buildSpec` | pending→커밋용 ShapeSpec (penSize/shapeFill/radius 반영) |
| `drawSpec` | 그리기 중 프리뷰 Spec |
| `geomFromSpec` | Spec→핸들용 pending 유사 지오메트리 역변환 |

## 11. shape_overlay (공개 4 + 내부 3)

- `packOf(css)` / `computePaintLayers(spec, color)` / `deviceOf(view,dpr,x,y)` / `renderOverlay(ctx,spec,layers,box,handles,view,dpr,tmp)`.
- `computePaintLayers`: line=`lineMask`, ellipse=`ellipseMask`, rect/rrect=`rrectMask`. fill 모드=외곽 전체, 아니면 `outlineRing`. 전경 1색만 페인트 (배경 슬롯 미사용).
- `renderOverlay`: DOM 있으면 임시 캔버스 블릿(`blitLayer`), 없으면 `fillLayer` 폴백 → 파선 bbox + 8px 고정 핸들(`drawFrame`).

## 11-1. 파일 구성과 미해결 사항 (2026-10-01 확인)

`src/tools/`는 10개 파일이다. `input_controller.js`(432줄)가 분리 대상 1위이고
`shape.js`(333줄)가 2위다. 둘 다 250줄 권장선을 넘으며 `wheel_accumulator.js`·
`tool_event_builder.js`·`cursor_state.js`·`shape_state.js`·`shape_pointer.js`·
`shape_keyboard.js`는 **계획만 있고 미구현** 상태다.

- `input_controller.js`에 `WheelAccumulator`·`buildToolEvent`·커서 상태 머신이 인라인 상태다.
- `shape_overlay.js`는 존재하며 `computePaintLayers`가 `core/raster/raster_masks.js`와 `core/raster/segment.js`를 import 한다.
- **커서 기대값은 해소되었다(2026-10-01).** `pen.js`·`eyedropper.js`·`fill.js`의
  `get cursor()`는 SVG `url("data:image/svg+xml,...") <hx> <hy>, <fallback>` 형태다.
  `tests/tools_basic.test.mjs`가 `"crosshair"`를 하드코딩해 3건이 실패했으나, 문서
  (`ui.md` §1, `tools.md` §5)가 SVG 커서를 규범으로 서술하므로 **테스트가 낡은 것**이었다.
  `cursorFallback()` 헬퍼를 추가해 SVG URL 형태와 폴백 키워드만 검증하도록 바꿨다
  (`pen`·`eyedropper` → `crosshair`, `fill` → `copy`). 커서 시각을 바꿔도 깨지지 않는다.
- `shape_geom.js`는 `core/raster/raster_snap.js`를, `shape.js`는 `core/raster/shape_raster.js`를 import 한다.

## 12. 대표 플로우 (포인터→도구→커밋)

1. `pointerdown` → 팬 판정(팬이면 팬 시작) → 아니면 버튼 수락 확인 → `ToolManager.pointerDown(ToolEvent)` → 도구 `beginEdit+strokePoint+flush`.
2. `pointermove(coalesced)` → `pointerMove` → `strokeSegment` 순회 + `flush` (부분 렌더).
3. `pointerup` → 잔여 구간 연결 → `edit.commit` → `PaintCommand` 히스토리 기록. Esc·blur·도구전환 → `edit.cancel` + dirty 재렌더.
4. 도형: `pointerDown(빈곳)=commit 후 새 그리기` → `pointerUp=pending 저장` → 핸들/이동/수치 편집 → `Enter·빈클릭·층전환=commit`.

## 13. DOM 격리

- DOM 접촉 허용: `InputController`(리스너·캡처·커서·wheel·blur/visibility)와 `shape_overlay.blitLayer`(임시 canvas)만.
- 도구 본체(`pen/eyedropper/fill/hand/shape/shape_geom`)는 DOM 참조 금지 — `pixelToDevice·view·dpr`는 `overlay(ctx, info)` 인자로만 주입.
- 좌표 클리핑은 도구가 하지 않음 (`PixelWriter.set`이 viewport 밖 무시). 모듈 규약: 상대 경로 ESM import만, `export default`·동적 `import()`·`import.meta` 없음.
