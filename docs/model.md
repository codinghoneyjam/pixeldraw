# model — 문서·레이어·히스토리·세션

## 역할 요약

- 문서 상태의 유일 소유자. 모든 변경은 Command 객체로만 수행(생성자·`loadDocument` 제외).
- Undo 예산(200스텝/96MiB)과 dirty(저장 여부) 추적.
- UI와 도구가 구독하는 `EventTarget` 세션 + 스트로크용 `Edit` 핸들 제공.

## 파일별 API

### layer.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `BLEND_MODES` | `["normal"]` 고정 | UI는 normal만 노출 |
| `LAYER_PROP_FIELDS` | `["name","visible","locked","opacity"]` | Command가 수정 가능한 필드 |
| `validateLayerName` | `validateLayerName(name)` | 1–128자·비공백, 위반 `INVALID_STATE` |
| `validateLayerOpacity` | `validateLayerOpacity(opacity)` | [0,1] 유한수, 위반 `OUT_OF_RANGE` |
| `validateLayerBlend` | `validateLayerBlend(blend)` | normal 외 `INVALID_STATE` |
| `Layer` | `constructor(id, name, visible, locked, opacity, blend, store)` | 값 검증 후 보관 |
| — | `static create({id, name, widthPx, heightPx})` | 표시·미잠금·불투명 1·normal 기본 |
| — | `cloneWith({id, name})` | store 깊은 복제 |
| — | `setProp(field, value)` | 필드별 검증 대입 |

### document.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `IdGen` | `constructor()` / `seed(ids)` / `next()` | `layer-NNNN` 카운터, 결정적 |
| `validateBackground` | `validateBackground(v)` | transparent 또는 hex |
| `Document` | `constructor({id, name, canvas, layers, activeLayerId, ids})` | 크기·층수·중복id·활성층 검증 |
| — | `getLayer(id)` / `findLayer(id)` / `indexOf(id)` / `get activeLayer` | 조회(get 실패 `LAYER_NOT_FOUND`) |
| — | `_insert(layer, index)` / `_remove(id)` / `_move(id, toIndex)` / `_setActive(id)` / `_setCanvasSize(w,h)` / `_setBackground(v)` / `_rename(name)` | Command 전용 변이(밑줄 메서드 직접 호출 금지) |
| `newDocument` | `newDocument({widthPx, heightPx, background, name, id})` | 1층 문서 생성, id 미지정 시 `doc-NNNN` |

### commands.js (모두 `do(doc)/undo(doc)/byteSize()/effects()` 보유)

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `PaintCommand` | `constructor(changeSet, label="연필")` | 청크 before/after 교체 |
| `AddLayerCommand` | `constructor(layer, index, {makeActive, label})` | 삽입+활성화 |
| `RemoveLayerCommand` | `constructor(layerId, label)` | 제거(마지막 1층 제거는 Document가 거부) |
| `MoveLayerCommand` | `constructor(layerId, from, to, label)` | 순서 이동, 0바이트 |
| `SetLayerPropCommand` | `constructor(layerId, field, before, after, label)` | opacity만 `mergeWith` 병합 |
| `MergeDownCommand` | `constructor(upperId, label)` | 윗층을 아랫층에 `over()` 병합 후 윗층 제거, 잠금/숨김층 거부 |
| `ResizeCanvasCommand` | `constructor(newW, newH, label)` | 잘린 청크 보관 후 복원 가능 |

`effects()`는 `{layers:{reason,layerIds}, pixels:[{layerId,chunks}|{layerId,all}]}` 형태.

### history.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `UndoManager` | `constructor({limitSteps, limitBytes})` | 기본 200 / 96MiB |
| — | `setDocument(doc)` / `commit(cmd,{applied})` / `undo()` / `redo()` | commit은 미적용 시 `do` 실행, opacity 병합 시도 |
| — | `canUndo()/canRedo()/undoLabel()/redoLabel()` | 상태 조회 |
| — | `breakMerge()` / `markSaved()` / `isDirty()` / `clear()` / `subscribe(fn)` | 병합 차단·저장점·dirty 판정 |

예산 초과 시 가장 오래된 항목부터 제거(단 1개는 유지). 저장점이 제거되면 `isDirty()`는 true 고착.

### session.js

| 이름 | 시그니처 | 설명 |
|---|---|---|
| `TOOL_IDS` | 9종 frozen | pen, eraser, eyedropper, fill, line, rect, rrect, ellipse, hand |
| `validateSetting` | `validateSetting(key, value)` | 키별 정규화·검증, 미지정 키 `INVALID_STATE` |
| `Session` | `extends EventTarget` | 설정+문서+히스토리+편집 상태 보유 |
| — | `setSetting(key, value)` | 변경 시 `settings-changed`, 동일값 false |
| — | `newDocument(opts)` / `loadDocument(doc,{markSaved})` | `document-replaced` 발행, 편집 초기화 |
| — | `execute(cmd)` / `record(cmd)` | 미적용 커밋 / 적용済み 커밋 + effect 분배 |
| — | `undo()` / `redo()` | 편집 중 false |
| — | `addLayer(name?)` / `duplicateLayer(id)` / `removeLayer(id)` / `moveLayer(id,toIndex)` / `mergeDown(id)` / `insertLayer(layer,index?)` | 레이어 편의 메서드 |
| — | `resizeCanvas(w,h)` | 동일 크기 false, 부정합 `CANVAS_SIZE_INVALID` |
| — | `renameLayer/setLayerVisible/setLayerLocked/setLayerOpacity/setActiveLayer` | 속성 변경(투명도 drag는 병합, `final:true`로 확정) |
| — | `beginEdit({layerId,label})` | 잠금/숨김층 거부(`LAYER_LOCKED/HIDDEN`), `Edit` 반환 |
| — | `notify(level,text,code)` / `emitToolState(tool,pending)` | 상태바·옵션바용 이벤트 |

`Edit` 핸들: `writer` + `flush()`(dirty만 `pixels-changed`) + `commit(label?)`(diff→`PaintCommand` 기록, 변경 없으면 false) + `cancel()`(복원).

## 핵심 흐름

- 스트로크 생명주기: `beginEdit` → writer 기록 + `flush`(실시간 렌더) → `commit` → `record(PaintCommand)` → `pixels-changed`+`history-changed`.
- 레이어 변경: `execute(Command)` → `_dispatchEffects` → `layers-changed`/`pixels-changed`.
- Undo: 편집 중에는 불가(`isEditing` 가드). `breakMerge`로 슬라이더 병합 종료.

## 주의점

- `Document._*` 밑줄 메서드를 Command 밖에서 호출 금지.
- `setActiveLayer`는 Command가 아니라 직접 대입 + `layers-changed(reason:active)`. Undo 대상 아님.
- 에러코드: `LAYER_LOCKED/HIDDEN/NOT_FOUND, DUP_LAYER_ID, ACTIVE_LAYER_MISSING, LAYER_LIMIT, CANVAS_SIZE_INVALID, OUT_OF_RANGE, INVALID_STATE`.
- `document_id`만 UI에서 `crypto.randomUUID()` 발급, 나머지는 카운터 결정성 유지.
