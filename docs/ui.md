# ui — 셸·패널·색·다이얼로그·크롬 (src/ui 36파일 = 루트 31 + actions/ 5)

> Session 메서드 호출+구독만. 모델 직접 변조 금지. 전역 싱글턴 직접 접근 없음.
> 규약: `mount*(root,deps)→()=>{}` dispose 반환. `!root||!document`면 no-op dispose.
> 리스너는 `disposers[]` 수집 후 일괄 해제. `setHidden/clearChildren` 외 DOM 직접 조작은 mount 내부로 한정.

## 1. boot (2종) — 조립+뷰 소유

### app.js (261줄) — `boot()` (export) + `createHistoryButtons/createMenubar/refreshPanelCanvases` 재수출
- 9단계: 1 설정(`dt.settings.v1`→PERSIST_KEYS 9종)+최근색(`dt.recentColors`,RECENT_MAX=32) 로드
- 2 AutosaveStore.open→peek→`showRestoreDialog`→restore(loadDocument)/discard(clear), 미복원시 newDocument(DEFAULT_DOC)
- 3 ToolManager 등록(pen/erase/fill/eyedropper/shape×4/hand)+CanvasRenderer.attach+InputController.attach+`viewStore.fit()`, fit후 `viewStore.subscribe(render)`
- 4 `mountStatus/mountColor/mountBrush/mountLayers/mountOptions/mountCollapsibleSections`+panelDisposers 수집
- 5 `store.attach(session)` 6 resize(renderer.resize+view 재clamp)+beforeunload(dirty 가드)
- 7 DOCUMENT_REPLACED→fit, STATUS_MESSAGE warn/error→toast(4s/8s) 8 `createShortcuts` 9 `?debug`→`window.__drawTool`
- 액션표: file.new/open/save/exportLayer/importLayer/exportPng, edit.undo/redo, canvas.resize, view.zoomIn/out/fit/actual/gridCycle, layer.add/duplicate/remove/mergeDown/up/down, brush.step/color.swap/reset. menubar `button[data-action]`→actions, toolbar `button[data-tool]`→setSetting(activeTool)+`iconFor` 채움+aria-pressed 동기화.
- 구독: SETTINGS_CHANGED(activeTool→툴바, 전체→250ms 디바운스 영속), DOCUMENT_REPLACED, STATUS_MESSAGE.
- 주의: `#dt-app/#dt-canvas-host` 없으면 throw. `runAction`이 DrawToolError→notify, 그외→toast+console.

### view_store.js — `createViewStore(host,session)` → `{get/set/subscribe/center/zoomTo/zoomIn/zoomOut/fit/actual}`
- `clampView/fitView/stepZoom/zoomAt/actualSizeView`(render/view.js)에 위임. set은 항상 clamp+emit. get은 복사본.
- viewport는 host.clientWidth/Height, 실패시 800×600. center=viewport/2. zoomTo는 center 기준 zoomAt.
- 구독: Session 직접 구독 없음. 역으로 app·statusbar가 `subscribe(refresh/render)`로 소비.
- 주의: doc 없으면 clamp는 복사만, fit/actual은 no-op. NaN·음수 zoom 무시.

## 2. panel (5종) — 도구·층·옵션 패널

### panel_brush.js — `mountBrush(root,{session})` + `SIZE_PRESETS(16단계 1–64 frozen)/paintPreview/presetColumns/presetRows/stepPenSize`
- sync(from): num/range/프리셋 aria-pressed/64×64 `paintPreview(brushFootprint 마스크 중앙 1:1)` 동기화. setSize→setSetting, 실패시 notify+resync.
- `presetColumns(count)`: 4부터 최소약수, 없으면 4. `presetRows=ceil(count/cols)`. CSS `--dt-preset-cols/rows`+`dt-preset-grid`로 4×4.
- 구독: SETTINGS_CHANGED(penSize→sync, primaryColor→preview만).
- 주의: 셀 `min-width:0` 필수(32px 잔재시 6/5/5 감김). 휠 입력은 `bindWheelInputs`(Shift=5단위).

### panel_layers.js — `mountLayers(root,{session})` + `paintThumb(canvas,layer)`(32×32 최근접)
- 불투명도 쌍동기+역순(top-first) 리스트박스+썸네일(250ms 스로틀)+추가/복제/삭제/병합/위·아래+더블클릭 개명+드래그 미구현시 버튼 순서.
- 구독: DOCUMENT_REPLACED/LAYERS_CHANGED/HISTORY_CHANGED→전체 리렌더+썸네일, SETTINGS_CHANGED 무시.
- 주의: 위로=index+1(뒤가 위). 최하층 병합 비활성. 썸네일은 `refreshPanelCanvases`에서도 재호출.

