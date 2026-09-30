# ui — 셸·패널·단축키·다이얼로그

## 역할 요약

- `app.js` 부팅 순서(설정→복원→도구/렌더/입력→패널→자동저장→윈도우 이벤트) 조립.
- UI는 Session 메서드 호출 + Session 이벤트 구독만. 모델 직접 변조 금지, 싱글턴 전역 상태 직접 접근 없음.
- 모든 한글 문구는 `strings.js` 중앙 관리, 아이콘은 인라인 SVG + 한글 폴백.

## 파일별 API

### app.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `boot` | `boot()` (export) | 9단계 부팅, `document` 없으면 미실행, `?debug` 시 `window.__drawTool` 노출 |
| viewStore | `get/set/subscribe/center/zoomTo/zoomIn/zoomOut/fit/actual` | app 소유 뷰 상태, 항상 clamp |

부팅 순서: 1 설정·최근색(localStorage) → 2 자동저장 peek·복원 선택 → 3 도구 등록(펜·지우개·통·스포이트·도형4·손)+렌더러+입력+fit → 4 패널·상태바 → 5 자동저장 attach → 6 resize·beforeunload → 7 문서교체→fit·알림 토스트 → 8 단축키 → 9 디버그. 액션 테이블: file.*·edit.*·canvas.resize·view.*·layer.*·brush.step·color.swap/reset. 메뉴·툴바는 `data-action`·`data-tool` 속성 배선.

### dom.js (Node-safe)

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `el` | `el(tag, attrs, children)` | 생성 또는 스텁 반환 |
| `qs` / `qsa` | `qs(root, sel)` / `qsa(root, sel)` | 단일·복수 조회 |
| `on` | `on(target, type, fn, opts)` | 등록 후 해제함수 반환 |
| `setHidden` / `clearChildren` | `setHidden(node, hidden)` / `clearChildren(node)` | hidden 토글·자식 비움 |

### strings.js / icons.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `STRINGS` | frozen(메뉴·액션·도구·힌트·패널·상태·다이얼로그·토스트) | 한글 중앙 카탈로그 |
| `msg` | `msg(key, fallback)` | 키 조회 |
| `ICONS` | frozen 15종 | pen/eraser/fill/eyedropper/line/rect/rrect/ellipse/hand/eye/eyeOff/lock/unlock/swap/plus |
| `iconFor` | `iconFor(id)` | SVG 또는 한글 폴백(펜·지·통·스·선·사·둥·타·손) |
| `hasIcon` | `hasIcon(id)` | SVG 보유 여부 |

### panel_color.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `DEFAULT_PALETTE` | 32색 frozen | 기본 팔레트 |
| `mountColor` | `mountColor(root, deps)` | 슬롯·컬러픽커·팔레트·최근색(32칸 고정) 마운트, dispose 반환 |
| `panel_color_wheel.js` | `createColorWheel({container,onChange})` | 색상환+SV 캔버스, HSV 수학, 래스터 캐시. DOM 없으면 `null` |

주/보조 슬롯, Shift+클릭=보조색, 우클릭=보조색, 그리기 Undo 발생 시 주색을 최근색에 push.
최근색은 `DEFAULT_PALETTE.length`칸을 **항상** 렌더링하고 미사용 칸은 `.dt-swatch.is-empty`(흰/회색 모자이크, `disabled`)로 채운다. 현재 색과 같은 팔레트 칸에는 `data-active="true"`가 붙는다(타깃 마커 표시용).

### panel_brush.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `SIZE_PRESETS` | 16단계 frozen | 1–64 |
| `paintPreview` | `paintPreview(canvas, size, hex)` | 64×64 1:1 footprint 미리보기 |
| `mountBrush` | `mountBrush(root, deps)` | 숫자·슬라이더·프리셋·미리보기 동기화 |
| `presetColumns` / `presetRows` | `presetColumns(count)` | 프리셋 그리드 열/행 수. 4 이상 최소 약수, 없으면 4 |

