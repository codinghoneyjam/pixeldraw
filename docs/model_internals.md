# model 내부 — 왜 이렇게 구현했나

`crates/engine/src/model/`의 소스 주석은 "서명만 보면 알 수 없는 것"만 남긴다.
긴 설명은 여기로 옮겼다. 규범은 `src/features/document/*.js`와
`src/features/layers/*.js`이고, `tests/model_*.test.mjs`가 동작을 핀한다.

각 절은 파사드 한 파일에 대응한다. 코드에는 한 줄 포인터만 남아 있다.

## 1. 모듈 지도

| 파일 | 하위 모듈 | 함의 |
|---|---|---|
| `history.rs` | `budget` · `undo_redo` · `tests` | 예산·dirty와 스택 이동을 갈랐다. 둘은 `UndoManager`의 필드를 공유하므로 필드는 `pub(super)` |
| `commands_pixel.rs` | `merge_down` · `resize` · `tests` | 픽셀을 고치는 두 명령. JS도 `commands.js`에서 같은 이유로 분리했다 |
| `document.rs` | `types` · `validate` · `tests` | **가드 순서가 분리 기준**. `validate`가 순서를 소유하고 `types`는 데이터만 |
| `commands.rs` | `paint` · `layer_cmd` · `prop` · `tests` | 명령 패밀리별. `chunk_list`는 `paint`만 쓴다 |
| `session.rs` | `core` · `tests` | `EditHandle`가 빌려야 하는 상태와 문서를 갈랐다(§7) |
| `settings.rs` | `types` · `validate` | 검증기와 어휘. DOM-free라 지연 이식 없음 |

## 2. 왜 파사드인가

`model/mod.rs`는 전체 모델의 파사드이고 이미 71줄이다. 규칙상 300줄 이상이면
검토·400줄은 하드리밿이므로, 410~668줄짜리 여섯 파일은 각자 하위 파사드로
쪼갰다. 파일명은 그대로 두고(`model/history.rs`) 같은 이름의 디렉터리를
옆에 붙이는 Rust 2018+ 문법을 쓴다 — `chunkstore.rs` + `chunkstore/`,
`view.rs` + `view/`와 같은 형태다.

공개 경로(`crate::model::history::…`, `crate::model::commands_pixel::…`)는
그대로이므로 `crates/engine/tests/*`와 이 파일들을 쓰는 다른 모듈은 손대지
않는다. 재수출은 원래 있던 모듈에 남겨 두었다.

## 3. history.js — UndoManager

### 3-1. `_savedTop`은 객체 동일성인데 `Box<dyn Command>`로는 못 한다

JS는 `evicted === this._savedTop`, `top !== this._savedTop`로 저장점을
비교한다. `Box<dyn Command>`에는 `PartialEq`가 없고, 원시 포인터 비교는
추적해야 할 제거(eviction)보다 오래 살아남는다. 그래서 **커밋 시 발급하는
단조 증가 `seq`를 정체성 대신 싣는다** — `seq`는 언두/리도 스택 사이를
이동하며 따라간다.

### 3-2. 예산은 스택을 비우지 않는다

`_enforceBudget`의 가드는 `length > 1`이다. 그래서 한 건이 한도를 아무리
넘어도 살아남는다. `while len > 1 && (len > limitSteps || totalBytes >
limitBytes)`에서 가장 오래된 것부터 제거한다. 저장점이 제거되면
`saved_top = None`, `saved_lost = true`로 못 박아 dirty를 영구 고착시킨다.

- `recalc_bytes`는 병합 성공 시에만 전역 합산을 다시 한다. 병합은 총량을
  바꾸지 않지만(`cmd.byte_size()`가 같은 레이어의 같은 청크를 가리키므로),
  JS가 그렇게 했고 비용이 작다.

### 3-3. 병합 정책은 네 항목 전부

