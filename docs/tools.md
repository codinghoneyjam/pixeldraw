# tools — 입력·펜·채우기·도형

## 역할 요약

- 포인터·휠·키 입력을 캔버스 정수 픽셀 이벤트로 정규화(`InputController`).
- 활성 도구 1개만 전달(`ToolManager`). 전환 시 이전 도구 `cancel`+`deactivate`.
- 모든 픽셀 기록은 `Session.beginEdit` 경유. 도구가 ChunkStore를 직접 만지지 않음.

## 파일별 API

### tool_base.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `Tool` | `constructor(env={})` | `{session, getView, requestRender}` 보관 |
| — | `get id` / `get cursor` | 식별·커서 |
| — | `activate()/deactivate()/pointerDown/Move/Up()/cancel()/hover()/keyDown()` | 기본 no-op(`keyDown` false) |
| — | `hasPending()` / `discardPending()` / `overlay(ctx, info)` | 보류 상태·프리뷰 훅 |

### tool_manager.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `ToolManager` | `constructor({session, getView, requestRender})` | `activeTool` 설정 구독 |
| — | `register(tool)` / `get(id)` / `get active` / `get activeId` | 등록·조회(미등록 접근 `INVALID_STATE`) |
| — | `pointerDown/Move/Up(ev)` / `cancel()` / `hover(ev)` / `keyDown(ev)` | 활성 도구 위임 |
| — | `get overlay` / `get cursor` / `dispose()` | 바인딩된 오버레이·커서·구독 해제 |

### input_controller.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `WheelAccumulator` | `constructor()` / `reset()` / `feed(deltaY, deltaMode, pageHeight)` | 60px당 1스텝, 방향 전환 시 잔량 교체 |
| `InputController` | `constructor({host, session, toolManager, getView, setView, getViewportSize, onHover, requestRender})` | 호스트 바인딩 설정 |
| — | `attach()` / `dispose()` | 리스너 등록·해제 |

정규화 이벤트: `{x,y(정수),fx,fy,sx,sy,button,shift,ctrl,alt,pointerId,pointerType,coalesced,timeStamp}`. coalesced 중복 제거+끝점 보장. 중클릭·스페이스·hand 도구는 팬. Shift+휠은 수평 팬, 휠은 앵커 줌. Space 키·blur·visibilitychange 시 스트로크 취소 안전 처리.

### pen.js (펜+지우개)

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `PenTool` | `constructor(env={}, {mode="draw"})` | draw→id `pen`, erase→`eraser` |
| — | `pointerDown/Move/Up(ev)` / `cancel()` / `hover(ev)` / `overlay(ctx, info)` | 스트로크 생명주기·프리뷰 |

동작: `beginEdit`→`strokePoint`→이동마다 `strokeSegment`(coalesced 순회)→`flush`→`pointerUp`에서 `commit`. Shift+클릭은 마지막점 직선 후 즉시 확정. Alt+클릭은 표시 합성색 스포이트(주색 갱신). 지우개는 packed=0. 잠금/숨김층이면 스트로크 무시. zoom≥2에서 footprint 프리뷰, 미만은 십자선.

프리뷰 위치 규칙: `overlay`는 **스트로크 중일 때만** `_hover ?? _lastPoint`으로 폴백한다. 폴백을 무조건 적용하면 포인터가 캔버스를 떠난 뒤 `hover(null)`로 `_hover`가 비워져도 마지막 스트로크 픽셀에 프리뷰가 남고, 재진입 시 낡은 위치에서 스폰된다. 스트로크 중이 아니면 `_hover`(없으면 `null` → 미표시)만 따른다.

### eyedropper.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `EyedropperTool` | `constructor(env={})` | id `eyedropper` |
| — | `pointerDown/Move/Up()` / `hover()` / `overlay()` / `cancel()` / `deactivate()` | 드래그 연속 채취 |

