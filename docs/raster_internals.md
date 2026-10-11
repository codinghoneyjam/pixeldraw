# 래스터 내부 — 왜 이렇게 구현했나

`crates/engine/src/raster/`와 `crates/engine/src/io/png/`의 소스 주석은 "서명만 보면
알 수 없는 것"만 남긴다. 긴 설명은 여기로 옮겼다. 규범은 `src/core/raster/`와
`src/io/png.js`이고, `tools/parity_check.mjs`가 201 골든, `tools/png_cases_check.mjs`가
5/5를 핀한다.

모듈 분할은 파사드 패턴(`view.rs`와 같은 형태)이며 `docs/native_plan.md` §9의 줄 수
규칙(엔진 모듈 400줄 하드리밋·300줄 검토)을 따른다. **폴리곤 래스터는 예약 면제
대상**이므로 `polygon/`는 공격적으로 쪼개지 않고 관심사별로만 나눴다.

## 1. 분할 지도

| 파일 | 내용 |
|---|---|
| `raster/polygon.rs` | 파사드(`mod` + `pub use`만) |
| `raster/polygon/scanline.rs` | PIL `Draw.c` 스캔라인 엔진 — edge table·정렬·span fill |
| `raster/polygon/masks.rs` | 공개 마스크 빌더(채우기·외곽·inset ring·fixture 파서) |
| `raster/polygon/fillet.rs` | `fillet_polygon` — 코너 라운딩 기하 |
| `raster/polygon/tests.rs` | 골든 그룹 + 반올림 대비 핀 |
| `raster/composite_ops.rs` | 파사드 |
| `raster/composite_ops/common.rs` | 읽기 연산이 공유하는 픽셀 헬퍼 |
| `raster/composite_ops/{clip,erase,visor,sheen,hud}.rs` | 연산 하나씩 |
| `raster/arc.rs` | 파사드 |
| `raster/arc/angle.rs` | 샘플 타원·스트로크·각도 클립 + `near_parity` |
| `raster/arc/chord.rs` | `draw.chord` fill·outline |
| `io/png.rs` | 파사드 |
| `io/png/crc.rs` | 시그니처·big-endian 단어·CRC-32 표·chunk walker |
| `io/png/decode.rs` | 필터 0~4 언필터 |
| `io/png/encode.rs` | 필터 0 인코드 |

`raster/mod.rs`와 `io/mod.rs`의 `pub use` 블록은 그대로다. 모든 공개 항목은 이전과
같은 경로(`raster::polygon::…`·`io::png::…`)로 도달한다.

## 2. `Math.fround` — 왜 `inter_x` 하나로 충분한가

폴리곤 스캔라인이 이 저장소에서 가장 위험한 코드다. JS는 교점을 세 번 `Math.fround`로
감싼다:

```js
Math.fround(Math.fround(Math.fround(y - y0) * dx) + x0)
```

`dx`와 `x0`은 둘 다 C `float`(JS는 double이지만 값이 float 표현식에서 왔다).
**두 f32 피연산자의 곱은 f64에 정확히 표현된다.** 그래서

- "f32 곱셈"과 "정확한 곱의 fround"는 **비트 동일**하고,
- `(y - y0) as f32 * e.dx` 다음 `fround(inner as f64 + e.x0 as f64)` 한 줄이 JS의
  3중 fround 식을 그대로 재현한다.

바깥쪽 `+ x0`의 명시적 fround가 **쓸데없는 노이즈가 아니다** — 같은 이유로 살려둬야
한다. `poly_add_edge`의 `e.dx = fround((x1-x0)/(y1-y0))`도 경로가 같다. 이 줄들의
형태를 바꾸면(예: f32 덧셈을 한 줄로 합치기) 마지막 비트가 흔들리고 골든
`polygon` 6 + `polygon_outline` 6 + `wide_line` 54건이 동시에 흔들린다.

### 반올림 네 종짜 — 혼동 금기

| 함수 | 출처 | 성질 | 쓰이는 곳 |
|---|---|---|---|
| `round_up` | PIL `roundUp` | 하프를 **+∞ 쪽** | span 왼쪽 경계 |
| `round_down` | PIL `roundDown` | 하프를 **-∞ 쪽** | span 오른쪽 경계 |
| `round_c` | C `roundf` | 하프를 **0에서 멀리** | vertex 퇴화 보정 |
| `py_round` | Python `round` | 하프를 **짝수로** | `fillet_polygon` |
| `round_half_up` | JS `Math.round` | 하프를 **+∞ 쪽** | `arc`·`composite_ops` |

