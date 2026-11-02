// Phase 0.1 — JS 엔진 성능 베이스라인. Run: node tools/bench_baseline.mjs
//
// Measures the current ESM engine so the Rust port (docs/native_plan.md §0.1)
// has numbers to beat. Nothing here is a gate — it is a measurement.
//
// Node >= 20. No arguments; edit the CONFIG block to change the workload.

import { Session } from "../src/features/document/session.js";
import { newDocument } from "../src/features/document/document.js";
import { compositeChunk } from "../src/render/composite.js";
import { flattenToRgba, exportPngBytes } from "../src/io/export_png.js";
import { documentToJson } from "../src/io/serialize.js";
import { floodFillScanline } from "../src/features/fill/fill.js";
import { strokeSegment, strokePoint } from "../src/features/pen/brush.js";
import { applyShape } from "../src/core/raster/shape_raster.js";
import { packRGBA } from "../src/core/pixel.js";
import { PixelWriter } from "../src/core/chunkstore.js";
import { CHUNK_PX } from "../src/core/constants.js";
import { writeFileSync } from "node:fs";

// <META - ROLE : workload knobs | L16-24>
const CONFIG = {
  widthPx: 2048,
  heightPx: 1088,
  layers: 64,
  penSizes: [1, 8, 64],
  fillRuns: 3,
  warmup: true,
};

const RED = packRGBA(255, 0, 0, 255);
const BLUE = packRGBA(0, 0, 255, 128);

// <META - ROLE : median-of-N timer that guards against a single GC spike | L26-36>
function timeIt(fn, runs = 5) {
  const samples = [];
  for (let i = 0; i < runs; i++) {
    const t0 = performance.now();
    fn();
    samples.push(performance.now() - t0);
  }
  samples.sort((a, b) => a - b);
  return {
    median: samples[Math.floor(samples.length / 2)],
    min: samples[0],
    max: samples[samples.length - 1],
  };
}

async function timeItAsync(fn, runs = 5) {
  const samples = [];
  for (let i = 0; i < runs; i++) {
    const t0 = performance.now();
    await fn();
    samples.push(performance.now() - t0);
  }
  samples.sort((a, b) => a - b);
  return {
    median: samples[Math.floor(samples.length / 2)],
    min: samples[0],
    max: samples[samples.length - 1],
  };
}

// <META - ROLE : per-sample fresh context, for ops that mutate their own input | L64-76>
function timeFresh(makeCtx, op, runs = 3) {
  const samples = [];
  for (let i = 0; i < runs; i++) {
    const ctx = makeCtx();
    const t0 = performance.now();
    op(ctx);
    samples.push(performance.now() - t0);
  }
  samples.sort((a, b) => a - b);
  return {
    median: samples[Math.floor(samples.length / 2)],
    min: samples[0],
    max: samples[samples.length - 1],
  };
}

// <META - ROLE : a fresh PixelWriter over a layer's store, as tools open one | L54-62>
function newPixelWriter(layer) {
  return new PixelWriter(layer.store, layer.id);
}

// <META - ROLE : a doc with every layer carrying content, the worst case for compositing | L64-85>
function makeBusyDoc() {
  const session = new Session();
  const doc = session.newDocument({
    widthPx: CONFIG.widthPx,
    heightPx: CONFIG.heightPx,
    background: "transparent",
  });
  for (let i = 1; i < CONFIG.layers; i++) session.addLayer();
  doc.layers.forEach((layer, li) => {
    layer.opacity = li % 4 === 0 ? 0.5 : 1;
    layer.blend = ["normal", "multiply", "screen", "overlay", "darken", "lighten"][li % 6];
    // Diagonal strokes give every layer a spread of chunks plus alpha variety.
    const writer = newPixelWriter(layer);
    for (let k = 0; k < 24; k++) {
      strokeSegment(
        writer,
        (k * 97) % CONFIG.widthPx,
        (k * 61) % CONFIG.heightPx,
        ((k * 97) + 400) % CONFIG.widthPx,
        ((k * 61) + 300) % CONFIG.heightPx,
        5,
        k % 3 === 0 ? BLUE : RED,
      );
    }
    writer.finish();
  });
  return { session, doc };
}

