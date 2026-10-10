# 작업 로그 (최신이 위)

형식: `## YYYY-MM-DD · Phase N.M · 제목` 뒤에 bullets.

---

## 2026-10-11 · Phase 1.7~1.8 · arc/chard, gradient, atlas paste

**커밋 대상**: `crates/engine/src/raster/{arc,gradient,atlas,mod}.rs`·`docs/progress/`
**테스트**: 67 → **86 그린**

- **1.7 arc/chord** — 골든 4건 전부 허용오차 안에 통과.
  - **near-parity 허용오차를 명시적 상수로 핀**(`raster::arc::near_parity`).
    `arc` missing 0/extra ≤16, `chord fill` missing 0/extra ≤4,
    `chord outline` missing ≤16/extra ≤40. **PIL에 더 가깝게 만들면 게이트가
    실패한다**는 점이 핵심 — 포팅은 JS 재현이지 오라클 정합이 아니다.
  - **`Math.round`은 `floor(v+0.5)`.** 부호와 무관하다. 내가 불필요하게
    분기했다가 `Math.round(-2.5)`가 -3이 나와 테스트가 잡았다. (`f64::round`은
    half-away-from-zero라 **쓰면 안 된다**.)
  - 각도는 PIL 규약(0°=3시, 시계방향 증가). `end < start`면 360을 더해 정규화.
- **1.8 gradient/atlas** — 골든 7건(gradient 2·paste 5) 전부 통과.
  - gradient는 **반올림이 아니라 절단**(`int()`). 레거시 쇼크웨이브 베이커의
    T-5 알파 패리티 게이트가 기준. half-up과는 절반의 픽셀에서 다르다.
  - atlas의 paste 법칙: `sheet.paste(part, pos, part)` — **타일이 자기 자신을
    마스크로** 쓰이므로 알파가 제곱된다(128 → 64). 평범한 source-over로는
    재현 불가. 256 알파 × 2000 RGBA로measured된 닫힌 형식을 그대로 쓴다.
  - **픽스처 hex는 RGBA 바이트 순서**다("0A141EFF" → r=0A). big-endian u32로
    파싱하면 채널이 뒤집힌다. 내가 그렇게 했다가 golden이 잡아줬다.

---

## 2026-10-11 · Phase 0 + 1.1~1.6 · 엔진 기초·블렌드·청크저장·래스터 4종

**커밋 대상**: `Cargo.toml`·`Cargo.lock`·`crates/`·`docs/`·`tools/`(신규 3개)
**테스트**: 0 → **67 그린**

### Phase 0 — 기반 정비

- **0.1 베이스라인 측정**(`tools/bench_baseline.mjs`, 14항목).
  2048×1088·64레이어 기준 median:
  - stroke pen=1 2.69ms / pen=8 44.9ms / **pen=64 2785.7ms**(세그먼트당 ~139ms)
  - floodFill 146.2ms · applyShape ellipse fill 20.4ms · polygon fill 26.8ms
  - **composite 2176청크×64레이어 391.2ms** · **flattenToRgba 2002.3ms**
  - exportPngBytes 2070.7ms · documentToJson 1909.7ms(JSON 7.5MB)
  - 찾은 함정: floodFill은 입주를 변형시키므로 2회차부터 무효 샘플이 된다.
    `timeFresh()`(샘플마다 새 컨텍스트)로 고쳐 재측정.
- **0.2 워크스페이스**: 루트 `Cargo.toml` + `crates/{engine,platform,app}`.
  `unsafe_code = "forbid"`, `panic = "abort"`(release). `src-tauri`는
  `exclude`(Phase 6까지 base64 테스트가 오라클로 가치 있음).
- **0.3 오라클 동결**(`tools/oracle_manifest.mjs`): src/tests/reference 193파일
  sha256 매니페스트. 태그가 아닌 파일 기반 — 미커밋 트리에서도 동작하고,
  어떤 파일이 움직였는지 정확히 말해준다. digest `de5de4f5…`.
- **0.4 픽스처 하네스**: 처음 `tests/common/`에 뒀다가 `src/fixtures.rs`로
  이동. 단위 테스트도 같은 리더를 써야 해서. 부수적으로 `serde_json`이 정규
  의존성이 됐는데 1.13에서 원래 필요하므로 순서가 앞당겨진 셈.
