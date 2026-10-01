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
core → model → io ─┐
core → model → render ─┤→ tools → ui
core → shape_raster → shape 도구 ─┘
```

- `core`: 상수·에러·이벤트·픽셀·합성·청크저장·브러시·도형 래스터(`core/raster/` 4파일). DOM 금지.
- `model`: 문서·레이어·Command·Undo·Session(EventTarget). DOM 금지.
- `io`: PNG·검증·직렬화·파일·IndexedDB. `file_io`·`store_idb`만 브라우저 API 접촉(지연 접근).
- `render`: `view`·`composite`는 DOM-free(Node import 가능), `renderer`·`grid`·`background`는 캔버스 표시용.
- `tools`: 순수 로직 + Session `beginEdit` 경유 픽셀 기록. 직접 ChunkStore 변조 금지.
- `ui`: Session 메서드 호출 + Session 이벤트 구독만. 모델 직접 변조 금지.

## 문서 목록

| 파일 | 범위 |
|---|---|
| `contract.md` | 고정 상수·전역 불변 조건·에러 코드 카탈로그·DOM 금지 구역·의존 DAG |
| `core.md` | 상수·에러·이벤트·픽셀·합성·청크·브러시·도형 래스터(`raster/` 4파일) |
| `model.md` | Layer·Document·IdGen·Command·History·Session+Edit |
| `io.md` | base64·PNG·검증·직렬화·내보내기·파일·자동저장 |
| `render.md` | view·composite·renderer·grid·background |
| `tools.md` | base·manager·input·pen/eraser·eyedropper·fill·hand·shape |
| `ui.md` | 부팅 순서·패널·단축키·다이얼로그·상태바·아이콘 |

## 테스트·검증

```bash
npm test                          # node --test tests/ (Node ≥ 20)
npm run parity                     # node tools/parity_check.mjs (115 골든 검사)
node tools/png_cases_check.mjs
node tools/view_check.mjs
python schema/schema_selfcheck.py
```

### 알려진 상태 (2026-10-01)

- `npm test`의 `node --test tests/`는 Node 24에서 디렉터리 인자를 받지 않아 실패한다.
  대신 `node --test "tests/*.test.mjs"`를 직접 실행해야 한다.
- 그 명령 기준 **152건 중 148건 통과, 4건 실패**다. 실패 내역은
  `docs/tools.md` §11-1(커서 기대값 3건)과 `docs/io.md` §11(import 경로 1건)에 기술되어 있다.
- `npm run parity`(115 골든 검사)는 통과한다. 래스터 수학 자체는 안전하다.

## 전역 주의점

전역 규칙의 규범 원문은 `docs/contract.md`다. 요약만 적는다.

- 좌표는 정수 픽셀. 포인터→픽셀은 `Math.floor`, 반올림은 `rnd(v)=Math.floor(v+0.5)`.
- 생성 색은 불투명(255) 또는 투명(0)만. 투명 픽셀은 `(0,0,0,0)` 정규화.
- 픽셀 변경은 `PixelWriter` 또는 Command `do/undo`만 가능.
- Document 변경은 Command로만(생성자·`loadDocument` 제외).
- 에러는 `DrawToolError(code, message, details?)` 하나만. 코드 목록은 `contract.md` §3 참고.
- DOM 금지 구역은 `contract.md` §4 참고.
