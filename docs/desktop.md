# 데스크톱 셸 — Tauri

`python run.py`(브라우저) 외에 **네이티브 exe**로 실행하는 경로다. 편집기 코드는
그대로이고, 셸이 네이티브 파일 대화상자와 파일 쓰기만 맡는다.

```bash
npm install          # @tauri-apps/cli (개발 의존성)
npm run app:dev      # run.py + Tauri 창 (개발)
npm run app:build    # src-tauri/target/release/pixeldraw.exe + 설치 파일
npm run app:stage    # exe에 들어갈 파일만 모으기 (빌드가 내부에서 호출)
npm run app:icon     # 아이콘 원본 생성 + 파생 크기
```

## 1. 왜 언어를 안 바꿨나

픽셀 전문 에디터(Aseprite·GraphicsGale·Krita·LibreSprite)는 대부분 C++로
만들어져 있다. 그래도 이 프로젝트는 JS를 그대로 쓴다. 이유는 세 가지다.

1. 캔버스 2D + RGBA 버퍼로 충분하다. 픽셀 에디터가 C++이 압도적인 건 역사와
   배포 크기의 문제이지, 어떤 API를 써야 했기 때문이 아니다.
2. `src/core/**`의 래스터는 Pillow와 비트 단위로 맞춰져 있다(라인 마스크
   54/54, golden 201건, recipe 패리티 39/39). 언어를 바꾸면 이 검증 전부를
   다시 쌓아야 한다.
3. 테스트 60개 파일 7,900줄이 Node의 DOM-free 모듈 위에서 돈다.

## 2. 구조

```
index.html ─ src/ui/app.js ─ src/io/file_io.js ─ 쳐
                                                 ├ window.__TAURI__ 있음 → 네이티브 명령
                                                 └ 없음 → 브라우저 피커/다운로드
src-tauri/src/lib.rs ─ pick_save_path · pick_open_path · write_file_bytes · read_file_bytes
```

`file_io.js`는 모듈 로드 시점에 `window`를 만지지 않는다. 호출할 때
`window.__TAURI__`가 있으면 네이티브 경로, 없으면 기존 브라우저 경로를 쓴다.
그래서 `run.py`로 여는 것과 exe로 여는 것이 **같은 소스**다.

- **저장**: `pick_save_path`(네이티브 저장 대화상자) → 사용자가 고른 경로를
  `write_file_bytes`에 그대로 쓴다. 다운로드 폴더로 흘러가지 않는다.
- **열기**: `pick_open_path` → `read_file_bytes` → `new File([bytes], name)`.
  File을 돌려주므로 `readJsonFile`과 `src/ui/actions/file_actions.js`는
  네이티브 런타임인지 모른다.
- **페이로드**: base64 문자열로 주고받는다. 바이너리를 IPC에 어떻게 싣는지는
  Tauri 버전에 따라 달랐는데, base64는 JSON만 있으면 어디서나 같게 동작한다.
  인코더는 `src/io/base64.js`(`btoa`/`atob`)를 그대로 쓰고, Rust 쪽에는 같은
  RFC 4648 변환이 `src-tauri/src/lib.rs`에 있다(테스트 3건).

## 3. 무엇이 exe에 들어가나

`tools/stage_app.mjs`가 `build/app/`에 런타임 파일만 복사한다.

| 들어가는 것 | 안 들어가는 것 |
|---|---|
| `index.html` | `tests/` 60개 파일 |
| `style_*.css` 4개 | `tools/` (검증·베이킹) |
| `src/**/*.js` (117개) | `asset_work/` 673개 파일 |
| `tools/recipe/text_glyphs.json` | `viewer/`·`schema/`·`reference/`·`docs/` |

배포 대상 123개 파일 640 KiB. `tools/check_stage.mjs`는 이 안에서 정적 import가
깨지지 않았는지 모듈 그래프를 따라가며 확인한다(108개 모듈).

`tauri.conf.json`의 `build.frontendDist`가 `../build/app`을 가리키고,
`build.beforeBuildCommand`가 스테이징을 먼저 실행한다.

## 4. 런타임 요구사항

윈도우의 **WebView2 런타임**을 쓴다. Windows 10 21H2 이후·Windows 11에는 기본
설치되어 있고 자동으로 업데이트된다. 없는 환경에 설치 파일로 배포하려면 Tauri의
`webviewInstallMode`를 `downloadBootstrapper` 또는 `embedBootstrapper`로 바꾼다
(인터넷 또는 파일 크기 증가).

브라우저와 달리 exe는 오프라인에서도 `localStorage`(설정)·`indexedDB`
(자동저장)가 전부 동작한다. `file://` 단일 HTML로 열면 Chromium이 이 둘을 막아
설정과 자동저장이 죽는데, 데스크톱 셸은 출처(origin)가 `http://tauri.localhost`라
그런 제한이 없다.

## 5. 규약과 충돌하는 지점

- `docs/contract.md` §5의 "실행 진입점은 `run.py` 하나다"는 이제 틀렸다.
  규범 원문을 "브라우저 진입점은 `run.py`, 네이티브 진입점은 `src-tauri/`"로
  고쳤다.
- `src/`는 계속 상대 경로 ESM import만 쓴다. `window.__TAURI__`는 번들러나
  바벨이 아니라 Tauri가 주입하는 전역 객체이므로 번들 규칙과 무관하다.
- `package.json`에 `devDependencies`가 하나 생겼다(`@tauri-apps/cli`).
  **런타임 의존성은 여전히 0개**고, npm 설치 없이도 `python run.py`는 그대로
  돌아간다.

## 6. 상태 (2026-10-10)

- `npm run app:build` 성공 — `pixeldraw.exe` 9.1 MB, 창 타이틀 `Pixeldraw`,
  기동 메모리 33 MB.
- `cargo test --lib` 3/3 (base64 라운드트립·패딩·거부).
- `npm test` 410/410 (네이티브 브리지 6건 추가).
- 설치 파일(NSIS)은 NSIS 다운로드 단계에서 `os error 5`로 한 번 실패했다.
  exe 자체는 그 전에 완성돼 있으므로, 설치 파일이 필요하면
  `%LOCALAPPDATA%\tauri\nsis-3.11`을 지우고 재빌드한다.
