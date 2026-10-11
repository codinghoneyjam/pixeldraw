# 작업 로그 (최신이 위)

형식: `## YYYY-MM-DD · Phase N.M · 제목` 뒤에 bullets.

---

## 2026-10-11 · Phase 1.13b · 파사드 분할 10모듈 (io/png·model 6·raster 3)

**커밋 대상**: `crates/engine/src/{io/png, model/{commands,commands_pixel,document,history,session,settings}, raster/{arc,composite_ops,polygon}}.{rs,/*}`·`docs/{model_internals,raster_internals}.md`·`docs/progress/`
**테스트**: 305 그린 유지(lib 270 — 분할은 테스트 이동뿐 증감 없음)

- model 6 파일(410~668줄)·raster 3 파일·`io/png`를 `mod` 선언 + `pub use`
  파사드로 쪼갰다. 파일명 그대로 옆에 같은 이름 디렉터리를 붙이는
  Rust 2018+ 문법(`chunkstore.rs` + `chunkstore/` 형태). 공개 경로
  (`crate::model::history::…` 등)가 그대로여서 `crates/engine/tests/*`와
  다른 모듈은 손대지 않았다.
- 분할 지도: `io/png`→crc·decode·encode / `model/commands`→layer_cmd·paint·prop /
  `model/commands_pixel`→merge_down·resize / `model/document`→types·validate
  (**가드 순서는 `validate`가 소유** — 한 바퀴 검사를 쪼갰다가 코드가
  엉킨 트랩) / `model/history`→budget·undo_redo / `model/session`→core·ops
  (`SessionCore` 분리 이유는 model_internals §7) / `model/settings`→
  types·validate / `raster/arc`→angle·chord / `raster/composite_ops`→
  clip·common·erase·hud·sheen·visor / `raster/polygon`→fillet·masks·scanline
  (폴리곤은 예약 면제 대상이라 관심사별로만 분할).
- **최대 파일 355줄**(`model/layer.rs` — 기존, 분할 대상 아님). 분할된
  파일은 전부 300줄 이하.
- **긴 설명은 `docs/model_internals.md`·`docs/raster_internals.md`로 옮겼다.**
  소스 주석은 "서명만 보면 알 수 없는 것"(JS 패리티 트랩·비트 동일성을
  지탱하는 연산 순서·일부러 고치지 않은 이유)만 남긴다. 코드를 그대로
  되풀이하는 주석은 삭제했다.
- **살려둬야 하는 주석 한 줄**: `polygon/fillet.rs`의 `py_round` 짝수
  내림 분리를 갈라놓는 주석 — 지우면 clippy `if_same_then_else`가
  `-D warnings`를 낸다(raster_internals §2).
- 시각 보드 `docs/progress/board.html` 추가(오프라인 단일 파일).
- clippy·fmt 그린. 골든 201·png 5/5·JS 오라클 410은 코드 경로가 그대로여서
  재측정 없이 유효.

---

## 2026-10-11 · Phase 1.12 · model(document/layer/history/commands/session)

**커밋**: `8c17959`(view와 합쳐 커밋됨)·`src/features/{document,layers}` 967줄 → 15 모듈 5,088줄
**테스트**: 141 → **303 그린**(phase 기여 +106)

### 못 박아둔 JS 의미론(순서가 결과를 바꾸는 것들)

- **`Document` 생성자는 레이어별 검사를 한 바퀴에 한다.** 중복 id가 저장소 크기
  검사보다 **먼저** 올라간다(`document.js` L84-95). 처음에 두 바퀴로 쪼갰다가
  `[정상, 잘못된크기, 중복]`이 `INVALID_STATE`가 아닌 `DUP_LAYER_ID`로 나오는 걸
  테스트가 잡았다.
- **`_remove`의 active 재지정**은 스플라이스 **뒤**의 `min(index, len-1)`. 구멈으로
  밀려올라간 레이어가 후임이지, 아래 레이어가 아니다. 세 개의 내 테스트가 틀렸다.
