# model — 문서·레이어·히스토리·세션 (Command-only + EventTarget)

## 1. 개요·원칙·통합구조

- 역할: 문서 상태의 유일 소유자. 픽셀·레이어·캔버스 모든 변이는 Command 객체 경유.
- Command-only 원칙: `Document._*` 밑줄 변이 메서드는 Command 내부에서만 호출.
- 예외 2종: (1) 생성자·`Layer.create`·`newDocument` 초기 생성, (2) `loadDocument(doc)`
- 활성층 전환(`setActiveLayer`)은 Command가 아님. 직접 대입 + `layers-changed(reason:active)` 발행, Undo 제외.
- Session 통합구조: `Session extends EventTarget`이 설정+문서+히스토리+편집 상태를 한곳에 보유.
- Session 필드: `_settings`(복제+freeze 노출), `_doc`, `_history(UndoManager)`, `_edit(활성 Edit|null)`, `_lastCmd`.
- 이벤트 분배: `execute/record/undo/redo` → `commit` → `_dispatchEffects(cmd)` → `effects()` 분해 발행.
- 발행 이벤트: `layers-changed` / `pixels-changed` / `history-changed` / `document-replaced` / `settings-changed` + `notify`→`status-message`, `emitToolState`→`tool-state`.
- `history-changed` detail: `{canUndo,canRedo,undoLabel,redoLabel,dirty}` (`_historyState()`).
- `_lastCmd` 추적: `history.subscribe(kind!=="clear")` 콜백이 갱신, undo/redo 후 effect 재분배에 사용.

## 2. 파일별 상세

### 2.1 commands.js — 7종 (공통 `do(doc)/undo(doc)/byteSize()/effects()`)

- `PaintCommand(changeSet,label="연필")`: `type="paint"`. 청크 before/after 교체. `byteSize=changeSetBytes`. `effects={layers:null,pixels:[{layerId,chunks:[{cx,cy}]}]}`.
- `AddLayerCommand(layer,index,{makeActive=true,label="레이어 추가"})`: `type="add-layer"`. do=삽입+활성화(이전 활성 저장), undo=제거+활성 복원. `byteSize=chunkCount*CHUNK_LEN(4096)`. effects reason `add` + 픽셀 있으면 `{layerId,all:true}`.
- `RemoveLayerCommand(layerId,label)`: `type="remove-layer"`. do=`_remove` 보관(`_removed={layer,index}`+이전활성), undo=원위치 삽입+활성 복원. 미실행 시 `byteSize=0`. effects reason `remove`.
- `MoveLayerCommand(layerId,from,to,label)`: `type="move-layer"`. do=`_move(to)`, undo=`_move(from)`. `byteSize=0`. effects reason `move`, pixels `[]`.
- `SetLayerPropCommand(layerId,field,before,after,label)`: `type="set-layer-prop"`. 생성자에서 `LAYER_PROP_FIELDS` 외 `INVALID_STATE`. do/undo=`setProp`. `byteSize=0`. effects reason `prop`. `mergeWith(prev)`는 opacity·동일층·동일필드일 때만 `prev.after=after` 병합 후 true.
- `MergeDownCommand(upperId,label)`: `type="merge-down"`. `_resolve`=윗층 인덱스 조회(없으면 `LAYER_NOT_FOUND`, 0번이면 `INVALID_STATE`), `_checkFlags`=잠금이면 `LAYER_LOCKED`, 숨김이면 `LAYER_HIDDEN`. do=아랫층 전체 스냅샷(`_lowerBefore`)+픽셀별 `over()` 합성+`lower.opacity=1`+윗층 `_remove`+활성=아랫층. undo=아랫층 전역 클리어 후 스냅샷 복원+불투명도 복원+윗층 원위치 삽입. `byteSize=lowerBefore+upper` 청크바이트. effects=`{layers:{reason:"remove"},pixels:[{lowerId,all:true}]}`.
- `ResizeCanvasCommand(newW,newH,label)`: 생성자에서 `isValidCanvasSize` 위반 `CANVAS_SIZE_INVALID`. `type="resize-canvas"`. do=첫실행에旧크기 캡처+전층 `store.resizeTo` 잘림 수집(`_cut`)+`_setCanvasSize`. undo=旧크기 복원+리사이즈+잘림 재삽입. `byteSize=_cut` 합. effects=`{layers:null,pixels:전층 all:true}`.

### 2.2 document.js — Document / IdGen