`!mergeBroken && top 존재 && cmd.supports_merge() && cmd.mergeWith(top)`.
`SetLayerPropCommand`만 `mergeWith`를 가진다. **병합은 `top`에게 알리지 `cmd`에게
알리지 않는다** — `cmd.merge_with(top.cmd.as_mut())`. 그런데
`Session::execute`는 commit의 반환값인 `cmd.effects()`를 재dispatch한다.
`SetLayerProp`에서는 결과가 같으므로 기록만 했다.

병합 성공 시 반환하는 effects는 **`top`의 것**이다(`undo_stack.last()의
`cmd.effects()`), 방금 들어온 명령의 것이 아니다.

### 3-4. 실패한 `do`/`undo`는 명령을 도로 넣는다

JS는 catch → 복원 → rethrow였다. 실패한 명령 하나가 스택을 바이트 단위로
원상복구하게 하려면 이 순서가 필요하다. `commit`은 `do`를 push **전에**
돌리므로 예외가 나면 아무 것도 쌓이지 않는다.

### 3-5. `clear()`는 `_lastCmd`를 리셋하지 않는다

JS의 history subscription은 `kind === "clear"`를 건너뛰었다. 그래서
`last_effects()`가 `clear()`를 살아남고, `session.js` L45-47도 `clear`를
건너뛴다. 따라서 clear는 통지하되 `last_effects`는 손대지 않는다.

### 3-6. `notify`가 `self`를 필드로 쪼개는 이유

스택 top 엔트리의 빌림과 `self.listeners`의 빌림이 겹치지 않는다는 것을
컴파일러에게 증명하려면 `self`를 필드별로 destructuring해야 한다. 스택을
레퍼런스로 넘기면 두 빌림이 동시에 보인다.

- `Session`은 자기 자신에게 구독할 수 없다(자기 필드를 빌린 채 자신을
  빌릴 수 없다). 그래서 `_lastCmd`를 관리자의 `last_effects()`로 옮겼고,
  `notify`가 갱신한다(`clear` 제외).
- `EditHandle`은 수명 동안 `SessionCore` 전체를 빌리므로 **리스너는
  `begin_edit` 전에 등록해야 한다.** JS는 `writer.set`과 `commit` 사이에
  `addEventListener`할 수 있었으나 그 창에서 dispatch하는 것이 없어 그
  단정은 그대로다.

## 4. commands_pixel.js — MergeDown / ResizeCanvas

### 4-1. `mergePixel`는 아래→위로 투명을 먼저 합성한다

`lower over transparent`을 먼저 하고, 그 결과 위에 `upper`를 올린다. 이것이
최적화가 아니라 **그 순서가 `over(mid, upper)`를 만들기 때문**이다. 반대로
하면 아래쪽이 반투명한 어디서나 색이 달라진다.

- JS는 그다음 `packRGBA(r,g,b,a)` → `unpackRGBA(p)` 왕복을 했다. 둘 다
  `overBlend`가 이미 클램프한 값에 대해 항등이고 `a === 0`은 위에서
  건너뛰었으므로, 바이트를 그대로 복사한다.

### 4-2. 병합은 불투명도를 굽고 아래쪽 것을 1로 만든다

흰@0.5 위 검정 = `[128,128,128,255]`. 아래층 opacity는 픽셀에 구워지므로
1이 된다. `undo`는 아래층을 전역 클리어한 뒤 스냅샷을 되돌리고 opacity를
복원한다 — **클리어가 먼저**여야 잘린 청크가 남지 않는다.

### 4-3. 영구한 것은 데이터가 아니라 병합 자체

`do`가 upper의 blend·opacity를 lower 픽셀에 구워 버리므로 blend **관계**는
사라진다. 병합 뒤에 lower의 blend를 `multiply`로 바꾸면 병합된 영역도 다시
블렌드된다. 두 레이어 모양은 되돌릴 수 없고 undo만이 유일한 복원 수단이다.
JS 주석이 같은 말을 하고 `blend_modes.test.mjs`가 양쪽을 핀한다.

### 4-4. `_checkFlags`는 숨김보다 잠금을 먼저 본다

둘 다 걸린 레이어는 `LAYER_LOCKED`로 거부된다. 가드가 스냅샷 **전에** 올라
거부된 병합이 레이어를 바이트 단위로 원상복구하게 한다.

### 4-5. `_captured`는 첫 `do`의 크기를 붙잡는다

do/undo/do가 자기 꼬리를 쫓지 않는다. 리사이즈는 일반적으로 왕복이 아니다 —
두 크기 모두에 걸쳐 일부만 밖에 있던 청크는 재스캔이 아니라 스냅샷에서만
복원된다.

### 4-6. 왜 `ChunkStore` 두 개를 동시에 빌리는가

아래층/윗층 스토어를 읽고 `put_chunk`은 아래층을 써야 한다. 그래서 내부
청크 루프를 `merge_chunks`로 분리해 두 빌림이 끝난 뒤에 쓰기를 한다.

## 5. document.js — 가드 순서

### 5-1. 생성자는 레이어별 검사를 한 바퀴에 한다

중복 id가 저장소 크기 검사보다 **먼저** 올라간다(document.js L84-95).
처음에 두 바퀴로 쪼갰다가 `[정상, 잘못된크기, 중복]`이 `INVALID_STATE`가
아닌 `DUP_LAYER_ID`로 나오는 것을 테스트가 잡았다. 그래서 `validate.rs`가
순서를 소유하고 `types.rs`는 데이터만 갖는다.

전체 순서: 캔버스 크기 → 배경 → 뷰포트 → 층수(1~64, `LAYER_LIMIT`) →
층별(중복 id → 저장소 크기) → 활성 id.

### 5-2. `_insert` 인덱스

`undefined`면 append, `len` 초과는 클램프가 아니라 `OUT_OF_RANGE`. 가드
순서 = 중복id → 64한도 → 저장소크기 → 인덱스. 그래서 꽉 찬 문서에 중복을
넣어도 `DUP_LAYER_ID`가 나온다.

### 5-3. `_remove`의 active 재지정

스플라이스 **뒤**의 `min(index, len-1)`. 구멍으로 밀려올라간 레이어가
후임이지, 아래 레이어가 아니다. 세 개의 내 테스트가 틀렸다.

### 5-4. `active_index`가 되돌리는 0

문서 불변조건이 활성 id의 존재를 보장하므로 불가능한 분기는 panic 대신 0을
돌린다.

## 6. commands.js — 명령 패밀리

### 6-1. `AddLayerCommand.undo`는 지운 레이어를 버린다

commands.js L62-64. 따라서 추가한 레이어에 친 줄은 **자기 PaintCommand**로만
살아남는다. Rust는 `apply`에서 클론을 넣어 그 성질을 재현한다. 레이어를
복제본으로 넣는 것도 같은 이유 — JS가 옮긴 것은 명령이 들고 있던 바로 그
객체였다.

### 6-2. `MoveLayerCommand`가 `from`을 붙잡는 이유

`_move` 스플라이스가 `from`과 `to`를 비가환으로 만든다: 1→0은 0→1의 역이
아니다. undo 시점에는 이미 레이어가 움직였으므로 `from`을 캡처해 둔다.

### 6-3. 병합은 opacity 필드 하나

`mergeWith`를 가진 명령은 `SetLayerPropCommand`뿐이고, 병합 대상은 같은
레이어 + 같은 필드 + **opacity 필드 하나**. 이름/visible/locked은 몇 번
발화해도 자기 히스토리 항목으로 남는다.

### 6-4. `setProp("visible", 1)`의 미도달 경로

JS는 조용히 `false`를 저장했다. `LayerPropValue`를 필드별 태그로 만들어
불일치를 명시했고, JS의 모든 생산자는 이미 맞는 타입을 넘겼으므로 그 경로는
도달 불가하다.

## 7. session.js — Session / SessionCore

### 7-1. 왜 `SessionCore`로 갈랐는가

`PixelWriter`가 레이어 스토어를 빌리므로 `EditHandle`은 수명 동안 세션의
비문서 상태를 함께 빌려야 한다. 문서와 같은 구조체에 그 상태가 있으면
불가능하다. 그래서 `Session { doc, core }`, `SessionCore { history, editing,
bus, settings }`로 쪼갰다. 결과적으로 `EditHandle`은 `Session` 전체가 아니라
`SessionCore`만 빌린다.

### 7-2. 버려진 핸들은 세션을 영원히 `editing`으로 둔다

`Drop`을 일부러 넣지 않았다 — JS의 `_edit` 참조가 갖던 새는 그대로이며
테스트로 못 박았다.

### 7-3. `Session::undo()`는 문서도 스택도 비면 `false`

history.js L73이 `_doc`를 만지기 **전에** 언두 스택 길이를 본다. 처음에는
`INVALID_STATE`를 냈고 세션 테스트가 잡았다. 그래서 `step`은 스택 길이를
먼저 본다.

### 7-4. `edit.commit()`의 순서

`finish()` 뒤에 `takeDirty()`를 읽는다. Rust의 `finish`이 writer를 소모하므로
먼저 비운다. `finish`은 `_dirty`를 건드리지 않아 목록은 일치.

- `edit.commit()`의 `session._edit !== edit` 가드는 핸들을 consume하면
  **컴파일 에러**가 되는 구조다. 타입계가 흡수했다.

### 7-5. `moveLayer`의 `from === toIndex`

범위 검사 **전에** `false`를 돌려준다. 같은 자리 이동은 no-op, `-1`은
`OUT_OF_RANGE`.

### 7-6. 값이 안 바뀌어도 `final:true`는 `breakMerge`를 부른다

슬라이더 드래그 한 번 = 히스토리 한 항목과 필드 넉거듬 한 항목씩의
차이다. 레이어 작업은 그 외에는 전부 "같은 값이면 명령 없이 false"다.

### 7-7. `setActiveLayer`는 self-dispatch

히스토리 항목이 없으면서 `layers-changed(reason: active)`를 스스로 발행한다.
명령 바깥에서 나오는 유일한 effect다.

### 7-8. `emitToolState`의 `pending`는 `String`

shape.js L150-153이 `JSON.stringify({ pending, vertices })`를 비교하므로
**텍스트가 계약**이다. 실제 타입은 Phase 2/3 몫.

## 8. settings_validator.js

DOM-free였다. `errors`·`events`·`constants`·`pixel` 만 import하므로 그대로
`model::settings`으로 포팅하고 deferred 시킬 것이 없었다.

### 8-1. 에러 코드의 비대칭

미지정 키는 `INVALID_STATE`, 나머지 전부는 `OUT_OF_RANGE`. 옵션 바가 코드
이름으로 잡으므로 JS 트리의 비대칭을 그대로 보존한다.

### 8-2. `validateLayerName`은 JS `String.length`

UTF-16 코드 단위다. 이모지 65자 이름은 128 단위에서 거부된다.
`encode_utf16().count()`로 포팅.

### 8-3. 어디서 왔는지 모르는 숫자

`penSize` 1~64, `shapeRadius` 0~2048, 기본값 1/8은 JS 매개변수 기본값과
하드코딩이다. `PEN_MIN`/`PEN_MAX`는 `docs/contract.md` §1에 핀되어 있고,
`shapeRadius`의 상한과 기본 8은 핀되어 있지 않다 — 모듈에 숫자를 새로 만들지
않는다는 규칙에 따라 남겨 두었고 계약 표 행 추가는 하지 않았다.
`validateColor`의 alpha 0 거부도 하드코딩 트랩이다(완전 투명 물감은 조용히
지우므로).