### panel_options.js — `mountOptions(root,{session,toolManager,view})` (+`mountCollapsibleSections` 재수출)
- 그리드 셀렉트+줌 셀렉트(ZOOM_LEVELS)+fit/actual+펜크기 라벨+도형(fill 라디오/radius/lock/snap/bbox x/y/w/h+commit/cancel)+`createTooltips` 내장.
- syncSections: `[data-for-tools]` 표시전환, line은 채움 비활성, rrect만 radius 활성, 보류 있을때만 bbox 활성. 숨김 섹션은 `tips.closeWithin`.
- 구독: SETTINGS_CHANGED(activeTool·grid·shape계)→sync, shape 보류 이벤트→bbox.
- 주의: 줌 셀렉트는 view에 위임. 옵션바 `overflow:hidden`이라 툴팁은 body 부착.

### collapsible_sections.js — `mountCollapsibleSections(roots,{onExpand})`
- `section[data-collapsed]`+`.dt-panel-head` 클릭 토글→`data-collapsed/aria-expanded`+`dt.settings.v1.collapsedPanels`에 병합 영속.
- 펼침시 `onExpand(id,section)` 1회 호출. app은 여기에 `refreshPanelCanvases`를 건다.
- 주의: localStorage 파손시 `{}` 폴백. id 없는 섹션은 영속 생략·토글만.

### panel_refresh.js — `refreshPanelCanvases(session)`
- 브러시 preview+`li[data-layer-id]`별 `paintThumb` 재도색. 접힘→펼침시 캔버스 백킹스토어 유실 복구용.
- 주의: doc 없으면 레이어 순회 생략. mount 아님(dispose 없음).

## 3. color (9종) — 파사드+파이프라인+휠+32색