async function main() {
  const rows = [];
  const log = (name, unit, r) => {
    rows.push({ name, unit, median: r.median, min: r.min, max: r.max });
    console.log(
      `${name.padEnd(34)} median ${r.median.toFixed(3).padStart(10)} ${unit}` +
        `   (min ${r.min.toFixed(3)} / max ${r.max.toFixed(3)})`,
    );
  };

  const w = CONFIG.widthPx;
  const h = CONFIG.heightPx;
  const chunksX = w / CHUNK_PX;
  const chunksY = h / CHUNK_PX;

  console.log(`\n[pixeldraw] JS engine baseline — ${w}x${h}, ${CONFIG.layers} layers`);
  console.log(`node ${process.version}\n`);

  // ---------------- 1. pen stroke ----------------
  for (const n of CONFIG.penSizes) {
    const session = new Session();
    const doc = session.newDocument({ widthPx: w, heightPx: h });
    const writer = newPixelWriter(doc.layers[0]);
    const packed = RED;
    const r = timeIt(() => {
      for (let k = 0; k < 20; k++) {
        strokeSegment(writer, 10, (k * 13) % h, w - 10, (k * 29) % h, n, packed);
      }
    }, 5);
    log(`stroke pen=${n} (20 segs)`, "ms", r);
    if (writer.takeDirty().length === 0) throw new Error("stroke produced no dirty chunks");
  }

  // single click dot
  {
    const session = new Session();
    const doc = session.newDocument({ widthPx: w, heightPx: h });
    const writer = newPixelWriter(doc.layers[0]);
    const r = timeIt(() => strokePoint(writer, 512, 512, 64, RED), 5);
    log("strokePoint pen=64 (1 dot)", "ms", r);
  }

  // ---------------- 2. flood fill ----------------
  // The walk mutates its input, so each sample gets a FRESH writer: re-timing an
  // already-filled canvas would measure the early-out instead of the algorithm.
  for (const label of ["empty canvas", "canvas already RED"]) {
    const seedFor = label === "empty canvas" ? 0 : RED;
    const r = timeFresh(
      () => {
        const session = new Session();
        const doc = session.newDocument({ widthPx: w, heightPx: h });
        const writer = newPixelWriter(doc.layers[0]);
        if (label !== "empty canvas") {
          for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) writer.set(x, y, RED);
          writer.takeDirty();
        } else {
          writer.takeDirty();
        }
        return writer;
      },
      (writer) => {
        floodFillScanline(
          (x, y, p) => writer.set(x, y, p),
          (x, y) => writer.get(x, y),
          w,
          h,
          0,
          0,
          seedFor,
          packRGBA(0, 255, 0, 255),
        );
      },
      CONFIG.fillRuns,
    );
    log(`floodFill ${label}`, "ms", r);
  }

  // ---------------- 3. shape raster ----------------
  {
    const session = new Session();
    const doc = session.newDocument({ widthPx: w, heightPx: h });
    const writer = newPixelWriter(doc.layers[0]);
    const shapes = [
      { label: "ellipse 1024x1024 fill", spec: { kind: "ellipse", bbox: [0, 0, 1024, 1024], fillMode: "fill" } },
      { label: "rrect 1024x1024 r=64 outline sw=4", spec: { kind: "rrect", bbox: [0, 0, 1024, 1024], radius: 64, strokeWidth: 4 } },
      { label: "line 2048x1088 sw=8", spec: { kind: "line", p0: [0, 0], p1: [2047, 1079], strokeWidth: 8 } },
      {
        label: "polygon 32-gon 1000px fill",
        spec: {
          kind: "polygon",
          fillMode: "fill",
          points: Array.from({ length: 32 }, (_, i) => {
            const a = (i / 32) * Math.PI * 2;
            return [512 + Math.round(Math.cos(a) * 500), 512 + Math.round(Math.sin(a) * 500)];
          }),
        },
      },
    ];
    for (const { label, spec } of shapes) {
      const r = timeIt(() => applyShape(writer, spec, RED, BLUE), 5);
      log(`applyShape ${label}`, "ms", r);
    }
  }

  // ---------------- 4. composite every chunk ----------------
  {
    const { doc } = makeBusyDoc();
    const out = new Uint8ClampedArray(CHUNK_PX * CHUNK_PX * 4);
    const r = timeIt(() => {
      for (let cy = 0; cy < chunksY; cy++) {
        for (let cx = 0; cx < chunksX; cx++) compositeChunk(doc, cx, cy, out);
      }
    }, 5);
    log(`composite all ${chunksX * chunksY} chunks x ${CONFIG.layers} layers`, "ms", r);
  }

  // ---------------- 5. flatten + PNG ----------------
  {
    const { doc } = makeBusyDoc();
    const r = timeIt(() => flattenToRgba(doc, { includeBackground: true }), 5);
    log("flattenToRgba 2048x1088", "ms", r);
    const rgba = flattenToRgba(doc, { includeBackground: true });
    const r2 = await timeItAsync(() => exportPngBytes(doc, { includeBackground: true }), 3);
    log("exportPngBytes 2048x1088", "ms", r2);
    if (rgba.length !== w * h * 4) throw new Error("flatten size wrong");
  }

  // ---------------- 6. save/load JSON ----------------
  {
    const { doc } = makeBusyDoc();
    const r = await timeItAsync(() => documentToJson(doc), 3);
    log("documentToJson (save)", "ms", r);
    const text = JSON.stringify(await documentToJson(doc));
    console.log(`${"  -> json size".padEnd(34)} ${(text.length / 1024).toFixed(0).padStart(10)} KB`);
    if (process.argv.includes("--emit")) {
      writeFileSync("build/bench_baseline.json", JSON.stringify(rows, null, 2));
      console.log("\nwrote build/bench_baseline.json");
    }
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
