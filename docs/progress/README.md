# 진행 로그 — native 이전

`docs/native_plan.md`가 **무엇을 할지**(로드맵)라면, 이 폴더는 **무엇을 했는지**를
기록한다. Phase 단위 작업이 끝날 때마다 두 파일을 갱신하고 커밋한다.

| 파일 | 역할 |
|---|---|
| `README.md` | 이 문서 — 규칙, 현재 상태 보드, 검증 명령 |
| `log.md` | **append-only 작업 로그**. 최신 항목이 위 |

## 규칙

1. **태스크 완료 = 로그 1줄 + 보드 체크 + 커밋.** 셋 중 하나만 하면 안 된다.
2. 커밋은 **이전 작업 파일만** `git add`로 명시한다. 사용자의 미커밋 WIP
   (`src/`·`tests/`·`asset_work/`·`viewer/` 등)은 절대 같이 커밋하지 않는다.
3. 커밋 메시지 첫 줄은 `phase N: <한 줄 요약>`.
4. 로그에는 반드시 **테스트 수(전→후)**와 **막힌 점/발견**을 적는다. 숫자가
   없으면 진척도 판단할 수 없다.
5. `native_plan.md`의 상태표와 이 폴더의 보드가 어긋나면 **이 폴더가 최신**이다.
   native_plan.md는 Phase 종료 시점에만 따라간다.
6. **오케스트레이터 운용 규칙**(2026-10-11 추가).
   - 작업은 **파일 disjoint + 의존성 없게** 나눠 서브에이전트에 분배한다. 두 에이전트가
     같은 파일을 만지게 하지 않는다(서로의 컴파일을 깨뜨려 게이트 판정이 불가능해짐).
   - **모듈 크기: 300줄 이상이면 검토, 400줄은 하드리밋.** 넘으면 분할한다.
   - **모듈 분리는 파사드 패턴을 기본**으로 한다. `mod.rs`/`<name>.rs`는
     `mod` 선언 + `pub use` 재수출만, 구현은 하위 모듈로. 예: `view.rs`.
   - **주석은 불필요하고 긴 설명을 배제한다.** 남기는 것은 "서명만 보면 알 수 없는
     것" — JS 패리티 트랩, 실측 수치, 명백한 수정을 일부러 하지 않은 이유, 비트
     동일성을 지탱하는 연산 순서뿐. 코드를 그대로 되풀이하는 주석은 삭제.
   - **긴 설명은 md에 쓴다.** 근거·도표·배경은 `docs/*.md`로 보내고 소스에는 한 줄
     포인터만 남긴다. 예: `docs/view_math.md`.
   - **상수는 국소적이지 않으면 `docs/contract.md`를 먼저 고친다.** JS 기본매개변수
     값이나 하드코딩 숫자를 모듈에 새로 만들지 않는다. contract 표에 올리고 상수로 참조.

## 커밋 대상 경로

```
Cargo.toml Cargo.lock crates/ .gitignore
docs/native_plan.md docs/README.md docs/progress/
tools/bench_baseline.mjs tools/oracle_manifest.mjs tools/oracle-manifest.json
```

## 검증 명령

```bash
cargo test --workspace                          # 골든 + 모델 + 히스토리
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
node tools/oracle_manifest.mjs --check          # JS 오라클 동결 확인
node tools/bench_baseline.mjs                   # JS 베이스라인 재측정
npm test                                        # JS 오라클 410
```

## 상태 보드

```text
Phase 0  기반 정비        [x] 0.1 베이스라인  [x] 0.2 워크스페이스
                          [x] 0.3 오라클 동결 [x] 0.4 픽스처 하네스  [x] 0.5 보류
Phase 1  엔진             [x] 1.1 constants/errors/events/pixel
                          [x] 1.2 blend
                          [x] 1.3 chunkstore
                          [x] 1.4 raster brush+masks
                          [x] 1.5 raster segment (wide line)
                          [x] 1.6 raster polygon
                          [x] 1.7 raster arc/chord
                          [x] 1.8 raster gradient/atlas
                          [x] 1.9 raster composite_ops
                          [x] 1.10 composite + kernels + snap
                          [x] 1.11 io/png + io/base64
                          [x] 1.12 model
                          [x] 1.13 io/serialize + vector/recipe/validate (39/39 native parity)
                          [x] 1.13a chunkstore 파사드 분할
                        [x] 1.13b 파사드 분할 10모듈 (io/png·model 6·raster 3)
Phase 2  네이티브 캔버스   [x] 2.3 view 선행 (파사드 7분할)
                          [ ] 2.1 · 2.2 · 2.4 ~ 2.7
Phase 3  UI              [x] 3.1 ~ 3.10
Phase 4  자산 파이프라인   [x] 4.1 viewer 제거, 개발 자산 유지(A 선택)
Phase 5  성능            [ ] 5.1 ~ 5.7
Phase 6  절삭·패키징      [ ] 6.1 ~ 6.7
```

시각 보드: [board.html](board.html) — 브라우저로 열면 되는 오프라인 단일 파일.

## 누적 게이트

| 지표 | 현재 (2026-10-11 실측) |
|---|---|
| `cargo test --workspace` | **321 / 321** (lib 284 + integration 37, native recipe parity 39/39 포함) |
| `cargo clippy -D warnings` | 그린 |
| `cargo fmt --check` | 그린 |
| JS 오라클(`npm test`) | 410 / 410 |
| 래스터 골든 201건 중 커버 | **201 / 201** |
| `png_cases` 5/5 | **5 / 5** |
| Phase 1 게이트 | **완료** — Rust native recipe 39/39, schema consistency 포함 JS 410/410; view 수학 2.3 선행 완료 |
| JS 베이스라인 | 측정 완료(14항목, `docs/native_plan.md` §6 Phase 0.1) |

### 커밋 이력

| sha | 내용 |
|---|---|
| `d5c2744` | phase 0 + 1.1~1.6 — workspace, oracle freeze, engine core, raster masks/blend/polygon/wide-line |
| `dcb1673` | phase 1.7~1.8 — arc/chord near-parity 핀, gradient 절단 규칙, atlas paste 알파 제곱 |
| `1314937` | phase 1.9 — apply_shape 진입점 + composite_ops 5종 |
| `85fe248` | phase 1.10~1.11 — snap 37건, composite/kernels, png 5/5 |
| `8c17959` | phase 1.12+2.3 — model 15모듈 + view 수학, 285 그린 |
| `6158fc3` | phase 2.3b — view 파사드 7분할, contract 상수 3핀 |
| `93f024c` | docs — 1.12 parity 트랩 기록 (가드 순서·Undo 정체성·병합 정책) |
| `37cf0f6` | phase 1.13a — chunkstore 파사드 분할 |
| `69f3467` | phase 1.13b — io/png·model 6·raster 3 파사드 분할, internals 문서 2편 |
