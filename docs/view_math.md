# view 수학 — 왜 이렇게 구현했나

`crates/engine/src/view/`의 소스 주석은 "서명만 보면 알 수 없는 것"만 남긴다.
긴 설명은 여기로 옮겼다. 규범은 `src/features/viewport/{view,view_store}.js`이고,
`tools/view_check.mjs`가 그 동작을 핀한다.

## 1. 좌표 모델

`View = { zoom, offsetX, offsetY }`. `zoom`는 캔버스 1px당 CSS px,
`(offsetX, offsetY)`는 캔버스 (0,0)이 뷰포트 좌상단에서 어디로 가는지다.

| 변환 | 식 | 비고 |
|---|---|---|
| `screenToCanvas` | `(s - offset) / zoom` | 반올림 없음 |
| `canvasToScreen` | `offset + x * zoom` | **곱셈 먼저**(아래 §4) |
| `pixelAt` | `screenToCanvas` + `Math.floor` | 클램프 없음(아래 §3) |
| `visibleChunkRange` | 두 점의 floor × `CHUNK_PX` | 먼 가장자리 감산(아래 §5) |

캔버스 좌표는 `i64`를 쓴다. 캔버스 왼쪽·위의 포인터는 음수가 정당하고
(`view_check.mjs`가 `pixelAt(v, 99, 49) == {-1, -1}`를 핀한다), 오프캔버스 페인팅
테스트는 `-5000`까지 닿으므로 `u32`는 감싼다.

## 2. 상수 — 어디서 왔나

| 상수 | 값 | 출처 |
|---|---:|---|
| `ZOOM_LEVELS` | 12 rung | `docs/contract.md` §1. `constants.rs`에만 있고 여기서는 재수출만 |
| `CLAMP_MARGIN` | 32 | `view.js` `clampView(…, margin = 32)`. contract §1에 핀 |
| `FIT_PADDING` | 16 | `view.js` `fitView(…, padding = 16)`. contract §1에 핀 |
| `CHUNK_EDGE_EPS` | `1e-9` | `view.js` `visibleChunkRange`의 하드코딩. contract §1에 핀 |
| `Viewport::DEFAULT` | 800×600 | `view_store.js`의 `clientWidth \|\| 800` 폴백 |

네 번째까지 contract에 없었고 이번에 추가했다. **모듈에 숫자를 새로 만들지 않는다** —
JS 기본매개변수 값도 contract 표에 올린 뒤 상수로 참조한다.

## 3. `Math.round`은 `f64::round`이 아니다

가장 큰 트랩이다. JS는 하프를 **+∞ 쪽**으로 올린다(`Math.round(-2.5) === -2`).
Rust `f64::round`는 **0에서 멀리** 반올림한다(`(-2.5f64).round() === -3`).
둘은 **음수 값의 `.5` 경계에서만** 다르고, 그때는 정확히 캔버스가 뷰포트보다 클 때다.
`fit_view`·`actual_size_view`가 음수 오프셋으로 중앙정렬하는 유일한 경우이므로
`round_half_up = Math.floor(v + 0.5)`로 포팅했다.

`Math.floor` vs `as i64`도 같다. `pixel_at`과 청크 인덱스의 바닥은 `v.floor() as i64`.
`as i64`는 0 쪽으로 자르므로 `-0.5`가 `-1`이 아니라 `0`이 되고 캔버스가 왼쪽 가장자리
너머로 거울상이 된다. 두 변환은 헬퍼 하나로 격리했다.

## 4. 연산 순서가 비트 동일성을 결정한다

`canvas_to_screen`은 곱셈을 먼저 쓴다(`offset + x * zoom`). 순서를 바꾸면 느려지지
않지만 값이 흔들리고, `zoom_at`이 만드는 값에 대해 `screen_to_canvas`의 정확한
역함수가 아니게 된다.

`zoom_at`도 `k = newZoom / zoom`을 **한 번** 계산해 양축에 재사용한다. 인자를
달리 묶으면 마지막 비트가 달라지고, `view_check.mjs`의 "앵커 고정" 불변량이
1e-9 허용오차로 떨어지는 대신 정확히 통과하지 못한다.

