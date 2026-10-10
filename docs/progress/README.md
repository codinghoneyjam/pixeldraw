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
                          [ ] 1.9 raster composite_ops
                          [ ] 1.10 composite + kernels
                          [ ] 1.11 io/png
                          [ ] 1.12 model
                          [ ] 1.13 io/serialize
Phase 2  네이티브 캔버스   [ ] 2.1 ~ 2.7
Phase 3  UI              [ ] 3.1 ~ 3.10
Phase 4  자산 파이프라인   [ ]
Phase 5  성능            [ ] 5.1 ~ 5.7
Phase 6  절삭·패키징      [ ] 6.1 ~ 6.7
```

## 누적 게이트

| 지표 | 현재 |
|---|---|
| `cargo test` | **86 / 86** |
| `cargo clippy -D warnings` | 그린 |
| `cargo fmt --check` | 그린 |
| JS 오라클(`npm test`) | 410 / 410 |
| 골든 201건 중 커버 | **148 / 201** |
| JS 베이스라인 | 측정 완료(14항목, `docs/native_plan.md` §6 Phase 0.1) |

### 커밋 이력

| sha | 내용 |
|---|---|
| `d5c2744` | phase 0 + 1.1~1.6 — workspace, oracle freeze, engine core, raster masks/blend/polygon/wide-line |
| `1.7~1.8` | arc/chord near-parity 핀, gradient 절단 규칙, atlas paste 알파 제곱 |