클릭=주색, Shift+클릭=보조색, Alt=활성 레이어 원시값, 기본=표시 합성색. 투명은 "투명 픽셀" 알림.

버튼 매핑: 좌클릭=주색 / 좌클릭+Shift=보조색 / **우클릭=보조색** / Alt=활성 레이어 원시값(버튼과 직교). 가운데클릭은 무시.

버튼 수용은 도구 소유다. `acceptsButton(button)`를 오버라이드하고 `input_controller`는 활성 도구의 메서드를 물어본다. **메서드가 없으면 좌클릭만**으로 폴백하므로 다른 도구는 동작이 그대로다. `pointermove`는 `button === -1`을 보고하므로 버튼은 `pointerDown`에서 래치(`_btn`)해야 한다 — 그러지 않으면 우클릭 드래그가 보조색으로 시작하다가 이동마다 주색으로 뒤집힌다.

### fill.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `floodFillScanline` | `floodFillScanline(writer, get, w, h, sx, sy, seed, fill)` | 4방향 스캔라인 채우기(허용오차 0) |
| `FillTool` | id `fill`, `pointerDown(ev)` | 활성층 seed→주색 채우기, 1회성 Command |

동일색이면 알림 후 `cancel`. 캔버스 밖 클릭 무시.

### hand.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `HandTool` | id `hand`, cursor `grab` | 그리기 없음, 팬은 InputController 담당 |

### shape.js + shape_geom.js + shape_overlay.js

`ShapeTool` (Tool 상속 아님, 동일 덕타이핑): `constructor(env, kind)` — kind는 line/rect/rrect/ellipse.

| 이름 | 시그니처 | 설명 |
|---|---|---|
| 상태 | `hasPending()/getPending()/setPending(v)/discardPending()/cancel()` | pending 상태머신(idle/drawing/pending/resizing/moving) |
| — | `pointerDown/Move/Up(ev)` / `keyDown(ev)` | 드래그 생성·핸들 리사이즈·이동 |
| — | `commit()` / `paintPreview(writer)` / `overlay(ctx, info)` | 확정(히스토리 기록)·테스트 훅·WYSIWYG 프리뷰 |
| `shape_geom` | `layoutHandles/hitHandle/insideShape/dragGeom/resizeGeom/moveGeom/parseLineGeom/parseBoxGeom/buildSpec/drawSpec/geomFromSpec` + `distSeg` + `HANDLE_IDS` | 핸들·히트·지오메트리 순수 함수 |
| `shape_overlay` | `packOf(css)/computePaintLayers(spec,settings)/deviceOf(view,dpr,x,y)/renderOverlay(...)` | 색층 전개·오버레이 렌더 |

조작: 빈 클릭 확정(commit), 핸들 드래그 리사이즈, 내부 드래그 이동, Enter 확정, Esc 취소, 방향키 nudge(Shift=32px). 활성층 변경 시 자동 commit, 층 삭제 시 보류 파기. `both` 모드는 바깥=보조색+테두리=주색.

## 핵심 흐름 (스트로크 생명주기)

1. `pointerDown`→`session.beginEdit`→`strokePoint`→`flush`(부분 렌더).
2. `pointerMove`→coalesced 점 순회 `strokeSegment`→`flush`.
3. `pointerUp`→잔여 구간 연결→`edit.commit`→`PaintCommand` 기록.
4. 취소(Esc·포커스 상실·도구 전환)→`edit.cancel`→원복 + dirty 재렌더.

## 주의점

- 도구 순수 로직은 Node import 가능해야 함(DOM 참조 금지).
- viewport 밖 좌표는 `PixelWriter.set`이 무시하므로 도구가 별도 클리핑 불필요.
- 잠금/숨김층 `beginEdit` 실패는 조용히 무시(펜) 또는 보류 파기(도형).
- 모듈 규약: 상대 경로 ESM import만, `export default`·동적 `import()`·`import.meta` 없음 (`contract.md` §5).
