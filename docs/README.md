# draw_tool_v2 — 모듈 문서

픽셀펜 래스터 에디터. 32px 청크 레이어, 안티앨리어싱 없음, npm 의존성 0, 빌드 단계 없음.

용어(색·청크·브러시·좌표 정수 규칙 등)는 `docs/contract.md` §1–§4를 따른다.

`docs/contract.md`가 전역 규칙(고정 상수·불변 조건·에러 코드·DOM 금지 구역)의 유일한 규범 문서다.
코드 주석의 `contract §N` 표기도 이 문서를 가리킨다.

## 실행

```bash
cd draw_tool_v2
python run.py
```

`run.py`가 빈 루프백 포트를 자동으로 잡고 서버를 띄운 뒤 기본 브라우저를 연다. `Ctrl+C`로 종료.

| 옵션 | 설명 |
|---|---|
| `--port N` | 지정 포트를 먼저 시도. 이미 쓰고 있으면 자동으로 다른 포트를 고른다 |
| `--no-browser` | 브라우저를 열지 않고 URL만 출력 |
| `--host H` | 바인드 주소(기본 `127.0.0.1`) |

ESM이므로 `file://` 직접 열기는 미지원(Chromium CORS 차단). 그래서 로컬 서버가 반드시 필요하다.

구버전의 `tools/bundle_single.py` 단일 HTML 번들러와 그 산출물은 **제거되었다**. 실행 진입점은 `run.py` 하나다.

## 아키텍처 DAG

```
core → features/document → io ─┐
core → features/document → render ─┤→ tools → ui
core → shape_raster → features/shape 도구 ─┘
```

- `core`: 상수·에러·이벤트·픽셀·합성·청크저장·도형 래스터(`core/raster/`). DOM 금지.
- `features/`: **툴 기능 단위 피처 패키지**. 각 피처가 자기 도구·패널·모델을 함께 가진다.
  `document`(문서·히스토리·세션·설정검증), `layers`(레이어·Command·편집세션·패널),
  `color`(색상 패널 7파일), `pen`(펜·지우개·브러시·브러시 패널),
  `shape`(shape 도구 6파일), `fill`·`eyedropper`·`hand`(단일 도구),
  `viewport`(view 수학 + view_store).
- `io`: PNG·검증·직렬화·파일·IndexedDB. `file_io`·`store_idb`만 브라우저 API 접촉(지연 접근).
  `serialize.js`는 래스터 JSON 절반 + 배럴, `serialize_vector.js`·`serialize_recipe.js`가 형제.
- `render`: `composite`는 DOM-free(Node import 가능), `renderer`·`grid`·`background`는 캔버스 표시용.
- `tools`: 입력 플럼빙(`tool_base`·`tool_manager`·`input_controller`와 리스너·이벤트 정규화).
  도구 구현체는 전부 `features/`로 옮겨갔다. 직접 ChunkStore 변조 금지.
- `ui`: Session 메서드 호출 + Session 이벤트 구독만. 모델 직접 변조 금지.

상세는 `docs/contract.md` §4(DOM 금지 구역)·§6(DAG)을 규범으로 본다.

## 문서 목록

| 파일 | 범위 |
|---|---|
| `contract.md` | 고정 상수·전역 불변 조건·에러 코드 카탈로그·DOM 금지 구역·의존 DAG |
| `core.md` | 상수·에러·이벤트·픽셀·합성·청크·도형 래스터(`raster/`) |
| `model.md` | 문서·히스토리·세션·레이어·Command (`features/document/`·`features/layers/`) |
| `io.md` | base64·PNG·검증·직렬화·내보내기·파일·자동저장 |
| `render.md` | view·composite·renderer·grid·background |
| `tools.md` | 베이스·매니저·입력 + features/pen·shape·fill·eyedropper·hand |
| `ui.md` | 부팅 순서·패널·단축키·다이얼로그·상태바·아이콘 |

## 테스트·검증

```bash
npm test                          # node --test tests/ (Node ≥ 20)
npm run parity                     # node tools/parity_check.mjs (115 골든 검사)
node tools/png_cases_check.mjs
node tools/view_check.mjs
python schema/schema_selfcheck.py
```

### 알려진 상태 (2026-10-08)

- `npm test`의 `node --test tests/`는 Node 24에서 디렉터리 인자를 받지 않아 실패한다.
  `package.json`의 스크립트 수정이 남아 있다. 현재는 `node --test "tests/*.test.mjs"`를
  직접 실행할 것.
- 그 명령 기준 **324건 전부 통과**한다.
- 나머지 게이트도 모두 통과한다: parity 201건·png_cases 5/5·view OK·schema selfcheck OK·
  recipe parity 39/39·layer gallery 34/34·tool integrity check 31건.
- `node tests/tool_integrity_check.mjs`는 2026-10-08부터 300줄 초과 검사 범위를 `src/`로
  좁혔다. 이전에는 리포지토리 전체를 훑어 `asset_work/`·`tools/`·`viewer/`의
  생성물·오라클·빌드 스캐폴드가 줄 수 기준을 넘어 **항상 실패**하고 있었다.
  현재 기준 `src/` 내 300줄 초과 파일은 `src/core/raster/polygon.js`(301줄, 예약 면제)
  단 하나뿐이다.

### 수정 이력

- `tests/io_basic.test.mjs:16` — `buildMetaRecord`/`buildChunkRecords`/`chunkRecordKey`
  import 경로를 `idb_record_builder.js`로 변경. 분리 후 `store_idb.js`가 이 심볼을
  재노출하지 않아 모듈 로드가 실패하고 io 계층 검증이 0건 실행되던 상태였다.
- `tests/tools_basic.test.mjs` — 커서 기대값 3건을 `"crosshair"` 하드코딩에서
  `cursorFallback()` 헬퍼로 교체. SVG `url(...)` 형태와 폴백 키워드만 검증하므로
  커서 시각을 바꿔도 깨지지 않는다. `fill`의 폴백은 `copy`다(기존 기대값과 다름).

## 전역 주의점

전역 규칙의 규범 원문은 `docs/contract.md`다. 요약만 적는다.

- 좌표는 정수 픽셀. 포인터→픽셀은 `Math.floor`, 반올림은 `rnd(v)=Math.floor(v+0.5)`.
- 생성 색은 불투명(255) 또는 투명(0)만. 투명 픽셀은 `(0,0,0,0)` 정규화.
- 픽셀 변경은 `PixelWriter` 또는 Command `do/undo`만 가능.
- Document 변경은 Command로만(생성자·`loadDocument` 제외).
- 에러는 `DrawToolError(code, message, details?)` 하나만. 코드 목록은 `contract.md` §3 참고.
- DOM 금지 구역은 `contract.md` §4 참고.
