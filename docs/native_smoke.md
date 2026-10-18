> `docs/native_smoke.md`
>
> Phase 2 수동 스모크 — 재현 가능한 단계, 기대 결과, 실패 시 의심 모듈.
> GUI를 띄울 수 없는 실행(헤드리스 CI·에이전트)은 항목을 "미실행"으로 적는다.
> 자동 커버: `session_glue::tests::headless_stroke_reaches_the_framebuffer`
> (컨트롤러→매니저→comp→display→undo).

# Native 스모크 (Phase 2)

실행 권장(저장소 루트 PowerShell): `cargo run --release -p pixeldraw`.
직접 실행 파일: `target\release\pixeldraw.exe`.
Debug 실행이 되는 환경에서는 `cargo run -p pixeldraw`도 동일한 UI를 띄운다.
창 제목 `Pixeldraw`, 시작 크기 1920×1080(16:9 — 모니터가 더 작으면 첫 프레임에
같은 비율로 축소·중앙 배치, `shell.rs::fit_window_to_monitor`), 최소 크기 900×560.

## UI 레이아웃

- [ ] 메뉴바와 옵션바가 창의 egui 콘텐츠 전체 폭을 차지하고, 상태바도 하단 전체 폭에 놓인다
      → `ui/mod.rs` 패널 선언 순서
- [ ] 메뉴바 오른쪽에 실행 취소/다시 실행이 보인다. 편집 메뉴 항목과 같은 결과이며
      이력이 없거나 끝에 도달하면 해당 버튼은 비활성이다 → `ui/menubar.rs`
- [ ] 툴 레일에는 도구 선택만 있다. 확대/축소는 보기 메뉴 또는 휠/단축키로 동작한다
      → `ui/toolbar.rs`, `ui/menubar.rs`
- [ ] 펜·도형·호·텍스트 도구를 바꿨을 때 옵션바가 해당 도구 옵션으로 바뀐다.
      채우기·스포이트·손 도구에는 불필요한 옵션이 나오지 않는다 → `ui/optionbar.rs`
- [ ] 왼쪽 색상/팔레트/브러시 패널과 오른쪽 레이어 패널은 옵션바 아래,
      상태바 위에 있고 각각 스크롤된다
- [ ] 기본 창(1920×1080, 16:9 — 모니터가 작으면 같은 비율로 축소된 크기)과 최소 창
      (900×560)에서 패널을 드래그해도 캔버스가
      최소 220pt 폭으로 남고 가로바 폭은 변하지 않는다
- [ ] 왼쪽 패널을 최소 폭으로 줄여도 색상 슬롯·색상환·RGBA 입력이 잘리지 않고
      필요한 항목은 다음 줄로 이어진다
- [ ] 짧은 창 높이에서 툴 레일과 사이드바 끝까지 스크롤할 수 있고,
      상태바의 문서 크기·좌표·확대율은 계속 보인다

## 준비

- [ ] 창이 뜨고 체커보드가 블러 없이 보인다 → `shell.rs`, `canvas_texture.rs`
- [ ] 상단 임시 바에 `연필`이 깨지지 않고 보인다 → `shell.rs` 한글 폰트
      (없으면 `pixeldraw: no Korean system font found` 경고 후 라틴 폴백)

## 그리기

- [ ] 펜 선택 → 드래그 → 선이 보인다. 메뉴바 Undo 버튼 또는 편집 메뉴로 실행 취소 후 사라진다
      → `tools/pen`, `session_glue`, `comp_cache`, `canvas_view`
- [ ] `지우개` → 그은 선 위 드래그 → 지워진다 → `tools/pen.rs` Erase
- [ ] `직선`/`사각형`/`타원` → 드래그 후 모양이 남는다(pending 커밋) → `tools/shape`
- [ ] `페인트통` → 빈 곳 클릭 → 채워진다 → `tools/fill.rs`
- [ ] 손 도구 → 드래그 → 화면이 움직인다. 보기 메뉴의 축소/확대/맞춤/실제 크기 동작
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
| 2026-10-17 | 에이전트 | UI 레이아웃 수동 스모크 미실행. `cargo test -p pixeldraw` 155/155, `cargo fmt --all --check` 통과. 기본/최소 창 GUI 확인 필요 |
