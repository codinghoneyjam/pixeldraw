> `docs/native_smoke.md`
>
> Phase 2 수동 스모크 — 재현 가능한 단계, 기대 결과, 실패 시 의심 모듈.
> GUI를 띄울 수 없는 실행(헤드리스 CI·에이전트)은 항목을 "미실행"으로 적는다.
> 자동 커버: `session_glue::tests::headless_stroke_reaches_the_framebuffer`
> (컨트롤러→매니저→comp→display→undo).

# Native 스모크 (Phase 2)

실행: `cargo run -p pixeldraw` (창 제목 `Pixeldraw`, 1280×820).

## 준비

- [ ] 창이 뜨고 체커보드가 블러 없이 보인다 → `shell.rs`, `canvas_texture.rs`
- [ ] 상단 임시 바에 `연필`이 깨지지 않고 보인다 → `shell.rs` 한글 폰트
      (없으면 `pixeldraw: no Korean system font found` 경고 후 라틴 폴백)

## 그리기

- [ ] `연필` 선택 → 드래그 → 선이 보인다. undo(`↩`) 후 사라진다
      → `tools/pen`, `session_glue`, `comp_cache`, `canvas_view`
- [ ] `지우개` → 그은 선 위 드래그 → 지워진다 → `tools/pen.rs` Erase
- [ ] `직선`/`사각형`/`타원` → 드래그 후 모양이 남는다(pending 커밋) → `tools/shape`
- [ ] `페인트통` → 빈 곳 클릭 → 채워진다 → `tools/fill.rs`
- [ ] `손` → 드래그 → 화면이 움직인다. `−`/`+` 줌, `맞춤`, `1:1` 동작
      → `input/controller.rs`, `view_store`
- [ ] 마우스 휠 → 커서 위치 기준 줌. Shift+휠 → 가로 팬 → `input/wheel.rs`

## 파일

- [ ] `저장` → `.pixeldraw.json` 기록 → `file_ops.rs`, `platform/files.rs`
- [ ] `열기` → 저장한 JSON → 레이어·픽셀 동일 → `io/serialize`
- [ ] `PNG` → 외부 뷰어로 열리는 PNG → `io/png`, `io/export`
- [ ] `열기` → PNG 파일 → 새 문서로 픽셀 복원 → `file_ops::open_png_bytes`
- [ ] `새 문서` → 512×512 빈 문서 + 맞춤 → `file_ops::new_document`

## 실패 시 의심 모듈

| 증상 | 볼 곳 |
|---|---|
| 그어도 안 보인다 | `session_glue::drain_events` → `comp.update` → `canvas_view::render_frame` |
| 그은 게 undo 안 된다 | `model/history`, `ToolManager::switch_to` 취소 여부 |
| 휠이 이상하다 | `input/wheel.rs` residue, `input_adapter::poll_frame_input` 부호 |
| 한글 깨짐 | `shell::install_korean_font`, 시스템 폰트 존재 |
| 저장 파일이 다르다 | `io/serialize` 키 순서 (Phase 1.13 게이트가 핀) |

## 실행 기록

| 날짜 | 실행자 | 결과 |
|---|---|---|
| 2026-10-12 | 에이전트 | 미실행 (헤드리스). 자동 테스트 451/451 + headless 프레임 테스트로 대체 |
| 2026-10-12 | 에이전트 | `cargo build --release -p pixeldraw` 성공 (38s, `pixeldraw.exe` 5.8MB). 창 실행은 미실행 |
