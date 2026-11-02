# Rust 400줄 초과 파일 모듈화 진행 상황

> 작업 원칙: 동작 변경 없이 **코드 이동 + `pub use` re-export**만 한다.
> 각 파일 완료 후 `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
> `cargo fmt --all -- --check`를 확인한다. 중단되면 이 표의 상태를 보고 이어서 한다.

## 완료

| 파일 | 이전 | 이후 | 분리 위치 |
|---|---:|---:|---|
| `crates/app/src/ui/atlas.rs` | 1581 | 264 | `ui/atlas/{state,layout_ui,preview,slots,import,export,tests}.rs` |
| `crates/app/src/features/text.rs` | 904 | 22 | `features/text/{state,tool,api,tests}.rs` + `optionbar.rs` |
| `crates/app/src/features/gradient/tool.rs` | 745 | 121 | `features/gradient/{state,tool,api}.rs` |
| `crates/app/src/features/arc/tool.rs` | 712 | 105 | `features/arc/{state,tool,api}.rs` |
| `crates/cli/src/main.rs` | 1192 | 185 | `cli/{atlas,doc,tests}.rs`, `cli/edit/{mod,json}.rs` |
| `crates/app/src/file_ops.rs` | 650 | 250 | `file_ops/atlas.rs`, `file_ops/atlas/tests.rs` |
| `crates/app/src/ui/autosave.rs` | 608 | 345 | `ui/autosave/{mod,tests}.rs` |
| `crates/app/src/ui/dialogs.rs` | 457 | 180 | `ui/dialogs/{apply,window,mod,tests}.rs` |
| `crates/app/src/ui/menubar.rs` | 524 | 302 | `ui/menubar/{menus,mod,tests}.rs` |
| `crates/app/src/ui/settings_store.rs` | 410 | 245 | `ui/settings_store/{mod,tests}.rs` |
| `crates/app/src/ui/toolbar.rs` | 426 | 265 | `ui/toolbar/{icons,mod,tests}.rs` |

## 진행 중 및 남은 파일

없음. 2026-10-29 전체 Rust 소스(`.rs`, `target` 제외) 기준으로 400줄 이상 파일 0개.

이번 분리에는 `engine/examples/bench.rs`의 합성/PNG/JSON 작업, 문서 검증,
입력·세션 처리, PNG scanline, 레시피 명령, 합성·내보내기 테스트도 포함한다.

## 검증 기록

- 2026-10-29: `cargo test --workspace` 통과 (engine 368 + integration tests,
  app 216, CLI 10), `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, `cargo check --workspace --all-targets` 통과.