- **`AddLayerCommand.undo`는 지운 레이어를 버린다**(`commands.js` L62-64). 따라서
  추가한 레이어에 친 줄은 **자기 PaintCommand**로만 살아남는다. Rust는 `apply`에서
  클론을 넣어 그 성질을 재현한다(주석 명시).
- **`_insert` 인덱스**: `undefined`면 append, `len` 초과는 클램프가 아니라
  `OUT_OF_RANGE`. 가드 순서 = 중복id → 64한도 → 저장소크기 → 인덱스.
- **Undo 정체성**: `_enforceBudget`의 가드는 `length > 1`이라 한 건의 과다 항목은
  항상 산다. `_savedTop`은 **객체 동일성**으로 비교하는데 `Box<dyn Command>`로는
  못 하므로, 스택 사이를 이동하는 `seq`로 대신 실었다.
- **`clear()`는 `_lastCmd`를 리셋하지 않는다**(`session.js` L45-47`이 `clear`를
  건너뜀). 그래서 `last_effects()`가 `clear()`를 살아남는다.
- **병합 정책**: `mergeWith`를 가진 명령은 `SetLayerPropCommand`뿐이고, 병합 대상은
  **opacity 필드 하나**, 같은 레이어+필수일 때만.
- **병합은 `top`에게 알리지 `cmd`에게 알리지 않는다**(`history.js` L61`), 그런데
  `Session::execute`가 `cmd.effects()`를 재dispatch한다. `SetLayerProp`에서는
  결과가 같으므로 기록만 했다.
- **`Session::undo()`는 문서도 스택도 비면 `false`**다 — `history.js` L73``가 `_doc`를
  만지기 **전에** 언두 스택 길이를 본다. 처음에는 `INVALID_STATE`를 냈고 세션
  테스트가 잡았다.
- **`moveLayer`**: `from === toIndex`이면 범위 검사 **전에** `false`를 돌려준다.
  같은 자리 이동은 no-op, `-1`은 `OUT_OF_RANGE`.
- **값이 안 바뀌어도 `setLayerOpacity(…, {final:true})`는 `breakMerge`를 부른다.**
  슬라이더 드래그 한 번 = 히스토리 한 항목과 필드 넉거듬 한 항목씩의 차이다.
- **`setActiveLayer`는 히스토리 항목이 없으면서 `layers-changed`를 self-dispatch한다.**
  명령 바깥에서 나오는 유일한 effect다.
- **`mergePixel`는 아래→위로 투명을 먼저 합성한다.** JS의 `packRGBA`→`unpackRGBA`
  왕복은 이미 클램프된 출력에서 항등이라 바이트를 그대로 옮긴다.
- **merge-down은 불투명도를 굽고 아래쪽 것을 1로 만든다.** 흰@0.5 위 검정 =
  `[128,128,128,255]`. `_checkFlags`는 숨김보다 **잠금을 먼저** 본다.
- **`_captured`는 첫 `do`의 크기를 붙잡는다** — do/undo/do가 자기 꼬리를 쫓지 않게.
- **`validateLayerName`은 JS `String.length`** = UTF-16 코드 단위. 이모지 65자 이름은
  128 단위에서 거부된다. `encode_utf16().count()`로 포팅.
- **`edit.commit()`은 `finish()` 뒤에 `takeDirty()`를 읽는다** — Rust의 `finish`이
  writer를 소모하므로 먼저 비운다. `finish`은 `_dirty`를 건드리지 않아 목록은 일치.

### 타입계가 흡수한 것(고치지 않고 기록)

- `setProp("visible", 1)`은 JS에서 조용히 `false`를 저장했다. `LayerPropValue`를
  필드별 태그로 만들어 불일치를 명시했고, JS의 모든 생산자는 이미 맞는 타입을
  넘겼으므로 그 경로는 도달 불가.