- `IdGen`: `_counter` + `seed(ids)`(정규식 `^layer-(\d+)$` 최대값 채택) + `next()`=`layer-NNNN` 4자리 패딩. 결정적 카운터.
- `validateBackground(v)`: `"transparent"` 또는 hex(`parseHex!=null`)만 허용, 위반 `INVALID_STATE`.
- `Document({id,name,canvas,layers,activeLayerId,ids})`: 캔버스 유효성→배경 검증→층수 1~64(`LAYER_LIMIT`)→`Layer` 인스턴스·`DUP_LAYER_ID`·store=canvas 크기 일치(`INVALID_STATE`)→활성층 존재(`ACTIVE_LAYER_MISSING`) 순 검증.
- 조회: `getLayer(id)`(실패 `LAYER_NOT_FOUND`), `findLayer(id)`(null), `indexOf(id)`(-1), `get activeLayer`.
- Command 전용 변이: `_insert(layer,index)`(중복id·64제한·store불일치·인덱스 범위 `OUT_OF_RANGE`), `_remove(id)`(마지막 1층 거부 `INVALID_STATE`, 활성층이면 이웃 승계, `{layer,index}` 반환), `_move(id,toIndex)`, `_setActive(id)`, `_setCanvasSize(w,h)`, `_setBackground(v)`, `_rename(name)`.
- `newDocument({widthPx=512,heightPx=512,background="transparent",name="Untitled",id})`: 크기·배경 검증, `doc-NNNN` 카운터 id, `Layer 1` 1층 문서 반환.

### 2.3 edit_session.js — beginEdit 생명주기

- `beginEdit(session,{layerId,label="연필"})`: 문서 요구+`isEditing`이면 `INVALID_STATE`. 잠금(`LAYER_LOCKED`)+숨김(`LAYER_HIDDEN`)은 `notify("warn")` 후 throw. `PixelWriter(store,layerId)` 생성 후 `session._edit` 등록.
- `Edit.writer`: 픽셀 기록기. `takeDirty/discard/finish` 경유.
- `flush()`: `writer.takeDirty()`를 `pixels-changed{layerId,chunks}` 발행(비어있으면 미발행). dirty 배열 반환. 커서 이동 중 실시간 렌더용.
- `commit(commitLabel?)`: 활성 Edit 아니면 `INVALID_STATE`. `finish()`로 changeSet 확정+잔여 dirty 발행+`_edit=null`. changeSet 있으면 `session.record(PaintCommand)` 후 true, 없으면 false(히스토리 미기록).
- `cancel()`: 활성 검증 후 `writer.discard()`(원복)+dirty 있으면 `pixels-changed` 발행+`_edit=null`. 히스토리 미기록.

### 2.4 history.js — UndoManager

- `constructor({limitSteps=200,limitBytes=96MiB})`: `_undo/_redo/_totalBytes/_savedTop/_savedLost/_mergeBroken/_subs`.
- `setDocument(doc)`: 스택·바이트·저장점 초기화, `_mergeBroken=true`(문서 교체 시 병합 단절).
- `commit(cmd,{applied=false})`: 미적용이면 `do` 실행. 병합 조건(`!_mergeBroken`+top존재+`mergeWith` 함수 병합 성공) 시 redo 클리어+바이트 재계산+`notify(commit,top)` 후 종료. 아니면 push+바이트 가산+redo 클리어+예산 강제+`_mergeBroken=(mergeWith 없음)`+notify.
- `undo()/redo()`: 실패 예외 시 스택 복구 후 rethrow. 성공 시 상대 스택 이동+`_mergeBroken=true`+notify, bool 반환. redo도 예산 강제.
- 예산: `length>1` 유지 조건으로 `limitSteps/Bytes` 초과 시 가장 오래된 것부터 제거(최소 1개 유지). 저장점 제거 시 `_savedLost=true`.
- dirty: `_savedLost`면 true 고착, 아니면 `top!==_savedTop`. `markSaved()`=현 top 저장, 빈 스택은 null 저장.
- `canUndo/canRedo/undoLabel/redoLabel/breakMerge/clear/subscribe(fn→unsubscribe 반환, 콜백 예외 console.error 격리)`.

### 2.5 layer.js — Layer·검증