프리셋 16개는 `dt-preset-grid` 클래스와 `--dt-preset-cols/rows`를 publish해서 **4행 4열**로 나온다. 셀은 `dt-preset`.

### panel_layers.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `paintThumb` | `paintThumb(canvas, layer)` | 32×32 최근접 축소 썸네일 |
| `mountLayers` | `mountLayers(root, deps)` | 불투명도·역순 리스트박스·썸네일·버튼·더블클릭 개명 |

표시는 top-first 역순, 썸네일은 250ms 스로틀 갱신. 위로=인덱스+1(뒤가 위). 병합 버튼은 최하층에서 비활성.

### panel_options.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `mountOptions` | `mountOptions(root, deps)` | 그리드·줌·도구별 섹션·도형 bbox 마운트 + 툴팁 연결 |

`data-for-tools` 섹션 표시 전환, line은 채움 라디오 비활성, rrect만 반경 활성, 보류 있을 때만 bbox 입력·확정·취소 활성. 줌 셀렉트는 `ZOOM_LEVELS` 옵션.

### tooltip.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `TIP_AUTO_HIDE_MS` | `2500` | 표시 후 자동 닫힘까지의 유휴 시간 |
| `createTooltips` | `createTooltips(root)` | `{dispose, close, closeWithin, isOpen}` |

`.dt-tip-trigger`(`?`, 설명은 `data-tip`)에 hover/focus 또는 클릭하면 `.dt-tooltip`이 `document.body`에 만들어져 표시된다. `mouseleave`·`focusout`·`Escape`·바깥 클릭·유휴 타임아웃으로 닫히고 트리거의 `aria-expanded`가 갱신된다. 동시 표시 1개. 옵션바가 `overflow:hidden`이라 body 부착이 필수다. `root`가 delegation 기준이라 섹션이 `hidden` 되면 `closeWithin(sec)`로 함께 닫는다.

### statusbar.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `mountStatus` | `mountStatus(root, deps)` | dispose(속성 `setCursor`·`refresh` 포함) 반환 |

표시: 메시지(정보 4s·경고 8s·에러 상주)·커서 px·셀(32px)·타일(64px) 정수 좌표·줌%·캔버스 크기·도구명·dirty `●`.

위치 표시는 `formatGridCoords(x,y)`가 **정수 픽셀을 한 번만 floor**한 뒤 32px 셀과 64px 타일 인덱스를 같은 정수에서 파생시킨다(`셀 3, 2 (32px) · 타일 1, 1 (64px)`). 소수점은 절대 나오지 않는다. 두 필드는 `formatPosition()` 한 번의 결과에서 동시에 채워지므로 서로 어긋날 수 없다. 캔버스 밖에서는 `formatPosition(null)`이 두 필드 모두에 `- , -`를 넣고, 그 외 영속 정보(줌·크기·도구·dirty)는 그대로 유지된다.

### dialogs.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `showNewDocumentDialog` | `showNewDocumentDialog()` | 타일 단위 입력+프리셋, 32px 배수 검증, 취소 null |
| `confirmDiscardChanges` | `confirmDiscardChanges()` | 변경 버리기 확인 bool |
| `showRestoreDialog` | `showRestoreDialog(info)` | "restore"/"discard", DOM 없으면 "discard" |
| `showResizeCanvasDialog` | `showResizeCanvasDialog(current)` | 32–1920×32–1088·32배수 검증 |
| `showProgress` | `showProgress(text)` | `{update(pct), close()}`, aria-busy 관리 |

공통: 포커스 트랩·Esc 취소·오버레이 클릭 취소·이전 포커스 복원.

### shortcuts.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `resolveShortcut` | `resolveShortcut(desc)` | 순수 키→의도 매핑(테스트 가능) |
| `isEditableTarget` | `isEditableTarget(target)` | 입력 요소 가드 |
| `createShortcuts` | `createShortcuts(deps)` | window keydown 핸들러 `{handleKeyDown, dispose}` |