- `validateViewport`의 "객체여야 한다"/"정수여야 한다" 가드는 `Viewport`가 구조체가
  되면 표현 불가. `composite.rs`가 `assertDoc`/`assertOut`에 쓴 것과 같은 처리.
  크기·범위 검사와 코드는 그대로.
- `edit.commit()`의 `session._edit !== edit` 가드는 핸들을 consume하면 **컴파일
  에러**가 되는 구조.

### 차용검사기가 강제한 API 형태

- `EditHandle`은 수명 동안 `Session` 전체를 빌린다(`PixelWriter`가 레이어 저장소를
  빌리므로). 그래서 **리스너는 `begin_edit` 전에 등록해야 한다.** JS는 `writer.set`
  과 `commit` 사이에 `addEventListener`할 수 있었으나 그 창에서 dispatch하는 것이
  없어 포팅된 단정은 그대로다.
- `Session`의 `_lastCmd`는 **히스토리 구독**이 쓰는데, 세션은 자기 자신에게 못 한다.
  `UndoManager::last_effects()`로 옮기고 `notify`가 갱신한다(`clear` 제외 전 kind).
- **버려진 핸들은 세션을 영원히 `editing`으로 둔다.** `Drop`을 일부러 넣지 않았다 —
  JS의 `_edit` 참조가 갖던 새는 그대로이며 테스트로 못 박었다.
- `emitToolState`의 `pending`는 `String`이다. `shape.js` L150-153``가
  `JSON.stringify({pending, vertices})`를 비교하므로 **텍스트가 계약**이다. 실제
  타입은 Phase 2/3 몫.

### 기타

- **`settings_validator.js`는 DOM-free였다.** `errors`·`events`·`constants`·`pixel`
  만 import하므로 그대로 `model::settings`으로 포팅하고 deferred 시킬 것이 없었다.
- `e2e_session.test.mjs` 9개 H3 그룹 중 **4개만 지금 이식 가능**. H3.1/3.2는
  ToolManager(2.7), H3.3/3.5는 `documentToJson`(1.13), H3.7은 flatten(1.10 보조).
  이식한 것은 `beginEdit` + brush 커널로 같은 문서모델 사실을 단정하며, flatten은
  `composite_chunk`로, `hashDoc`은 sha256으로 대체.
- **내 버그를 골든이 잡았다**: `BLACK`을 `0x000000ff`로 썼다 — 이건 `r=255, a=0`
  (투명한 빨강)이다. 패킹은 `a<<24|b<<16|g<<8|r`이므로 검정은 `0xff000000`,
  초록은 `0xff00ff00`.

---

## 2026-10-11 · Phase 2.3 · view + 모듈 분할 규칙 정착

**커밋 대상**: `crates/engine/src/view.rs` + `view/{types,geom,zoom,clamp,fit,chunks,store,tests}.rs`·`docs/contract.md`·`docs/README.md`·`docs/view_math.md`·`docs/progress/README.md`
**테스트**: 303 그린 유지(view 56)

- **view를 파사드로 분할했다.** `view.rs`는 `mod` 선언과 `pub use`만(31줄),
  구현은 7개 하위 모듈. 최대 파일 330줄.
- **주석을 트랩만 남기고 다 없앴다.** 원본 746줄 중 주석이 289줄(39%)이었다.
  서명만 보면 모를 것만 남겼다: `Math.round` ≠ `f64::round`의 음수 하프 경계,
  `canvas_to_screen`의 곱셈 우선 순서가 비트 동일성을 지탱하는 사실,
  `clamp_offset`의 `max-then-min`이 역전 범위에서 `hi`에 핀되는 사실,
  `visibleChunkRange`의 `-1e-9`.
- **긴 설명은 `docs/view_math.md`로 보냈다.** 좌표 모델·상수 출처·반올림 규칙·
  사다리 양끝 달라붙음·`ViewStore`의 세 가지 차이와 이벤트 카탈로그를 여덟로
  늘리지 않은 이유. 소스에는 포인터 한 줄만.