- **0.5 보류**: `src/`를 `reference-app/`으로 옮기면 tests의 상대 import
  수백 개가 한 번에 깨진다. 오라클은 제자리에서 도는 편이价值. 삭제는
  Phase 6.3로 이관.

### Phase 1.1~1.3 — 기초·블렌드·청크저장

- 상수·에러·이벤트·픽셀 1:1 포팅.
- **발견: 에러 카탈로그가 문서(19종)와 코드(21종)가 다르다.**
  `VIEWPORT_INVALID`·`VIEWPORT_OUT_OF_RANGE`가 코드에만 있다. 코드가 진실이므로
  21종으로 포팅하고 `CATALOG` 상수로 핀(중복 검사 테스트 포함).
- **발견: `GRID_MODES` 4종이 `docs/core.md` 표에 없다.** 함께 포팅.
- `is_all_zero`는 `unsafe` 없이 바이트 스캔. 워크스페이스가 unsafe를 금지하고,
  LLVM이 같은 워드 단위 루프로 벡터화한다.
- `forEachChunk` 순서는 **키 오름차준(`cy*64+cx`)** 이지 행 우선이 아니다.
  처음 내 기대값을 행 우선으로 적어 테스트가 실패했고, 그게 원본 규칙임을 확인.
- 내 테스트 기대값 오류 2건(after 값 착각·순서 착각) — 전부 내 실수.

### Phase 1.4~1.6 — 래스터 4종

- 골든 그룹 실제 데이터 통과: brush 14+hashes 6, ellipse 15, rrect 8,
  outline 10, bresenham 7, **wide_line 54**, polygon 6, polygon_outline 6,
  fillet 5.
- **`Math.fround`은 `f32` 캐스팅.** 폴리곤 `dx`·교점은 C float. 두 f32의 곱은
  f64에 정확하므로 "f32 곱"과 "정확한 곱의 fround"가 비트 동일 — 이 성질로
  `inter_x(y, e)` 헬퍼 하나가 JS의 3중 fround 식을 그대로 재현.
- **반올림 4종을 혼동 금기**: `round_up`/`round_down`(span 경계) ·
  `round_c`(C roundf) · `py_round`(Python half-to-even). 대비 테스트로 못 박음.
- **`usize` 부호있는 뺄셈**: `ellipseMask`의 `2x+1-w`는 왼쪽 반에서 음수.
  JS는 float라 통과, Rust는 디버그 패닉 → 전부 i64.
- **`PixelSink` 트레이트**: `PixelWriter` 구현은 음수 좌표를 버린다. `u32`
  캐스팅을 그대로 쓰면 (−5,−5)가 (2043,1083)에 찍히는 사고. 테스트로 못 박음.
- **`lineMask`는 최소 bbox** 반환. 골든 `wide_line`은 64×64 **전체 캔버스**
  기준이라 테스트에서 W×H에 블릿 후 비교.
- **픽스처 키 불일치**: `pts`(polygon 계열) vs `points`, `w/h` vs
  `W/H`(wide_line·stroke). 원본 불일치이며 규칙을 새로 만들지 않았다 —
  `points_from_json`은 두 키를 모두 수용.
- **큰 마스크는 `hashes` 그룹으로 검증**: brush 64/33, ellipse 128×64,
  rrect 200×120은 행 인라인이 불가. `sha2` dev-의존으로 sha256 재현.
  (직접 구현했다가 100줄 낭비임을 인정하고 크레이트로 교체.)

### 남은 과제 메모

- `stroke` 그룹(7건)은 `brushFootprint`+`strokeMask`(여러 점 경로) 조합이라
  1.10 kernels에서 함께 검증한다.
- `angle_snap`(8)·`resize`(15)·`drag_bbox`(8)·`unit_snap`(6)은 **스냅 기하**로
  1.7 이후 별도 모듈(`raster/snap.rs`)에서 검증. `core_basic`의 view/스냅
  테스트가 대응.
- `gradient`(2)·`paste`(5)는 1.8. `composite`(8)·`fill`(3)은 1.10.
- `arc`(2)·`chord`(2)는 **near-parity 허용오차**를 JS와 똑같이 재현해야
  한다(missing 0 / extras ≤16 등). 첫 구현은 1.7.