둘을 바꿔 끼우면 차이는 **정확히 `.5` 경계**에서만 나는데, PIL 출력은 거기에 픽셀을
자주 찍는다. `roundings_are_not_interchangeable`와
`round_half_up_matches_js_math_round`가 대비 핀이다. `f64::round`는
half-away-from-zero라 `round_c`와만 같고 나머지 넷과 다르다 — `py_round`에
`f64::round`을 쓰면 `round(2.5)`가 3이 되어 필렛 골든 5건이 깨진다.

`py_round`의 `r < 0.5` 분기와 `(f as i64) % 2 == 0` 분기는 **같은 값을 돌려준다.**
그런데 clippy `if_same_then_else`가 이 둘을 합치라고 `-D warnings`를 낸다. 그 두
분기를 갈라놓는 유일한 것이 "Python round가 짝수로 내린다"는 한 줄 주석이라는 사실을
확인했고, 그 주석은 **살려둬야 한다** — 주석을 지우면 게이트가 빨개진다.

## 3. near-parity 허용 오차 — 오라클이 아니라 JS가 정답

골든 픽스처는 PIL과 JS 트리의 **현재 동작 차이**를 허용오차로 핀한다. `arc` 그룹의
허용오차는 `tools/parity_check.mjs` 주석에서 측정해 `arc::near_parity` 상수로 박았다.

| 케이스 | 규칙 |
|---|---|
| `arc` | missing **0**, extra **≤ 16** |
| `chord` fill | missing **0**, extra **≤ 4** |
| `chord` outline | missing **≤ 16**, extra **≤ 40** |

**PIL에 더 가깁게 만들면 게이트가 실패한다.** Rust가 복제하는 대상은 PIL이 아니라 JS
구현이다. 각도 규약은 PIL(0° = 3시, 시계방향 증가)이고 `end < start`이면 360을 더해
정규화한다. `ImageDraw.arc`/`chord`는 **해석적 곡선이 아니라** ~0.5° 간격으로 샘플한
타원 둘레의 폴리라인이므로, 모듈이 얇은 것이 정상이다.

## 4. 와이드 라인 — 회전 사변형, n×n 스탬프가 아니다

`ImagingDrawWideLine`은 선분에 **수직으로 편이동**해서 회전된 직사각형을 만든 다음
폴리곤 스캔라인 엔진으로 채운다. 직관적인 대안 — Bresenham Walk 위에 n×n 정사각
스탬프를 찍고 수직 투영으로 캡을 다듬기 — 은 Pillow 12.1.0 기준 **9가지 형상 ×
너비 2..7에서 1건만 정확**하고, 회전 사변형 모델은 **54/54 정확**이다. `wide_line`
골든 그룹 54건이 이 선택을 핀한다.

`polygon_outline_mask`는 `d.line`(패스 중심 스트로크)을 모델링하지만
`polygon_inset_ring`는 `d.polygon` 외곽선을 모델링한다 — **PIL의 두 경로가 편이동
기준이 다르다.** 그래서 inset ring는 맨해튼 침식이 아니라 폐방 원반의 **유클리드
침식**이다. `diamond_gem` 육각형 width 3 실측: 유클리드 0 missing / 0 extra,
맨해튼 24 missing.

## 5. composite_ops — 왜 읽기가 필요한가

다섯 연산은 이미 칠해진 타일을 **읽어야** 하므로 mask-and-fill 프리미티브로 표현할 수
없다. `PixelSink` 트레이트에 `get`이 들어간 이유다.

- **bbox 관례 차이.** 툴 스펙은 `[x, y, w, h]`, `shapeFillMask`는 코너
  (`[x0,y0]~[x1,y1]`)를 쓴다. JS의 `shapeArgs`가 그 변환을 한다. 변환을 빼먹으면
  바이저 스캔라인이 전혀 어두워지지 않는다.
- **sheen의 3단계 순서가 load-bearing.** ①자체 오버레이에 긋고 ②오버레이 **알파**에
  바이저 마스크를 곱하고 ③그때 composite한다. composite 먼저·마스크 나중은 **다른
  이미지**가 나온다. ②의 곱은 `ImageChops.multiply` — 보간(lerp)이 아니다.
