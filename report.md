# Draw Tool v2 파일 분석 보고서

> **목적**: 파일 크기 및 SRP(단일 책임 원칙) 위반 분석을 통해 분리 필요 파일 식별  
> **기준일**: 2026-10-01  
> **분석 대상**: `draw_tool_v2/src/` 디렉토리 내 모든 `.js` 파일

---

## 1. 파일 크기 순위 (상위 15개)

| 순위 | 파일 경로 | 크기(KB) | 라인 수 | 판정 |
|:---:|:---|:---:|:---:|:---:|
| 1 | `src/ui/app.js` | 23.0 | 663 | ⚠️ **분리 필요** |
| 2 | `src/io/store_idb.js` | 14.1 | 417 | ⚠️ **분리 필요** |
| 3 | `src/tools/input_controller.js` | 13.5 | 454 | ⚠️ **분리 필요** |
| 4 | `src/tools/shape.js` | 13.0 | 345 | ⚠️ **분리 필요** |
| 5 | `src/model/session.js` | 11.6 | 377 | ⚠️ **분리 필요** |
| 6 | `src/ui/dialogs.js` | 11.0 | 305 | ⚠️ **분리 필요** |
| 7 | `src/ui/panel_options.js` | 11.0 | 288 | ⚠️ **분리 필요** |
| 8 | `src/io/validate.js` | 11.0 | 241 | ⚠️ **분리 필요** |
| 9 | `src/render/renderer.js` | 10.7 | 327 | ⚠️ **분리 필요** |
| 10 | `src/ui/panel_color.js` | 10.5 | 298 | ⚠️ **분리 필요** |
| 11 | `src/core/shape_raster.js` | 10.3 | 310 | ⚠️ **분리 필요** |
| 12 | `src/ui/panel_layers.js` | 9.4 | ~250 | ✅ 양호 |
| 13 | `src/ui/panel_color_wheel.js` | 9.2 | ~240 | ✅ 양호 |
| 14 | `src/model/commands.js` | 9.2 | ~240 | ✅ 양호 |
| 15 | `src/tools/pen.js` | 8.6 | ~220 | ✅ 양호 |

**기준**: 10KB 또는 300라인 초과 시 분리 검토 대상

---

## 2. SRP 위반 상세 분석

### 2.1 `src/ui/app.js` (23KB, 663라인) — **최우선 분리 대상**

#### 문제점
| 책임 영역 | 라인 범위 | 설명 |
|:---|:---:|:---|
| 뷰 스토어 | L52-121 | `createViewStore()` — 줌/오프셋 상태 관리, 구독 패턴 |
| 토스트 | L123-132 | `toast()` — UI 알림 표시 |
| 메뉴바 셸 | L136-209 | `createMenubar()` — 호버/클릭 열기, 지연 닫기 |
| 히스토리 버튼 | L212-261 | `createHistoryButtons()` — 실행 취소/다시 실행 활성화 상태 |
| 패널 새로고침 | L264-275 | `refreshPanelCanvases()` — 축소 섹션 재개 시 다시 그리기 |
| 부트 시퀀스 | L278-655 | `boot() — 전체 앱 조립 (1-9 단계) |
| 액션 핸들러 | L404-550 | 파일 저장/열기/내보내기/가져오기, 새 문서, 실행 취소/다시 실행, 레이어 작업, 보기 작업, 브러시/색상 작업 |

#### 분리 제안
```
src/ui/
├── app.js              # boot() 만 남기기 (~150라인)
├── view_store.js       # createViewStore() 이동
├── menubar.js          # createMenubar() 이동
├── history_buttons.js  # createHistoryButtons() 이동
├── panel_refresh.js    # refreshPanelCanvases() 이동
└── actions/
    ├── file_actions.js   # doSave, doOpen, doExportLayer, doImportLayer, doExportPng, doNew
    ├── edit_actions.js   # requestUndo, requestRedo
    ├── view_actions.js   # zoomIn, zoomOut, fit, actual, gridCycle
    ├── layer_actions.js  # add, duplicate, remove, mergeDown, up, down
    └── tool_actions.js   # brush.step, color.swap, color.reset