`clamp_offset`은 `val.max(lo).min(hi)` 순서다. 범위가 역전되면(`hi < lo`,
`canvas * zoom < 2*margin - vw`이 되면 발생) `max`가 `lo`로 붙잡고 `min`이 다시
`hi`로 끌어내린다. 두 호출을 바꾸면 `lo`에 핀된다. 32px 캔버스를 0.25배로 50px
뷰포트에 넣은 경우(`lo = 24 > hi = 18` → 18)로 핀했다.

## 5. `visibleChunkRange`의 `- 1e-9`

이 함수에서 가장 중요한 한 줄이다. 캔버스 경계가 청크 경계와 **정확히** 만날 때
청크를 하나 과다 보고하는 것을 막는다. 캔버스 128·뷰포트 64·줌 1이면 먼 가장자리는
캔버스 x=64에 있고, `floor(64/32) == 2`는 청크 2를 요구한다. 청크 2는 캔버스 64..95를
덮으므로 화면 밖이다. 엡실론을 빼면 1로 내려가 실제로 보이는 마지막 청크가 된다.
이게 없으면 렌더러가 아무도 못 보는 청크를 합성하고 `paintDirty` 예산을 태운다.

가까운 가장자리는 0에서 클램프한다. 캔버스 왼쪽·위의 청크는 존재하지 않기 때문이다.
역전된 범위는 `None`이고, 이건 JS의 "완전히 화면 밖" 상태다.

## 6. 줌 사다리 — 양끝에서 "엉뚱한 곳에 달라붙는다"

`stepZoom`의 두 폴백은 정확히 반대 방향으로 잘못된 곳에 달라붙는다.

- `step_zoom(64, 1) === 32`: `.find(l => l > 64)`이 아무것도 못 찾아 JS가
  `ZOOM_LEVELS.at(-1)`로 폴백.
- `step_zoom(0.1, -1) === 0.25`: 같은 논리가 아래끝.

사다리 밖에 있는 뷰는 다음 스텝에서 제자리가 아니라 사다리 **쪽으로** 움직인다.
`dir == 0`은 어떤 룽에도 스냅되지 않는 순수 항등이다.

`fit_view`는 사다리 룽을 넘지 않는다. 캔버스가 0.25배보다 커도 0.25배에서 멈춘다 —
사다리는 고정 화이트리스트지 연속 스케일이 아니다.

## 7. `ViewStore` — 세 가지 차이

| JS | Rust | 이유 |
|---|---|---|
| `host.clientWidth` 매번 읽기 | `set_viewport(w, h)` | DOM 읽기 제거 |
| `session.doc` | `set_canvas(w, h)` | 문서 붙박이 분리. "문서 없음"은 그대로 클램프 없는 상태 |
| `subscribe` 집합 | `ViewChange::events()` → `&[]` | 아래 |

**이벤트 카탈로그를 여덟로 늘리지 않았다.** `view_store.js`의 두 구독자는
`renderer.requestRender()`와 상태바 다시 그리기였고, 둘 다 `events::EVENTS`의 일곱
신호가 아니었다(ui/app.js에 직접 배선). 그래서 카탈로그는 그대로 두고 `changed == true`
자체를 레드로 신호로 쓴다. `events()`가 메서드인 이유는 나중에 진짜 신호가 생겨도
호출부를 고치지 않아도 되게 하려는 것이다.

`zoomTo`는 `!finite || z <= 0`를 **무시**하지만 `zoom_at`은 같은 값에서 **에러**를
낸다. 순수 함수는 엄격하고 저장소는 관대하다. 일부러 그대로 뒀다 —
`ui/actions/action_map.js`가 사용자 입력을 여기로 흘려보내기 때문이다.
`Err` 팔에 도달하지 않음(직전에 검증함)에도 `match`를 유지하는 이유도 테스트 밖에
패닉 경로를 두지 않으려는 것이다.

`center()`는 **뷰포트** 중심이지 캔버스 중심이 아니다. 리사이즈가 보고되기 전에는
폴백 뷰포트를 읽으므로 앵커는 실제 창 크기와 무관하게 처음엔 400×300이다.

`set_viewport`는 창이 커지면서 화면 밖으로 나간 캔버스를 다시 끌어들일 수 있도록
나쁜 크기를 `Viewport::DEFAULT`로 대체한다(JS의 `try {} catch {}`를 글자 그대로).

## 8. 측정

성능 수치는 `docs/perf_js_vs_rust.md`. 이 모듈은 픽셀을 만지지 않으므로 그 표의
대상이 아니다.