- `BLEND_MODES=["normal"]` frozen, `LAYER_PROP_FIELDS=["name","visible","locked","opacity"]` frozen.
- `validateLayerName`: 1~128자·비공백 문자열, 위반 `INVALID_STATE`. `validateLayerOpacity`: [0,1] 유한수, 위반 `OUT_OF_RANGE`. `validateLayerBlend`: normal 외 `INVALID_STATE`.
- `Layer(id,name,visible,locked,opacity,blend,store)`: 이름·불투명도·블렌드 검증 후 보관.
- `static create({id,name,widthPx,heightPx})`: 크기 검증+이름 검증, 표시·미잠금·불투명 1·normal·새 `ChunkStore` 기본.
- `cloneWith({id,name})`: 이름 검증, visible/locked/opacity/blend 계승+`store.clone()` 깊은 복제.
- `setProp(field,value)`: name=검증대입, visible/locked=`value===true` 강제, opacity=검증대입, 미지정 필드 `INVALID_STATE`.

### 2.6 layer_operations.js — 12함수 (인자/반환/Undo 규칙)

- `addLayer(session,name?)→id`: `Layer N+1` 기본명, `AddLayerCommand(끝삽입,makeActive)`를 `execute`. Undo=추가 취소.
- `duplicateLayer(session,id)→id`: 원본 조회+`cloneWith("이름 copy")`+원본 다음 위치 삽입 라벨 "레이어 복제". Undo=복제 취소.
- `removeLayer(session,id)→void`: `RemoveLayerCommand` 실행. 마지막층은 Document가 거부. Undo=복원.
- `moveLayer(session,id,toIndex)→bool`: 동일 위치 false(무명령). 범위 외 `OUT_OF_RANGE`, 없음 `LAYER_NOT_FOUND`. `MoveLayerCommand` Undo=원위치.
- `mergeDown(session,id)→void`: `MergeDownCommand` 실행. 조건 위반 시 해당 코드 throw. Undo=분리 복원.
- `insertLayer(session,layer,index?)→id`: 기본 활성층 다음 삽입+활성화. Undo=삽입 취소.
- `resizeCanvas(session,w,h)→bool`: 동일 크기 false. 부정합 `CANVAS_SIZE_INVALID`. Undo=旧크기+잘림 복원.
- `renameLayer/setLayerVisible/setLayerLocked(session,id,v)→bool`: 동일값 false(무명령), `SetLayerPropCommand` 실행. 이름은 사전 검증. Undo=이전값.
- `setLayerOpacity(session,id,value,{final=false})→bool`: 사전 검증, 동일값이면 final일 때만 `breakMerge` 후 false. 실행 후 final이면 `breakMerge`(드래그 병합 확정). opacity만 Undo 병합 대상.
- `setActiveLayer(session,id)→bool`: 동일 활성 false. Command 미사용·직접 `_setActive`+`layers-changed(reason:active)` 발행. **Undo 제외**·히스토리 미기록.

### 2.7 session.js + settings_validator.js — Session 전체 메서드·설정 3요소

- 기본 설정 9키: `primaryColor/secondaryColor(#000000/#ffffff)` `penSize:1` `activeTool:"pen"` `gridMode:"off"` `snapUnit:false` `shapeFill:"outline"` `shapeRadius:8` `shapeLockAspect:false`.
- `TOOL_IDS` 9종 frozen: pen/eraser/eyedropper/fill/line/rect/rrect/ellipse/hand.
- `validateSetting(key,value)→정규값`: 미지정 키 `INVALID_STATE`. 색상=hex 정규화(`toHex`, 알파≠255 거부 `OUT_OF_RANGE`), penSize 1~64·shapeRadius 0~2048 정수, activeTool/gridMode/shapeFill 열거 검증, snapUnit/shapeLockAspect 불리언 검증.
- `setSetting(key,value)→bool`: 정규화 후 동일값 false, 변경 시 `settings-changed{key,value}`.
- `newDocument(opts)/loadDocument(doc,{markSaved=true})→doc`: 히스토리 재지정+저장점+편집 초기화+`document-replaced{document}`+history 발행. load는 `Document` 아니면 `INVALID_STATE`.
- `execute(cmd)`=미적용 커밋+effect 분배, `record(cmd)`=적용済み 커밋+분배. `undo()/redo()`=편집 중 false, 성공 시 `_lastCmd` effect 재분배.
- `beginEdit(opts)→Edit`, `notify(level,text,code)→status-message`, `emitToolState(tool,pending)→tool-state`.
- getters: `doc/history/settings(동결복제)/isEditing`, `_requireDoc`(없으면 `INVALID_STATE`).

## 3. 대표 플로우