- **contract에 상수 3개를 핀했다.** `VIEW_CLAMP_MARGIN` 32·`VIEW_FIT_PADDING` 16·
  `VIEW_CHUNK_EDGE_EPS` 1e-9. 셋 다 JS 기본매개변수/하드코딩이었고 contract에 없었다.
  모듈에 숫자를 새로 만들지 않는다는 규칙의 첫 사례.
- **내 테스트 착오 4건**: ①512 캔버스를 800 뷰포트에 클램프하면 원점은 이미
  유효 범위라 **no-op**(768으로 끌려가는다고 착각) ②fit 후 actual은 같은 뷰에
  도달하므로 `changed: false`가 정답 ③뷰포트를 키우면 재클램프가 변화를 만들지
  않을 수 있다 ④`cx0 >= 0`은 타입상 항상 참이라 비교가 무효 — `cx0 == 0`으로.

---

## 2026-10-11 · Phase 1.10~1.11 · composite·kernels·snap·png

**커밋 대상**: `crates/engine/src/{composite,kernels,io/}.rs`·`crates/engine/src/raster/snap.rs`·`docs/progress/`
**테스트**: 101 → **141 그린**(골든 201 + png 5 전부)

- **1.10 snap**(`raster/snap.rs`) — `angleSnap`·`dragBBox`·`resizeBBox`·`unitSnap`.
  골든 `angle_snap` 8 + `resize` 15 + `drag_bbox` 8 + `unit_snap` 6 = 37건 전건.
  - **좌표 도메인을 `i64`로 잡았다.** JS는 여기서 `Number`를 쓴다. 드래그로
    만들어진 박스의 원점은 캔버스 밖 수천 픽셀까지 나갈 수 있고(`oob_drawing`이
    `-5000`에 찍는다), `resizeBBox`의 종횡비 고정 분기는 두 변의 곱을 계산한다.
    `i32`로 가면 조용히 넘칠 자리가 넉넉히 있다.
  - **`Math.floor(x/2)`가 아니다.** JS `Math.floor((size-ns)/2)`는 양수 전용이지만
    이 경로는 음수가 된다. Rust `div_euclid(2)`로 옮겨야 JS와 같다.
  - 핸들 파싱은 JS의 `handle.includes("e")` 그대로. **그래서 핸들 문자열에 `e`가
    하나라도 들어 있으면 동쪽으로 해석된다** — 테스트에서 "move"를 무동작
    핸들로 썼다가 `e`가 걸려 실패했다. 원본의 날카로운 모서리를 그대로 뒀다.
- **1.10 composite**(`composite.rs`) — `compositeChunk`·`samplePixel`.
  - **레이어 배열은 아래→위 순서이다.** 패널이 `topFirst = layers.reverse()`로
    그리므로 배열 마지막이 맨 위. 처음에 반대로 짜서 테스트가 잡았다.
  - `assertDoc`·`assertOut`의 `INVALID_STATE` 두 가드는 타입으로 흡수했다.
    슬라이스와 길이 검사만 남기고, 길이 검사는 **같은 코드를 그대로** 낸다.
  - **NaN 방어를 명시했다.** JS `!(opacity > 0)`는 NaN을 가린다.
    `opacity <= 0.0`으로 쓰면 NaN이 통과하므로 `is_nan() || <= 0.0`으로 풀었다.
  - 알파 0이면 RGB까지 0으로 지운다. 그래야 청크가 전부 0이 되어 `isAllZero`로
   Prune되고 지운 자국이 파일에 남지 않는다.
- **1.10 kernels**(`kernels.rs`) — `floodFillScanline`(골든 `fill` 3건)과
  `strokeMask`(골든 `stroke` 7건). 나머지 두 커널은 1.4/1.9에서 이미 검증된
  `raster::brush`·`raster::shape_raster`에 있으므로 옮기지 않았다(중복 금지).
  - `strokeMask`는 JS 테스트가 쓰던 가짜 writer와 같은 형태의 `MaskSink`.
    음수 좌표는 버리고 캔버스 오른쪽으로 **감싸지 않는다**.