단축키: Ctrl+N/O/S/E, Ctrl+Shift+N(층 추가)·J(복제)·M(병합)·](위)·[(아래), Ctrl+'/그리드, Ctrl+Z/Y(+Shift+Z), B/E/G/I/L/R/U/O/H 도구, X 색교환, D 기본색, [/] 굵기(Shift 5단위), +/-/0/1 줌·맞춤·100%. Alt 조합은 무시, 입력 요소·다이얼로그 중에는 비활성, 도구 `keyDown`이 먼저 소비.

## 핵심 흐름

- 저장: 진행 표시→`documentToJson`→`saveTextFile`→`markSaved`→토스트.
- 열기: dirty면 확인→`pickFile`→`readJsonFile`→문서/레이어 분기→`loadDocument`.
- 설정 영속: 변경 후 250ms 디바운스 localStorage 기록(`dt.settings.v1`, 최근색 `dt.recentColors`).

## 주의점

- `index.html` id 계약: `#dt-app #dt-menubar #dt-optionbar #dt-toolbar #dt-left-panels #dt-right-panels #dt-canvas-host #dt-statusbar #dt-dialog-root #dt-toast` + 각 패널 내부 id. JS는 이 id에만 의존.
- style.css 토큰: `--bg --panel --panel-2 --border --text --text-dim --accent --accent-strong --danger --warn --canvas-bg --radius --gap`, 폰트·`--toolbar-w:44px --panel-w:240px --right-w:260px`. 썸네일·브러시 미리보기는 `pixelated`.
- **픽셀 폰트는 쓰지 않는다.** 가독성이 크게 떨어져 원본 `--font-ui`/`--font-mono`로 되돌렸다(Press Start 2P 인라인 삭제 완료).
- **모서리 둥글기는 토큰 2종**으로 나눈다. `--radius-box`(px, 큰 컨테이너)와 `--radius-pct`(% , 스와치·버튼·마커). %는 요소 박스 기준이라 크기가 달라도 비율이 유지되며 5%/10% 단위로 조정한다. 컨테이너에 %를 쓰면 반지름이 부풀어 오므로 쓰지 않는다.
- 컨테이너 박스(패널 섹션·옵션바 그룹·다이얼로그·토스트·진행창)는 `--radius-box` 얇은 곡선 + **`overflow: hidden`**. `overflow: hidden`은 필수다 — 없으면 테두리만 둥글고 내부 배경·각진 자식(팔레트 그리드·버튼 행)이 모서리를 덮어 "둥근 테두리 + 직각 내부"로 보인다.
- 옵션바는 1행(최소 40px)이다. 과거의 2행(80px) 예약은 항상 표시되던 `.dt-hint` 줄 때문에 있던 것으로, 힌트가 툴팁으로 바뀌면서 제거됐다.
- 설명 문구는 `.dt-hint` 상시 표시가 아니라 `.dt-tip-trigger`(`?` 버튼, `data-tip`에 텍스트)에서 툴팁으로 나온다. `tooltip.js`가 `document.body`에 `.dt-tooltip`을 붙인다(옵션바가 `overflow:hidden`이라 내부에 두면 잘린다). hover/focus 또는 클릭 시 표시, `mouseleave`·`Escape`·바깥 클릭으로 닫히고 `TIP_AUTO_HIDE_MS`(2500ms) 후 자동 닫힘. 섹션이 `hidden` 되면 `closeWithin`으로 함께 닫힌다.
- 선택 표시(타깃 마커): 활성 도구·활성 굵기 프리셋·활성 크기 프리셋·활성 색 슬롯·현재 색과 일치하는 팔레트 칸(`.dt-swatch[data-active="true"]`)·활성 레이어 행에 **둥근 직사각형의 네 귀퉁이만** 그린다. 하나의 `::before`에 전체 라운드 아웃라인을 그린 뒤 `clip-path` 폴리곤으로 각 변 중앙을 잘라내면 모서리 호만 남는다. 검정 테두리는 `box-shadow` spread로 빨간 선 **아래**에 깔린다(빨간 위에서도 보이게).
  - 팔 길이는 **고정**(`--dt-arm-l/d`, 11px)이어야 한다. 간격을 고정으로 두면 팔이 요소 크기에 따라 늘어나 넓은 행에서는 거의 이어져 그냥 빨간 테두리로 보인다.
- 컬러픽커 구조: `.dt-picker`(헤드 → `.dt-wheel-wrap` → `.dt-color-fields`) → 팔레트 → 최근 색. 색상환 176px 안에 SV 98px를 중앙 배치하며, SV 크기는 `panel_color_wheel.js`가 `RING_IN`과 클리어런스로 **계산**한다(`floor(2·(RING_IN−RING_CLEARANCE)/√2)`). 하드코딩하면 모서리가 잘려 일부 색이 선택 불가해진다. SV는 `border-radius: 0` 정사각형이고 모든 `(s,v)`(모서리 포함)가 선택 가능해야 한다.

레이아웃 소유권은 `style.css` 하나다. `panel_color_wheel.js`는 더 이상 인라인 레이아웃을 쓰지 않으므로 `WHEEL_PX`/`SV_PX`와 스타일시트의 176/98이 어긋나면 SV가 링 밖으로 나가거나 잘린다.

숫자 필드는 `.dt-color-fields`(4열 그리드)에 R/G/B가 1·2·3열을, `.dt-color-field--hex`가 4열의 2행을 `span`한다(= 요청의 2행 4열). 라벨은 `[data-ch]`로 구분해 R 빨강 / G 초록 / B 파랑.

**색 SSOT**: `canonicalHex()`가 유일한 읽기 경로, `applyColor()`가 유일한 쓰기 경로다. 네이티브 입력·HEX·R/G/B 각·휠·팔레트 클릭·최근색 클릭·우클릭이 전부 `applyColor`로 수렴하고, `paint()`가 `canonicalHex()`를 한 번 읽어 모든 컨트롤에 전파한다. 프로그램적 쓰기에 락을 걸어 `SETTINGS_CHANGED` 재진입을 무력화하므로 피드백 루프가 없고, 잘못된 입력은 필드를 정본값으로 되돌린다(포커스 중이어도).

**보조색에서 최근색이 죽던 원인**: `pickRecent`가 활성 슬롯을 무시하고 **항상 `primaryColor`에** 기록했다. 보조색이 활성인 상태에서는 모든 최근색 클릭이 주색을 바꿔 필드가 얼어붙었고, 게다가 `renderRecent`가 매번 `replaceChildren`로 32개 버튼을 재생성해 클릭 대상이 사라졌다. 이제 `pickSwatch`는 활성 슬롯을 보고(Shift는 반대 슬롯), 그리드는 한 번만 만들고 제자리에서 다시 칠하며, 색을 고르는 행위 자체를 최근색에 기록한다.
- 최근 색은 `DEFAULT_PALETTE.length`(32)칸 고정 렌더링이고, 미사용 칸은 `.dt-swatch.is-empty` 흰/회색 모자이크 + `disabled`. 저장은 `app.js`의 `RECENT_MAX = 32` 상한(값은 최근색 그리드 너비와 동기화해야 한다).
- 굵기 프리셋은 `#dt-size-presets.dt-preset-grid`가 4×4 그리드다(`--dt-preset-cols/rows`를 JS가 publish). 셀 `min-width: 0`이 필수 — 기존 `min-width: 32px`가 남으면 다시 6/5/5로 감긴다.
- 다이얼로그 열림 중에는 단축키·입력이 비활성(`aria-busy`·role=dialog 검사).
- 모듈 규약: 상대 경로 ESM import만, `export default`·동적 `import()`·`import.meta` 없음 (`contract.md` §5).
