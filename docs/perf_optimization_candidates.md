# Performance Optimization Candidates

This document tracks the measured JS baseline, the Rust port baseline, the
current optimization experiments, and the planned next steps. It is intentionally
conservative: every source-changing optimization must keep `cargo test`,
golden parity, golden pixel equality, and case counts unchanged, otherwise it is
rolled back.

## Current measurements (2048×1088, 64 layers)

| Item | JS baseline | Rust port baseline | Target-cpu native experiment |
|---|---:|---:|---:|
| stroke pen=1 (20 segs) | 1.578 ms | 0.221 ms | - |
| stroke pen=8 (20 segs) | 42.406 ms | 4.303 ms | - |
| stroke pen=64 (20 segs) | 2654.258 ms | 29.540 ms | 34.381 ms |
| strokePoint pen=64 | 0.162 ms | 0.004 ms | - |
| floodFill empty canvas | 136.496 ms | 6.561 ms | 6.613 ms |
| floodFill canvas already RED | 148.076 ms | 9.977 ms | - |
| applyShape ellipse 1024² fill | 19.097 ms | 2.999 ms | - |
| applyShape rrect outline | 10.602 ms | 4.586 ms | - |
| applyShape line 2048×1088 sw=8 | 4.052 ms | 1.303 ms | - |
| applyShape polygon 32-gon | 25.651 ms | 1.002 ms | - |
| composite 2176 chunks × 64 | 320.167 ms | 104.783 ms | - |
| composite (rayon) | - | 15.810 ms | 13.461 ms |
| flattenToRgba | 1810.042 ms | 14.934 ms | 13.996 ms |
| exportPngBytes | 1877.174 ms | 32.947 ms | - |
| documentToJson (save) | 1702.710 ms | omitted | - |

## Current frame-loop measurements (64 layers)

| Stage | Measured |
|---|---:|
| doc().cloned() benchmark only | 52.358 ms |
| comp.update() 1 chunk | 0.904 ms |
| draw_display_into() | 0.262 ms |
| texture upload copy | 0.269 ms |

The real `render_frame` path borrows `&Document`; it does not clone the document.

## Rollback policy

- Source changes are rejected if golden parity, golden hashes, `png_cases`, model/session tests, clippy, or fmt fail.
- Target-cpu native is an environment experiment only. It is currently mixed: better for composite/flatten, slower for stroke pen=64, so it should not replace the default release profile yet.
- Algorithm changes that improve one workload but regress another by more than 10% should be rolled back or made optional behind `cfg`.

## Candidate optimizations, not yet applied

1. Flood fill hierarchical chunk/block fill plus bitwise scanline fill.
2. Composite layer skipping plus SIMD elementwise blend.
3. Stroke tile reuse and brush footprint caching.
4. PNG filter and deflate tuning without changing decoded pixels.
5. Optional target-cpu-native or PGO build profiles.

## Applied build changes

None yet. The default `Cargo.toml` remains:

```toml
opt-level = 3
lto = "thin"
codegen-units = 1
panic = "abort"
```