- **1.11 png** — CRC32·필터 0~4 디코드·필터 0 인코드. `png_cases` 5/5.
  - `CompressionStream`은 `flate2`(miniz_oxide). 압축 바이트는 다르고 픽셀은 같다.
  - 에러 코드 배열: 시그니처 실패 `PNG_SIGNATURE`, 잘림/CRC 불일치 `PNG_CRC`,
    비RGBA·비인터레이스·크기 `PNG_UNSUPPORTED`. 잘린 파일에서 어떤 코드가 나오는지
    8바이트 경계를 끊어가며 못 박았다(0~7바이트 → SIGNATURE, 그 이상 → CRC/UNSUPPORTED).
  - 디코드 루프는 `split_at_mut` 대신 **인덱스만** 쓴다. 위 행을 이미 복원한
    상태로 읽어야 하므로 차용 검사기가 튜�플로 `(위, 아래)`를 동시에 빌리면
    끝이 안 난다.
  - **필터 1~4는 픽스처가 태워주지 않는다.** `all_five_filters_6x5`는 다섯 필터를
    다 쓰지만 한 이미지뿐이라, 각 필터를 **직접 역인코드해 넣는** 별도 테스트를
    뒀다. 그게 없으면 paeth 분기가 실제로 한 번도 실행되지 않는 채로 통과한다.

---

## 2026-10-11 · Phase 1.9 · apply_shape + composite_ops

**커밋 대상**: `crates/engine/src/raster/{shape_raster,composite_ops,brush,mod}.rs`·`docs/progress/`
**테스트**: 86 → **101 그린**

- **`apply_shape`**(shape_raster.rs) — 모든 도형 경로의 공용 진입점. 펜·도형
  툴·HUD 브래킷·바이저 필이 전부 여기로 통과한다. 이미 검증된 마스크들을
  조합하므로 패리티는 그것들을 상속 + 쓰기 순서.
- **composite_ops 5종** — clipAlpha·erasePolygon·shapeFillMask·visorLayer·
  sheenOverlay·hudBrackets.
  - **`PixelSink`에 `get`을 추가했다.** 이 연산들은 이미 칠해진 타일을
    **읽어야** 하므로 mask-and-fill로 express 불가. 트레이트에 읽기가 들어간 이유.
  - **sheen의 3단계 순서가 load-bearing.** 레거시는 ①자체 오버레이에 그리고
    ②오버레이 **알파**에 바이저 마스크를 곱하고 ③그때 composite한다.
    composite 먼저·마스크 나중은 **다른 이미지**가 나온다. 단계를 합치지 않았다.
  - **bbox 관례 차이를 찾았다.** 툴 스펙은 `[x,y,w,h]`, `shapeFillMask`는 코너
    (`[[x0,y0],[x1,y1]]`)를 쓴다. JS의 `shapeArgs`가 그 변환을 한다. 내가
    처음에 bbox를 그대로 코너로 넘겨 바이저 스캔라인이 전혀 어두워지지 않았고
    테스트가 잡았다. `[x,y,w,h] → (x0,y0)~(x0+w-1, y0+h-1)`로 수정.
  - `Math.round`은 부호 무관 `floor(v+0.5)`.
- 남은: `stroke`(7)·`angle_snap`(8)·`resize`(15)·`drag_bbox`(8)·`unit_snap`(6)
  그룹은 스냅 기하 + 커널 경로로 1.10에서 함께 검증.

---

## 2026-10-11 · Phase 1.7~1.8 · arc/chard, gradient, atlas paste

**커밋 대상**: `crates/engine/src/raster/{arc,gradient,atlas,mod}.rs`·`docs/progress/`
**테스트**: 67 → **86 그린**