```

---

### 2.2 `src/io/store_idb.js` (14.1KB, 417라인)

#### 문제점
| 책임 영역 | 라인 범위 | 설명 |
|:---|:---:|:---|
| DB 스키마/트랜잭션 | L60-100 | `openDatabase()`, `promisify()` |
| 레코드 빌더 | L16-62 | `chunkRecordKey()`, `buildMetaRecord()`, `buildChunkRecords()` |
| 디바운스 스케줄러 | L207-229 | `_schedule()` — 800ms 디바운스 + 5s 강제 플러시 |
| 더티 추적 | L171-205 | `_markPixels()`, `_markMeta()`, `_markReplaced()` |
| 플러시 로직 | L240-348 | `flushNow()`, `_flushInner()`, `_writeTxn()` |
| 문서 복원 | L351-414 | `load()`, `_isMetaValid()`, `clear()` |

#### 분리 제안
```
src/io/
├── store_idb.js          # AutosaveStore 클래스만 남기기 (~150라인)
├── idb_schema.js         # openDatabase(), promisify() 이동
├── idb_record_builder.js # buildMetaRecord(), buildChunkRecords() 이동
├── idb_scheduler.js      # 디바운스/강제 플러시 스케줄러 이동
└── idb_loader.js         # load(), _isMetaValid(), clear() 이동
```

---

### 2.3 `src/tools/input_controller.js` (13.5KB, 454라인)

#### 문제점
| 책임 영역 | 라인 범위 | 설명 |
|:---|:---:|:---|
| 휠 누적기 | L6-37 | `WheelAccumulator` 클래스 |
| 도구 이벤트 빌더 | L39-83 | `buildToolEvent()` — 좌표 변환, 이벤트 병합 |
| 커서 상태 뷰신 | L142-192 | `_resolveToolCursor()`, `_applyCursor()`, `_markInside()`, `_syncInside()` |
| 입력 컨트롤러 | L85-454 | `InputController` 클래스 — 포인터/휠/키보드 이벤트 처리 |

#### 분리 제안
```
src/tools/
├── input_controller.js   # InputController 클래스만 남기기 (~250라인)
├── wheel_accumulator.js  # WheelAccumulator 클래스 이동
├── tool_event_builder.js # buildToolEvent() 이동
└── cursor_state.js       # 커서 상태 뷰신 함수들 이동
```

---

### 2.4 `src/tools/shape.js` (13KB, 345라인)

#### 문제점
| 책임 영역 | 라인 범위 | 설명 |
|:---|:---:|:---|
| 상태 뷰신 | L22-106 | `ShapeTool` 클래스 — idle/drawing/pending/resizing/moving 상태 관리 |
| 포인터 이벤트 | L137-214 | `pointerDown()`, `pointerMove()`, `pointerUp()` |
| 키보드 이벤트 | L217-246 | `keyDown()` — Enter/Escape/Delete/화살표 |
| 커밋/미리보기 | L249-344 | `getPending()`, `setPending()`, `commit()`, `paintPreview()`, `overlay()` |

#### 분리 제안
```
src/tools/
├── shape.js              # ShapeTool 클래스 코어 (~150라인)
├── shape_state.js        # 상태 뷰신 헬퍼 (전이 검증, 플래그)
├── shape_pointer.js      # 포인터 이벤트 핸들러들
├── shape_keyboard.js     # 키보드 이벤트 핸들러
└── shape_overlay.js      # overlay(), paintPreview() 이동 (이미 존재)
```

---

### 2.5 `src/model/session.js` (11.6KB, 377라인)

#### 문제점
| 책임 영역 | 라인 범위 | 설명 |
|:---|:---:|:---|
| 설정 검증 | L37-88 | `validateColor()`, `validateInt()`, `validateSetting()` |
| 세션 코어 | L90-132 | `Session` 클래스 기본 상태 |
| 문서 관리 | L163-185 | `newDocument()`, `loadDocument()` |
| 히스토리 관리 | L187-209 | `execute()`, `record()`, `undo()`, `redo()` |
| 레이어 작업 | L211-311 | `addLayer()`, `duplicateLayer()`, `removeLayer()`, `moveLayer()`, `mergeDown()`, `insertLayer()`, `resizeCanvas()`, `renameLayer()`, `setLayerVisible()`, `setLayerLocked()`, `setLayerOpacity()`, `setActiveLayer()` |
| 편집 세션 | L313-368 | `beginEdit()` — PixelWriter 생성, 편집 객체 관리 |

#### 분리 제안
```
src/model/
├── session.js            # Session 클래스 코어 (~150라인)
├── settings_validator.js # validateSetting() 등 이동
├── layer_operations.js   # 레이어 CRUD 작업들 이동
└── edit_session.js       # beginEdit() 및 편집 객체 관리 이동
```

---

### 2.6 `src/ui/dialogs.js` (11KB, 305라인)

#### 문제점
| 책임 영역 | 라인 범위 | 설명 |
|:---|:---:|:---|
| 모달 셸 | L13-99 | `openModal()`, `addButton()`, `numberField()` |
| 새 문서 다이얼로그 | L106-191 | `showNewDocumentDialog()` |
| 변경 버리기 확인 | L194-209 | `confirmDiscardChanges()` |
| 자동저장 복원 | L212-228 | `showRestoreDialog()` |
| 캔버스 크기 조정 | L231-262 | `showResizeCanvasDialog()` |
| 진행 표시 | L265-305 | `showProgress()` |

#### 분리 제안
```
src/ui/
├── dialogs.js            # 모달 셸만 남기기 (~100라인)
├── dialog_new_doc.js     # showNewDocumentDialog() 이동
├── dialog_confirm.js     # confirmDiscardChanges() 이동
├── dialog_restore.js     # showRestoreDialog() 이동
├── dialog_resize.js      # showResizeCanvasDialog() 이동
└── dialog_progress.js    # showProgress() 이동
```

---

### 2.7 `src/ui/panel_options.js` (11KB, 288라인)

#### 문제점
| 책임 영역 | 라인 범위 | 설명 |
|:---|:---:|:---|
| 축소 가능 섹션 | L28-93 | `loadCollapsed()`, `persistCollapsed()`, `applyCollapsed()`, `mountCollapsibleSections()` |
| 옵션 바 | L96-288 | `mountOptions()` — 그리드/줌/도구별 섹션/도형 bbox |

#### 분리 제안
```
src/ui/
├── panel_options.js      # mountOptions() 만 남기기 (~180라인)
└── collapsible_sections.js # mountCollapsibleSections() 및 헬퍼 이동
```

---

### 2.8 `src/io/validate.js` (11KB, 241라인)

#### 문제점
| 책임 영역 | 라인 범위 | 설명 |
|:---|:---:|:---|
| 에러 수집기 | L19-33 | `collector()` |
| 구조 검증 | L35-138 | `checkCanvas()`, `checkChunk()`, `checkRaster()`, `checkLayer()` |
| 의미 검증 | L141-168 | `checkChunksSemantic()` — PNG 디코딩 |
| 문서 검증 | L171-222 | `validateDocument()` |
| 레이어 파일 검증 | L225-241 | `validateLayerFile()` |

#### 분리 제안
```
src/io/
├── validate.js           # validateDocument(), validateLayerFile() 만 남기기 (~80라인)
├── validate_structural.js # checkCanvas, checkChunk, checkRaster, checkLayer 이동
└── validate_semantic.js  # checkChunksSemantic() 이동
```

---

### 2.9 `src/render/renderer.js` (10.7KB, 327라인)

#### 문제점
| 책임 영역 | 라인 범위 | 설명 |
|:---|:---:|:---|
| 렌더러 수명 주기 | L28-128 | 생성자, `attach()`, `dispose()`, `resize()` |
| 더티 추적 | L151-171 | `requestRender()`, `invalidateAll()`, `invalidateChunks()` |
| 합성 | L207-276 | `_ensureComp()`, `_update()`, `_paintDirty()` |
| 디스플레이 그리기 | L278-327 | `_drawDisplay()`, `_blitVisible()` |

#### 분리 제안
```
src/render/
├── renderer.js           # CanvasRenderer 클래스 코어 (~150라인)
├── renderer_composite.js # _ensureComp(), _update(), _paintDirty() 이동
└── renderer_display.js   # _drawDisplay(), _blitVisible() 이동
```

---

### 2.10 `src/ui/panel_color.js` (10.5KB, 298라인)

#### 문제점
| 책임 영역 | 라인 범위 | 설명 |
|:---|:---:|:---|
| 색상 적용/페인트 | L28-96 | `makeSafe()`, `canonicalHex()`, `applyColor()`, `paint()`, `setSlot()`, `pickSwatch()` |
| 최근 색상 그리드 | L98-146 | `buildRecentGrid()`, `paintRecent()`, `pushRecent()` |
| 팔레트 | L148-176 | `mountPalette()`, `syncActiveSwatches()` |
| 색상 휠 | L178-185 | `mountWheel()` |
| 슬롯 버튼 | L187-214 | `mountSlotButtons()` |
| 세션 이벤트 | L225-254 | `mountSessionEvents()` |
| 마운트 | L269-298 | `mountColor()` |

#### 분리 제안
```
src/ui/
├── panel_color.js        # mountColor() 코어 (~100라인)
├── color_recent.js       # 최근 색상 그리드 로직 이동
├── color_palette.js      # 팔레트 로직 이동
├── color_wheel.js        # 색상 휠 마운트 이동
├── color_slots.js        # 슬롯 버튼 로직 이동
└── color_events.js       # 세션 이벤트 구독 이동
```

---

### 2.11 `src/core/shape_raster.js` (10.3KB, 310라인)

#### 문제점
| 책임 영역 | 라인 범위 | 설명 |
|:---|:---:|:---|
| 브러시 풋프린트 | L36-50 | `brushFootprint()` |
| Bresenham | L52-66 | `forEachBresenham()` |
| 마스크 생성 | L68-143 | `ellipseMask()`, `rrectMask()`, `outlineRing()` |
| 라인 마스크 | L145-182 | `lineMask()` |
| 스냅핑 | L184-265 | `angleSnap()`, `dragBBox()`, `resizeBBox()`, `unitSnap()` |
| 적용 | L267-310 | `writeMask()`, `applyShape()` |

#### 분리 제안
```
src/core/
├── shape_raster.js       # applyShape() 만 남기기 (~50라인)
├── raster_brush.js       # brushFootprint(), forEachBresenham() 이동
├── raster_masks.js       # ellipseMask(), rrectMask(), outlineRing(), lineMask() 이동
└── raster_snap.js        # angleSnap(), dragBBox(), resizeBBox(), unitSnap() 이동
```

---

## 3. 분리 우선순위

| 우선순위 | 파일 | 이유 |
|:---:|:---|:---|
| **P0** | `src/ui/app.js` | 가장 크고(23KB), 책임이 가장 많음 (7개 영역) |
| **P0** | `src/io/store_idb.js` | 417라인, 6개 책임 영역, DB 계층 복잡도 높음 |
| **P1** | `src/tools/input_controller.js` | 454라인, 입력 처리와 변환 로직 혼재 |
| **P1** | `src/tools/shape.js` | 345라인, 상태 뷰신 + 이벤트 처리 혼재 |
| **P1** | `src/model/session.js` | 377라인, 설정 검증/레이어 작업/편집 세션 혼재 |
| **P2** | `src/ui/dialogs.js` | 305라인, 모달 셸 + 5개 다이얼로그 혼재 |
| **P2** | `src/ui/panel_options.js` | 288라인, 축소 섹션 + 옵션 바 혼재 |
| **P2** | `src/io/validate.js` | 241라인, 구조/의미 검증 혼재 |
| **P2** | `src/render/renderer.js` | 327라인, 수명 주기/합성/디스플레이 혼재 |
| **P3** | `src/ui/panel_color.js` | 298라인, 6개 하위 컴포넌트 혼재 |
| **P3** | `src/core/shape_raster.js` | 310라인, 마스크/스냅핑/적용 혼재 |

---

## 4. 양호 판정 파일 (10KB 미만)

| 파일 | 크기(KB) | 라인 수 | 비고 |
|:---|:---:|:---:|:---|
| `src/ui/panel_layers.js` | 9.4 | ~250 | 단일 책임 (레이어 패널) |
| `src/ui/panel_color_wheel.js` | 9.2 | ~240 | 단일 책임 (색상 휠) |
| `src/model/commands.js` | 9.2 | ~240 | 단일 책임 (커맨드 패턴) |
| `src/tools/pen.js` | 8.6 | ~220 | 단일 책임 (펜 도구) |
| `src/ui/panel_brush.js` | 6.9 | ~180 | 단일 책임 (브러시 패널) |
| `src/core/chunkstore.js` | 6.5 | ~170 | 단일 책임 (청크 저장소) |
| `src/tools/shape_geom.js` | 6.2 | ~160 | 단일 책임 (도형 기하학) |
| `src/ui/tooltip.js` | 6.2 | ~160 | 단일 책임 (툴팁) |
| `src/ui/shortcuts.js` | 6.1 | ~160 | 단일 책임 (단축키) |
| `src/render/grid.js` | 5.7 | ~150 | 단일 책임 (그리드) |
| `src/model/document.js` | 5.7 | ~150 | 단일 책임 (문서 모델) |
| `src/io/png.js` | 5.6 | ~150 | 단일 책임 (PNG 인코딩) |
| `src/ui/statusbar.js` | 5.2 | ~140 | 단일 책임 (상태 표시줄) |
| `src/io/serialize.js` | 8.5 | ~220 | 단일 책임 (직렬화) |

---

## 5. 결론

- **총 소스 파일**: 63개
- **분리 필요 파일**: 11개 (17.5%)
- **양호 파일**: 52개 (82.5%)
- **가장 시급한 분리 대상**: `src/ui/app.js` (23KB, 663라인, 7개 책임 영역)

### 권장 사항
1. **P0 파일부터 분리 시작** — `app.js`와 `store_idb.js`가 가장 큰 기술 부채
2. **테스트 커버리지 확인 후 분리** — 리팩토링 전 테스트가 충분한지 확인
3. **점진적 분리** — 한 번에 하나의 파일만 분리하여 회귀 위험 최소화
4. **M2 워크플로우 적용** — 3개 이상의 파일 변경이 필요하므로 Planner → Scout → Implementer 파이프라인 사용 권장