- 스트로크 기록: `beginEdit({layerId})`→`writer` 픽셀 기록→`flush()`마다 `pixels-changed` 실시간 렌더→`commit(label)`→`finish()` changeSet→`record(PaintCommand)`→`pixels-changed(all아님,chunks)`+`history-changed(dirty:true)`.
- undo/redo+dirty: `markSaved` 저장점→편집→`isDirty()=true`→`undo()`→`_dispatchEffects(_lastCmd)`→렌더+`history-changed`→top==저장점이면 `isDirty()=false`. 저장점 제거(예산 초과) 시 dirty true 고착. redo 스택은 신규 commit 시 클리어.
- 레이어 추가/삭제: `addLayer("배경")`→`execute(AddLayerCommand)`→`layers-changed(reason:add)`(+픽셀 있으면 all:true)+history→`undo()`→제거+활성 복원. `removeLayer(id)`→`execute(RemoveLayerCommand)`→`reason:remove`→`undo()`→원 인덱스 삽입+활성 복원. 마지막 1층 삭제 시도는 `INVALID_STATE`로 명령 불발.

## 4. 불변조건·에러

- 불변: 층수 1~64, 층 id 유일, store 크기=canvas 크기, 활성층은 항상 존재, `_*` 직접 호출 금지, 편집 중 undo/redo 불가, redo는 신규 commit 시 소멸, 최소 1 undo 유지.
- 문서 id만 외부 `crypto.randomUUID()` 허용, 레이어 id는 `IdGen` 결정적 카운터(충돌 시 `DUP_LAYER_ID`).
- 에러코드: `LAYER_LOCKED/HIDDEN/NOT_FOUND`, `DUP_LAYER_ID`, `ACTIVE_LAYER_MISSING`, `LAYER_LIMIT`, `CANVAS_SIZE_INVALID`, `OUT_OF_RANGE`, `INVALID_STATE`.
- 검증 우선순위: 존재(`NOT_FOUND`)→권한/상태(LOCKED/HIDDEN)→범위(`OUT_OF_RANGE`/SIZE)→정합(`INVALID_STATE`/LIMIT).

## 5. 파일 구성과 잔여 리팩토링 대상

`src/model/`은 더 이상 없다. 2026-10-08 피처 기반 재편에서 모듈이 둘로 갈라졌다.

- `src/features/document/` — 문서 자체: `document.js`(220)·`history.js`(139)·`session.js`(157)·`settings_validator.js`(57)
- `src/features/layers/` — 레이어와 Command: `commands.js`(140)·`commands_pixel.js`(159)·`edit_session.js`(59)·`layer.js`(60)·`layer_operations.js`(110)·`layer_actions.js`(20)·`panel_layers.js`(264)

`commands.js`는 픽셀 Command를 `commands_pixel.js`로 분리한 뒤 배럴(재노출) 역할만
한다. `session.js`는 `layer_operations.js`의 12개 함수를 Session 메서드로 재노출하고
`edit_session.js`의 `beginEdit`를 감싸는 파사드 역할을 한다. 두 피처 모두 `core/`에만
의존하며 역참조는 없다.

| 파일 | 줄 | 비고 |
|:---|:---:|:---|
| `features/layers/panel_layers.js` | 264 | 레이어 패널 (DOM 허용) |
| `features/document/document.js` | 220 | 단일 책임 |
| `features/layers/commands_pixel.js` | 159 | `commands.js`에서 분리됨 |
| `features/document/session.js` | 157 | 분리 후 코어 + 파사드 |
| `features/layers/commands.js` | 140 | 7종 중 픽셀 외 Command 배럴 |
| `features/document/history.js` | 139 | UndoManager, 단일 책임 |
| `features/layers/layer_operations.js` | 110 | 분리됨 |
| `features/layers/layer.js` | 60 | 단일 책임 |
| `features/layers/edit_session.js` | 59 | 분리됨 |
| `features/document/settings_validator.js` | 57 | 분리됨 |
| `features/layers/layer_actions.js` | 20 | `ui/actions/`에서 이동 |

250줄 초과 파일은 `panel_layers.js` 264줄 하나뿐이다. `commands.js`는
`commands_pixel.js`로 분리되어 더 이상 후보가 아니다.


## Handoff

- **Wrote**: `draw_tool_v2/docs/model.md`
- **Result**: `src/model/` 제거를 반영해 두 피처(`features/document/`·`features/layers/`)로
  인벤토리 교정. `commands.js` 2차 분리는 이미 `commands_pixel.js`로 해소됨.
- **Next**: `panel_layers.js` 264줄이 유일한 250줄 초과 파일 — 목록 렌더와 썸네일
  스로틀 분리 검토.