- **1.7 arc/chord** — 골든 4건 전부 허용오차 안에 통과.
  - **near-parity 허용오차를 명시적 상수로 핀**(`raster::arc::near_parity`).
    `arc` missing 0/extra ≤16, `chord fill` missing 0/extra ≤4,
    `chord outline` missing ≤16/extra ≤40. **PIL에 더 가깝게 만들면 게이트가
    실패한다**는 점이 핵심 — 포팅은 JS 재현이지 오라클 정합이 아니다.
  - **`Math.round`은 `floor(v+0.5)`.** 부호와 무관하다. 내가 불필요하게
    분기했다가 `Math.round(-2.5)`가 -3이 나와 테스트가 잡았다. (`f64::round`은
    half-away-from-zero라 **쓰면 안 된다**.)
  - 각도는 PIL 규약(0°=3시, 시계방향 증가). `end < start`면 360을 더해 정규화.
- **1.8 gradient/atlas** — 골든 7건(gradient 2·paste 5) 전부 통과.
  - gradient는 **반올림이 아니라 절단**(`int()`). 레거시 쇼크웨이브 베이커의
    T-5 알파 패리티 게이트가 기준. half-up과는 절반의 픽셀에서 다르다.
  - atlas의 paste 법칙: `sheet.paste(part, pos, part)` — **타일이 자기 자신을
    마스크로** 쓰이므로 알파가 제곱된다(128 → 64). 평범한 source-over로는
    재현 불가. 256 알파 × 2000 RGBA로measured된 닫힌 형식을 그대로 쓴다.
  - **픽스처 hex는 RGBA 바이트 순서**다("0A141EFF" → r=0A). big-endian u32로
    파싱하면 채널이 뒤집힌다. 내가 그렇게 했다가 golden이 잡아줬다.

---

## 2026-10-11 · Phase 0 + 1.1~1.6 · 엔진 기초·블렌드·청크저장·래스터 4종

**커밋 대상**: `Cargo.toml`·`Cargo.lock`·`crates/`·`docs/`·`tools/`(신규 3개)
**테스트**: 0 → **67 그린**

### Phase 0 — 기반 정비

- **0.1 베이스라인 측정**(`tools/bench_baseline.mjs`, 14항목).
  2048×1088·64레이어 기준 median:
  - stroke pen=1 2.69ms / pen=8 44.9ms / **pen=64 2785.7ms**(세그먼트당 ~139ms)
  - floodFill 146.2ms · applyShape ellipse fill 20.4ms · polygon fill 26.8ms
  - **composite 2176청크×64레이어 391.2ms** · **flattenToRgba 2002.3ms**
  - exportPngBytes 2070.7ms · documentToJson 1909.7ms(JSON 7.5MB)
  - 찾은 함정: floodFill은 입주를 변형시키므로 2회차부터 무효 샘플이 된다.
    `timeFresh()`(샘플마다 새 컨텍스트)로 고쳐 재측정.
- **0.2 워크스페이스**: 루트 `Cargo.toml` + `crates/{engine,platform,app}`.
  `unsafe_code = "forbid"`, `panic = "abort"`(release). `src-tauri`는
  `exclude`(Phase 6까지 base64 테스트가 오라클로 가치 있음).
- **0.3 오라클 동결**(`tools/oracle_manifest.mjs`): src/tests/reference 193파일
  sha256 매니페스트. 태그가 아닌 파일 기반 — 미커밋 트리에서도 동작하고,
  어떤 파일이 움직였는지 정확히 말해준다. digest `de5de4f5…`.
- **0.4 픽스처 하네스**: 처음 `tests/common/`에 뒀다가 `src/fixtures.rs`로
  이동. 단위 테스트도 같은 리더를 써야 해서. 부수적으로 `serde_json`이 정규
  의존성이 됐는데 1.13에서 원래 필요하므로 순서가 앞당겨진 셈.
- **0.5 보류**: `src/`를 `reference-app/`으로 옮기면 tests의 상대 import
  수백 개가 한 번에 깨진다. 오라클은 제자리에서 도는 편이价值. 삭제는
  Phase 6.3로 이관.

### Phase 1.1~1.3 — 기초·블렌드·청크저장