### panel_color.js (파사드) — `mountColor(root,{session,palette,getRecent,saveRecent})` + `DEFAULT_PALETTE` 재수출
- ui 객체: `{session,slot(PRIMARY),lock,wheel,fields,recentCells,recentCount=PALETTE.length,els,canonical/paint/apply/applyColor/pickSwatch/pushRecent/safe}` 조립 후 buildRecentGrid→paintRecent→mountPalette→mountColorFields→mountSlotButtons→mountWheel+mountSessionEvents→paint(force).
- SSOT: `canonicalHex(ui)` 유일 읽기(활성슬롯→normHex, 실패시 #000000). `applyColor(ui,key,hex,{record})` 유일 쓰기(normHex 무효시 paint후 false, lock+1→setSetting→lock-1→paint→record시 pushRecent). `paint(ui,force)`: lock>0이면 읽기만, 슬롯버튼 배경+aria-pressed→paintColorFields→wheel.setFromRgb→syncActiveSwatches. `pickSwatch(e,hex)`: Shift=반대슬롯, 그외 활성슬롯, record:true. `setSlot` 전환후 강제 paint.
- dispose: `makeDispose`가 그리드 셀 리스너(offs)+5 mount dispose 일괄 해제.
- 주의: recentCount는 palette 길이와 동기(기본 32). `deps.palette` 주입시 RECENT_CELLS도 연동.

### panel_color_fields.js — `normHex(v)/isFocused/makeSwatchButton/resolveColorFields/paintColorFields/mountColorFields` + `CHANNELS=[r,g,b]`
- `normHex=parseHex→a==255 검증→toHex`, 무효시 null. `paintColorFields(fields,hex,force)`: 포커스 가드(force면 덮어씀) 후 HEX+RGB 반영.
- `makeSwatchButton({hex,aria,emptyLabel,onPick,onContextMenu},offs)`: `.dt-swatch`, 빈칸은 `.is-empty`+disabled+checkerboard. 리스너 offs 등록.
- `mountColorFields(ui,fields)`: HEX Enter/commit·RGB Enter/commit+무효시 canonical 복원. dispose=offs 해제.
- 주의: 프로그램적 쓰기시 lock로 SETTINGS_CHANGED 재진입 무력화. 무효 입력은 정본값 복원.

### color_events.js — `mountSessionEvents(ui)`
- SETTINGS_CHANGED(primary/secondary만)→`ui.paint(false)`+lock==0이면 pushRecent. HISTORY_CHANGED→undoLabel이 7종(연필·지우개·페인트통·직선·사각형·둥근사각형·타원)이면 primary pushRecent.
- 주의: 색 외 key는 무시. dispose는 removeEventListener 2종.

### color_palette.js — `mountPalette(ui,palette,offs)/syncActiveSwatches(ui)` + DEFAULT_PALETTE 재수출
- mount 1회 `replaceChildren`, 좌클릭→pickSwatch, 우클릭→보조슬롯 record. sync는 양 슬롯 색 집합과 `data-color` 비교 후 `data-active` 토글(타깃마커용).
- 주의: 매번 재생성 금지(클릭 대상 소실 원인). paint 경로에서만 sync 호출.

### color_palette_data.js — `DEFAULT_PALETTE` 32색 frozen (PICO-8 16+회색 8+채도 8)
- 주의: recent 고정칸 수도 이 length에서 파생. 순서 변경시 스냅샷 테스트 갱신.

### color_recent.js — `buildRecentGrid(ui,offs)/paintRecent(ui)/pushRecent(ui,hex)`
- build 1회 N셀 생성, 클릭→pickSwatch(disabled 가드). paint는 제자리 재칠, 미사용은 `.is-empty`+disabled+`빈 칸`. push는 dedup+앞삽입+`recentCount` cap→saveRecent→paint.
- 주의: `replaceChildren` 재생성 금지. 보조슬롯 활성시에도 활성슬롯 기준으로 기록(pickSwatch 경유).

### color_slots.js — `mountSlotButtons(ui)`
- fgbg 클릭+keydown→양 슬롯값 교환(applyColor 2회), reset→primary #000000/secondary #ffffff.
- 주의: swap 순서 고정(보조 먼저 읽고 쓰기). dispose=offs 해제.

### color_wheel.js — `mountWheel(ui)` (얇은 어댑터)
- `createColorWheel({container:els.wheelWrap,onChange:활성슬롯 record:false,onCommit:canonical pushRecent})` 연결. dispose=wheel.dispose.
- 주의: 휠은 paint()가 먹이는 순수 뷰. 드래그 중 onChange는 기록 없이 칠하고, 해제시 1회 기록.

### panel_color_wheel.js — `createColorWheel({container,onChange,onCommit})` + `WHEEL_PX=176/RING_OUT=88/RING_IN=74/RING_MID=81/RING_CLEARANCE=4/SV_PX=floor(2·(74−4)/√2)` + `rgbToHsv/hsvToRgb/angleToHue/hsvToHex`
- hue링+중앙 SV사각 캔버스 2개 생성(없으면 생성, `resolveCanvas`), 래스터 캐시+포인터 드래그 pickHue/pickSatVal. 반환 `{wheelCanvas,svCanvas,setHueSV,setFromRgb,dispose}`. DOM 없으면 null.
- 주의: SV 크기는 계산값 고정. 하드코딩시 모서리 잘림. style.css 176/98과 어긋나면 링 이탈. SV는 `border-radius:0` 정사각, 전(s,v) 선택 가능.

## 4. dialog (7종) — openModal 셸+재수출 파사드

### dialog_core.js — `openModal({title,build(body,bar,close),onAction})→{close,element,extra}` + `addButton(bar,label,onClick,{disabled,id})` + `numberField(body,id,label,value,step)`
- `#dt-dialog-root`에 overlay+`.dt-dialog[role=dialog][aria-modal]`+h2+body+buttons 생성. Esc→close(null), Tab 포커스트랩, overlay 클릭→close(null), 이전 포커스 복원+settled 1회 보장. 첫 focusable 자동 포커스. root 없으면 `{close:noop,element:null}`.
- 주의: document keydown을 capture로 등록, close시 해제. 중첩 모달은 호출자 책임(동시 1개 권장).

### dialogs.js (파사드) — `openModal/addButton/numberField`+5 show* 재수출のみ. app·액션은 이 파일만 import.
### dialog_confirm.js — `confirmDiscardChanges()→Promise<bool>` (DOM 없으면 false)
- "변경 버리기" 문구+취소(false)/버리기(true). onAction `v===true` 변환.
### dialog_new_doc.js — `showNewDocumentDialog()→Promise<desc|null>`
- 타일 단위 w/h(0.5–30/17)+프리셋 4종(2×2/8×8/16×16/30×17)+배경(transparent/흰/검)+이름. 32px 배수 검증, 무효시 에러문+생성 비활성.
### dialog_resize.js — `showResizeCanvasDialog(current)→Promise<{widthPx,heightPx}|null>`
- 32–1920×32–1088·32배수·정수 검증. input 이벤트마다 ok버튼+에러문 동기화.
### dialog_restore.js — `showRestoreDialog(info)→Promise<"restore"|"discard">` (DOM 없으면 "discard")
- 이름+updatedAt 표시, 버리기/복원 버튼. onAction restore以外 discard 정규화.
### dialog_progress.js — `showProgress(text)→{update(pct01),close()}`
- `#dt-app[aria-busy=true]`+`#dt-dialog-root`에 `.dt-progress[role=status]`+바. update는 0–1 clamp→width%. close는 노드 제거+aria-busy 복원. Node-safe no-op.
- 주의: 열림 중 `createShortcuts/isBusyUi`+`history_buttons/isBusy`가 입력·undo/redo 비활성.

## 5. chrome (8종) — dom·문구·아이콘·메뉴·히스토리·상태·단축키·툴팁

### dom.js — `el(tag,attrs,children)/qs(root,sel)/qsa(root,sel)/on(target,type,fn,opts)/setHidden/clearChildren` (Node-safe)
- el은 text/html/on*·boolean 속성 분기, 미지원시 스텁 `{tag,attrs,children}`. on은 해제함수 반환. setHidden은 toggleAttribute(hidden).
### strings.js — `STRINGS` frozen(메뉴·액션·도구·단축키·힌트·패널·상태·다이얼로그·토스트)+`msg(key,fallback)` (키 미보유시 fallback)
- 주의: 한글 문구는 반드시 여기 경유. 하드코딩 금지.
### icons.js — `ICONS` 15종 frozen(pen/eraser/fill/eyedropper/line/rect/rrect/ellipse/hand/eye/eyeOff/lock/unlock/swap/plus)+`iconFor(id)/hasIcon(id)`
- iconFor는 SVG 문자열, 미보유시 한글 폴백(펜·지·통·스·선·사·둥·타·손), 그외 `?`.
### menubar.js — `createMenubar(root)→{closeAll,dispose}` (MENU_CLOSE_DELAY_MS=220)
- `.dt-menu` hover-open+click 토글+leave 220ms 예약닫힘+바깥 pointerdown 닫힘. trigger `aria-expanded`+패널 hidden 동기화.
### history_buttons.js — `createHistoryButtons(root,session)→dispose`
- `.dt-menubar-actions button[data-action=edit.undo/redo]` disabled를 `canUndo/canRedo`+busy(dialog 존재·aria-busy)로 동기화. 구독: HISTORY_CHANGED/DOCUMENT_REPLACED+dialogRoot·app MutationObserver.
### statusbar.js — `mountStatus(root,{session,view})→dispose{setCursor,refresh}` + `formatGridCoords/formatPosition/OUTSIDE_POS`
- `formatGridCoords`: floor 1회→px문구+셀(32px)·타일(64px) 동시 파생. `formatPosition(null/NaN)`→`- , -` 양필드. 마운트시 setCursor(null)+refresh 선채움.
- refresh: 줌%+캔버스(`W×H px(T×T 타일)`)+도구명(STRINGS.tools)+dirty `●`. message는 info 4s/warn 8s/error 상주.
- 구독: STATUS_MESSAGE→show, DOCUMENT_REPLACED/HISTORY_CHANGED/LAYERS_CHANGED→refresh, SETTINGS_CHANGED(activeTool만), view.subscribe(refresh). onHover(pos)→setCursor는 input 경유.
### shortcuts.js — `resolveShortcut(desc)→{kind}|null` + `isEditableTarget(target)` + `createShortcuts({toolManager,session,actions,requestUndo,requestRedo,isBusy})→{handleKeyDown,dispose}`
- 매핑: Ctrl+N/O/S/E, Ctrl+Shift+N(layer.add)·J·M(merge)·]/[(up/down), Ctrl+'/그리드, Ctrl+Z/Y·Shift+Z, B/E/G/I/L/R/U/O/H, X(swap)D(reset), [/](Shift 5단위), +/-/0/1. Alt 무조건 null. Space false(핸드 스크롤에 양보).
- 순서: 입력요소 가드(Esc-blur만 허용)→busy(다이얼로그·aria-busy)→toolManager.keyDown 우선소비→resolve→preventDefault후 실행(undo/redo→requestUndo 우선, tool→setSetting, penSize→brush.step 우선, action→actions[]).
- 주의: window keydown에 1회 바인딩, dispose 해제. `?debug`와 무관.
### tooltip.js — `createTooltips(root)→{dispose,close,closeWithin,isOpen}` + `TIP_AUTO_HIDE_MS=2500`
- `.dt-tip-trigger(data-tip)` hover/focus/클릭(pin)→`document.body`에 `.dt-tooltip` 1개 표시, left/top만 인라인, clampBox로 뷰포트 고정. mouseleave/focusout/Esc/바깥클릭/유휴 2500ms로 닫힘+`aria-expanded` 갱신.
- 주의: 옵션바 `overflow:hidden`이라 body 부착 필수. `hidden` 섹션은 `closeWithin(sec)`로 함께 닫기. 텍스트 400자 cap.

## 6. actions/ (5종) — 액션 핸들러

`app.js`의 `runAction`이 `data-action` 문자열로 호출하는 핸들러 묶음. 전부 Session
메서드 호출만 하며 모델을 직접 변조하지 않는다. `app.js`는 이 파일들만 import 한다.

### file_actions.js (110줄) — 파일 I/O 6종
- `doSave(session, toast)` / `doOpen(session, toast)` / `doExportLayer(session, toast)` / `doImportLayer(session, toast)` / `doExportPng(session, toast)` — 모두 `async`, `io`(`serialize`·`file_io`·`export_png`)와 `core`만 참조한다.
- `doNew(session)` — `async`. 변경 버리기 확인 후 `newDocument(DEFAULT_DOC)`로 교체한다.

### edit_actions.js (20줄) — 실행 취소/다시 실행 2종
- `requestUndo(session, toolManager)` / `requestRedo(session, toolManager)` — 편집 중(`isEditing`)이면 무시한다.

### view_actions.js (20줄) — 보기 6종
- `zoomIn(viewStore)` / `zoomOut(viewStore)` / `fit(viewStore)` / `actual(viewStore)` — `viewStore`에만 위임하며 Session을 받지 않는다.
- `gridCycle(session)` — `gridMode`을 `off→unit→tile→pixel` 순환한다.
- `canvasResize(session)` — `async`. `showResizeCanvasDialog` 결과로 `session.resizeCanvas(w,h)`를 호출한다.

### layer_actions.js (20줄) — 레이어 6종
- `layerAdd(session)` / `layerDuplicate(session)` / `layerRemove(session)` / `layerMergeDown(session)` / `layerUp(session)` / `layerDown(session)` — `session`만 받아 `model/layer_operations`에 대응하는 Session 메서드를 호출한다.

### tool_actions.js (15줄) — 도구·색 3종
- `brushStep(session, delta)` / `colorSwap(session)` / `colorReset(session)` — `setSetting` 경유만 한다.

## 조립·호출 순서 (파일 없이 재현)
1 index.html id 계약: `#dt-app/#dt-menubar/#dt-optionbar/#dt-toolbar/#dt-left-panels/#dt-right-panels/#dt-canvas-host/#dt-statusbar/#dt-dialog-root/#dt-toast`+패널 내부 id. 2 app.boot 9단계→viewStore→tools/render/input→status+color/brush/layers/options/collapsible→autosave→window→toast→shortcuts→debug. 3 색 입력 8경로(네이티브·HEX·RGB·휠·팔레트·최근색·슬롯·우클릭)→applyColor→paint 전파→SETTINGS_CHANGED 영속(250ms)+최근색. 4 문서교체→view.fit+레이어·상태 refresh. 5 다이얼로그 중 입력·단축키·히스토리버튼 잠금.

## 주의점 (횡단)
- ESM 상대경로만. `export default·동적import·import.meta` 금지. UI→model 직접 쓰기 금지(Session 경유).
- 숫자 필드 `.dt-color-fields` 4열(R/G/B+hex span2행), 라벨 `[data-ch]`. 컬러픽커 `.dt-picker→.dt-wheel-wrap→.dt-color-fields→팔레트→최근색`.
- 선택 타깃마커=`::before`+clip-path 귀퉁이 4개+`--dt-arm-l/d` 11px 고정. 컨테이너 `--radius-box`+`overflow:hidden`, 스와치 `--radius-pct`.
- 픽셀폰트 금지. 썸네일·프리뷰는 `pixelated`.

## Handoff

- Wrote: `draw_tool_v2/docs/ui.md`
- Result: `ui/` 36파일 반영(`actions/` 5개 §6 신설), 파일 수 31→36 정정, app.js 라인 수 기입
- Next: Orchestrator — 250줄 초과 4파일(`panel_layers` 264·`panel_color_wheel` 250·`pen` 261·`input_controller` 432) 2차 분리 판단