- `clip_alpha`는 클립 **밖**의 알파를 지운다. 레거시가 알파 채널 전체에 마스크를
  곱했기 때문으로, 둥근 직사각을 넘친 본문 아트가 불투명하게 남지 않는다.
- `CrtLines`/`Hud`의 기본값과 sheen의 `110`/`60`·`2`/`1`은 JS 기본매개변수에서 온
  것이다. 원본 불일치를 닦아내지 않았다.

## 6. PNG — 에러 코드 경계와 압축 바이트

`CompressionStream("deflate")`를 `flate2`(miniz_oxide)로 바꿨다. **압축 바이트는
다르고 픽셀은 같다.** 계획의 난제 4가 이를 인정하고 게이트를 *디코드 픽셀 동일 +
저장→재로드 라운드트립*로 둔다.

에러 코드 배열은 8바이트 경계를 끊어가며 못 박았다:

| 잘린 길이 | 코드 |
|---|---|
| 0~7바이트 | `PNG_SIGNATURE` |
| 그 이상(헤더 불량·walk 실패·CRC 불일치·감압 실패) | `PNG_CRC` 또는 `PNG_UNSUPPORTED` |

`PNG_CRC`는 JS 디코더에서 "잘림"과 "체크섬 불일치"에 **같이** 쓰이고 카탈로그에 따로
코드가 없으므로 그 과부하를 그대로 옮겼다. IHDR은 depth 8·colour type 6·interlace 0만
받는다.

**필터 1~4는 픽스처가 태워주지 않는다.** `all_five_filters_6x5`는 다섯 필터를 다
쓰지만 이미지가 하나뿐이라, 각 필터를 **직접 역인코드해 넣는** 별도 테스트
(`each_row_filter_reverses_exactly`)를 뒀다. 그게 없으면 paeth 분기가 실제로 한 번도
실행되지 않는 채로 통과한다. 디코드 루프는 위 행을 이미 복원한 상태로 읽어야 하므로
`split_at_mut` 대신 **인덱스만** 쓴다.

CRC-32 표는 `crc32fast`(이미 `flate2`가 끌고온다)를 쓰지 않고 8줄로 직접 만든다.
워크스페이스가 `unsafe`를 금지하므로 `static mut` 트릭 대신 `OnceLock`을 쓴다. 256
항목 표를 호출마다 다시 만들면 청크 스트림에서 일의 대부분을 태우게 된다.

## 7. 상수 출처

contract §1에 핀된 것(`docs/contract.md`)은 모듈에 숫자를 새로 만들지 않고
`crate::constants`에서만 참조한다 — PNG의 `MAX_W_PX`/`MAX_H_PX`가 그 예다.

아래는 이번 구조 분리에서 **값을 바꾸지 않고** 자리만 옮긴, JS/Python 하드코딩에서
온 숫자들이다. 나중에 상수로 끌어올릴 때 출처 표로 쓴다.

| 값 | 출처 |
|---:|---|
| `step_deg = 0.5` | `arc.js`·`chord.js`의 호형 샘플 간격 |
| `fillet steps = 8` | 픽스처가 사용한 코너당 샘플 수 |
| sheen `alpha 110 / 60`, 너비 `2 / 1` | `composite_ops.js`의 sheen 두 선 |
| `CrtLines` 기본 `y0=45, y1=80, step=3, x0=20, x1=108, alpha=45` | `crt_lines` JS 기본매개변수 |
| `Hud` 기본 `36,60,92,82, arm 5, [255,255,255,235], w 1` | `_draw_hud_brackets` JS 기본값 |
| inset ring 반경 `width.max(0)`·폐방 원반 판정 `dx²+dy²≤r²` | PIL 외곽선 오프셋 규칙 |
| fillet의 `0.45 * tan_half` 클램프·`dot.clamp(-0.999, 0.999)` | `geometry.py fillet_polygon` 원문 |

이 표의 항목들은 이번 작업(구조 분리·동작 무변경)에서 contract.md를 고치지 않았고,
원본 JavaScript/Python에 그대로 남아 있는 값이다. 나중에 끌어올릴 때는 contract 표에
행을 먼저 추가한 뒤 상수로 참조한다.