- 상수·에러·이벤트·픽셀 1:1 포팅.
- **발견: 에러 카탈로그가 문서(19종)와 코드(21종)가 다르다.**
  `VIEWPORT_INVALID`·`VIEWPORT_OUT_OF_RANGE`가 코드에만 있다. 코드가 진실이므로
  21종으로 포팅하고 `CATALOG` 상수로 핀(중복 검사 테스트 포함).
- **발견: `GRID_MODES` 4종이 `docs/core.md` 표에 없다.** 함께 포팅.
- `is_all_zero`는 `unsafe` 없이 바이트 스캔. 워크스페이스가 unsafe를 금지하고,
  LLVM이 같은 워드 단위 루프로 벡터화한다.
- `forEachChunk` 순서는 **키 오름차준(`cy*64+cx`)** 이지 행 우선이 아니다.
  처음 내 기대값을 행 우선으로 적어 테스트가 실패했고, 그게 원본 규칙임을 확인.
- 내 테스트 기대값 오류 2건(after 값 착각·순서 착각) — 전부 내 실수.

### Phase 1.4~1.6 — 래스터 4종

- 골든 그룹 실제 데이터 통과: brush 14+hashes 6, ellipse 15, rrect 8,
  outline 10, bresenham 7, **wide_line 54**, polygon 6, polygon_outline 6,
  fillet 5.
- **`Math.fround`은 `f32` 캐스팅.** 폴리곤 `dx`·교점은 C float. 두 f32의 곱은
  f64에 정확하므로 "f32 곱"과 "정확한 곱의 fround"가 비트 동일 — 이 성질로
  `inter_x(y, e)` 헬퍼 하나가 JS의 3중 fround 식을 그대로 재현.
- **반올림 4종을 혼동 금기**: `round_up`/`round_down`(span 경계) ·
  `round_c`(C roundf) · `py_round`(Python half-to-even). 대비 테스트로 못 박음.
- **`usize` 부호있는 뺄셈**: `ellipseMask`의 `2x+1-w`는 왼쪽 반에서 음수.
  JS는 float라 통과, Rust는 디버그 패닉 → 전부 i64.
- **`PixelSink` 트레이트**: `PixelWriter` 구현은 음수 좌표를 버린다. `u32`
  캐스팅을 그대로 쓰면 (−5,−5)가 (2043,1083)에 찍히는 사고. 테스트로 못 박음.
- **`lineMask`는 최소 bbox** 반환. 골든 `wide_line`은 64×64 **전체 캔버스**
  기준이라 테스트에서 W×H에 블릿 후 비교.
- **픽스처 키 불일치**: `pts`(polygon 계열) vs `points`, `w/h` vs
  `W/H`(wide_line·stroke). 원본 불일치이며 규칙을 새로 만들지 않았다 —
  `points_from_json`은 두 키를 모두 수용.
- **큰 마스크는 `hashes` 그룹으로 검증**: brush 64/33, ellipse 128×64,
  rrect 200×120은 행 인라인이 불가. `sha2` dev-의존으로 sha256 재현.
  (직접 구현했다가 100줄 낭비임을 인정하고 크레이트로 교체.)

### 남은 과제 메모

- `stroke` 그룹(7건)은 `brushFootprint`+`strokeMask`(여러 점 경로) 조합이라
  1.10 kernels에서 함께 검증한다.
- `angle_snap`(8)·`resize`(15)·`drag_bbox`(8)·`unit_snap`(6)은 **스냅 기하**로
  1.7 이후 별도 모듈(`raster/snap.rs`)에서 검증. `core_basic`의 view/스냅
  테스트가 대응.
- `gradient`(2)·`paste`(5)는 1.8. `composite`(8)·`fill`(3)은 1.10.
- `arc`(2)·`chord`(2)는 **near-parity 허용오차**를 JS와 똑같이 재현해야
  한다(missing 0 / extras ≤16 등). 첫 구현은 1.7.
