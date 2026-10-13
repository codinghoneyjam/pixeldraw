// Grid oracle for the Rust display port (docs/progress/roadmap.md §3 W2).
//
// The JSON is produced by calling the JS originals, never by re-deriving the
// math, so `crates/engine/tests/display_parity.rs` compares Rust against the
// behaviour that shipped rather than against a second implementation of it.
//
//   node tools/display_cases.mjs            # write tools/display_cases.json
//   node tools/display_cases.mjs --stdout   # print the JSON instead
//
// `cases`   — gridLines(view, w, h, vw, vh, mode) over a zoom × mode × pan grid
// `strokes` — gridStrokeDevPx(a, dpr) over the alpha × dpr grid

import { CHUNK_PX, TILE_PX } from "../src/core/constants.js";
import { gridLines, gridStrokeDevPx } from "../src/render/grid.js";
import { writeFileSync } from "node:fs";

const MODES = ["off", "unit", "tile", "pixel"];
const ZOOMS = [0.25, 0.5, 1, 2, 8, 32];

// Pan, canvas size and viewport in CSS px. Negative offsets exercise the
// `k0 = max(0, ...)` clamp; a canvas bigger than the viewport exercises `k1`.
const VARIANTS = [
  { ox: 0, oy: 0, w: 64, h: 64, vw: 200, vh: 150 },
  { ox: 13, oy: -20, w: 128, h: 96, vw: 800, vh: 600 },
  { ox: -7.25, oy: 4.5, w: 32, h: 32, vw: 40, vh: 30 },
  { ox: 40.5, oy: -3, w: 128, h: 128, vw: 300, vh: 200 },
  { ox: -100, oy: 100, w: 2048, h: 1088, vw: 512, vh: 384 },
  { ox: -0.5, oy: -0.5, w: 32, h: 32, vw: 16, vh: 16 },
];

const cases = [];
for (const mode of MODES) {
  for (const z of ZOOMS) {
    for (const v of VARIANTS) {
      const view = { zoom: z, offsetX: v.ox, offsetY: v.oy };
      const out = gridLines(view, v.w, v.h, v.vw, v.vh, mode);
      cases.push({
        z, ox: v.ox, oy: v.oy, w: v.w, h: v.h, vw: v.vw, vh: v.vh, mode,
        out: out && { xs: out.xs, ys: out.ys },
      });
    }
  }
}

const strokes = [];
for (const a of [0.15, 0.3, 0.55]) {
  for (const dpr of [1, 1.25, 1.5, 2, 3]) {
    strokes.push({ a, dpr, px: gridStrokeDevPx(a, dpr) });
  }
}

const payload = {
  cases,
  strokes,
  meta: { chunkPx: CHUNK_PX, tilePx: TILE_PX, cases: cases.length, strokes: strokes.length },
};
const text = JSON.stringify(payload);
if (process.argv.includes("--stdout")) {
  process.stdout.write(text);
} else {
  const out = new URL("./display_cases.json", import.meta.url);
  writeFileSync(out, text + "\n");
  console.log(
    `display_cases.json: ${cases.length} gridLines cases, ${strokes.length} stroke cases ` +
      `(chunk ${CHUNK_PX}, tile ${TILE_PX})`,
  );
}
