// Basic golden + invariant tests for src/core/shape_raster.js.
// Reads normative vectors directly from docs fixtures (no copies).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { ellipseMask, rrectMask, outlineRing } from "../src/core/raster/raster_masks.js";
import { lineMask } from "../src/core/raster/segment.js";
import { angleSnap, dragBBox, resizeBBox, unitSnap } from "../src/core/raster/raster_snap.js";
import { applyShape } from "../src/core/raster/shape_raster.js";

const GOLDEN_URL = new URL(
  "./fixtures/raster_golden.json",
  import.meta.url,
);
const G = JSON.parse(readFileSync(GOLDEN_URL, "utf-8"));

const rowsOf = (mask) => {
  const rows = [];
  for (let y = 0; y < mask.h; y++) {
    let s = "";
    for (let x = 0; x < mask.w; x++) s += mask.data[y * mask.w + x] === 1 ? "#" : ".";
    rows.push(s);
  }
  return rows;
};

const symmetric = (mask) => {
  for (let y = 0; y < mask.h; y++) {
    for (let x = 0; x < mask.w; x++) {
      const v = mask.data[y * mask.w + x];
      if (mask.data[y * mask.w + (mask.w - 1 - x)] !== v) return false;
      if (mask.data[(mask.h - 1 - y) * mask.w + x] !== v) return false;
    }
  }
  return true;
};

const hasNoEmptyRowCol = (mask) => {
  for (let y = 0; y < mask.h; y++) {
    let any = false;
    for (let x = 0; x < mask.w; x++) {
      if (mask.data[y * mask.w + x] === 1) { any = true; break; }
    }
    if (!any) return false;
  }
  for (let x = 0; x < mask.w; x++) {
    let any = false;
    for (let y = 0; y < mask.h; y++) {
      if (mask.data[y * mask.w + x] === 1) { any = true; break; }
    }
    if (!any) return false;
  }
  return true;
};

// Clipped-canvas fake writer with strict bounds (writer.set must clip).
function makeCanvasWriter(W, H) {
  const grid = Array.from({ length: H }, () => new Array(W).fill(0));
  return {
    grid,
    set(x, y, packed) {
      if (x < 0 || y < 0 || x >= W || y >= H) return;
      grid[y][x] = packed;
    },
  };
}

describe("shape_raster golden vectors", () => {
  it("ellipseMask matches all golden ellipse vectors", () => {
    for (const c of G.ellipse) {
      assert.deepEqual(rowsOf(ellipseMask(c.w, c.h)), c.rows, `ellipse ${c.w}x${c.h}`);
    }
  });

  it("rrectMask matches all golden rrect vectors", () => {
    for (const c of G.rrect) {
      assert.deepEqual(rowsOf(rrectMask(c.w, c.h, c.r)), c.rows, `rrect ${c.w}x${c.h} r${c.r}`);
    }
  });

  it("outlineRing matches all golden outline vectors", () => {
    for (const c of G.outline) {
      assert.deepEqual(
        rowsOf(outlineRing(c.kind, c.w, c.h, c.r, c.n)),
        c.rows,
        `outline ${c.kind} ${c.w}x${c.h} r${c.r} n${c.n}`,
      );
    }
  });

  it("resize/drag/unit/angle vectors match golden", () => {
    for (const c of G.resize) {
      assert.deepEqual(
        resizeBBox(c.bbox, c.handle, c.pointer, c.lock, c.center),
        c.expect,
        `resize ${c.handle} ${c.pointer} lock=${c.lock} center=${c.center}`,
      );
    }
    assert.equal(G.resize.length, 15);
    for (const c of G.drag_bbox) {
      assert.deepEqual(dragBBox(c.p0, c.p1, c.lock, c.center), c.expect, `drag ${c.p0}->${c.p1}`);
    }
    for (const c of G.unit_snap) {
      assert.deepEqual(unitSnap(c.bbox), c.expect, `unit ${c.bbox}`);
    }
    for (const c of G.angle_snap) {
      assert.deepEqual(angleSnap(...c.p0, ...c.p1), c.expect, `angle ${c.p1}`);
    }
  });

  // Checked against G.wide_line (raw PIL d.line), NOT G.stroke: the stroke
  // group is the spec for the editor's round pen (core/brush.js), while
  // lineMask rasterises PIL's rotated-quad wide line. The two must not share a
  // fixture group -- see task memo T-7b.
  it("lineMask reproduces PIL wide_line goldens when clipped", () => {
    for (const c of G.wide_line) {
      const W = c.W;
      const H = c.H;
      const canvas = Array.from({ length: H }, () => new Array(W).fill("."));
      const m = lineMask(c.p0, c.p1, c.width);
      for (let y = 0; y < m.h; y++) {
        for (let x = 0; x < m.w; x++) {
          if (m.data[y * m.w + x] !== 1) continue;
          const cx = m.x + x;
          const cy = m.y + y;
          if (cx >= 0 && cy >= 0 && cx < W && cy < H) canvas[cy][cx] = "#";
        }
      }
      assert.deepEqual(canvas.map((r) => r.join("")), c.rows, `wide_line ${c.name}`);
    }
  });
  // The editor's round pen is verified separately against G.stroke by
  // tests/core_basic.test.mjs (strokePoint/strokeSegment from core/brush.js),
  // so it is not duplicated here.
});

describe("shape_raster invariants", () => {
  it("ellipse masks are symmetric with no empty row/col", () => {
    for (let w = 1; w <= 32; w++) {
      for (let h = 1; h <= 32; h++) {
        const m = ellipseMask(w, h);
        assert.equal(symmetric(m), true, `sym ellipse ${w}x${h}`);
        assert.equal(hasNoEmptyRowCol(m), true, `full ellipse ${w}x${h}`);
      }
    }
  });

  it("rrect masks are symmetric with no empty row/col", () => {
    for (let w = 1; w <= 16; w++) {
      for (let h = 1; h <= 16; h++) {
        for (const r of [0, 1, 2, 5, 99]) {
          const m = rrectMask(w, h, r);
          assert.equal(symmetric(m), true, `sym rrect ${w}x${h} r${r}`);
          assert.equal(hasNoEmptyRowCol(m), true, `full rrect ${w}x${h} r${r}`);
        }
      }
    }
  });

  it("outline inset is within outer; 2n>=min gives solid ring==outer", () => {
    for (const kind of ["ellipse", "rrect"]) {
      for (let w = 1; w <= 16; w++) {
        for (let h = 1; h <= 16; h++) {
          for (const r of [0, 3, 99]) {
            for (let n = 1; n <= 6; n++) {
              const outer = kind === "ellipse" ? ellipseMask(w, h) : rrectMask(w, h, r);
              const ring = outlineRing(kind, w, h, r, n);
              for (let i = 0; i < w * h; i++) {
                assert.ok(ring.data[i] === 0 || outer.data[i] === 1, `inset>outer ${kind} ${w}x${h} r${r} n${n}`);
              }
              if (2 * n >= Math.min(w, h)) {
                assert.deepEqual([...ring.data], [...outer.data], `solid ${kind} ${w}x${h} r${r} n${n}`);
              }
            }
          }
        }
      }
    }
  });
});

describe("applyShape", () => {
  it("fill paints outer with primary; outline keeps interior; both uses secondary inside", () => {
    const P = 7;
    const S = 9;
    const spec = { kind: "rrect", bbox: { x: 1, y: 1, w: 14, h: 9 }, radius: 3, strokeWidth: 2 };
    const outer = rrectMask(14, 9, 3);
    const ring = outlineRing("rrect", 14, 9, 3, 2);

    const wFill = makeCanvasWriter(20, 14);
    applyShape(wFill, { ...spec, fillMode: "fill" }, P, S);
    for (let y = 0; y < 14; y++) {
      for (let x = 0; x < 20; x++) {
        const lx = x - 1;
        const ly = y - 1;
        const want = lx >= 0 && ly >= 0 && lx < 14 && ly < 9 && outer.data[ly * 14 + lx] === 1 ? P : 0;
        assert.equal(wFill.grid[y][x], want, `fill ${x},${y}`);
      }
    }

    const wOut = makeCanvasWriter(20, 14);
    applyShape(wOut, { ...spec, fillMode: "outline" }, P, S);
    for (let y = 0; y < 14; y++) {
      for (let x = 0; x < 20; x++) {
        const lx = x - 1;
        const ly = y - 1;
        const want = lx >= 0 && ly >= 0 && lx < 14 && ly < 9 && ring.data[ly * 14 + lx] === 1 ? P : 0;
        assert.equal(wOut.grid[y][x], want, `outline ${x},${y}`);
      }
    }

    const wBoth = makeCanvasWriter(20, 14);
    applyShape(wBoth, { ...spec, fillMode: "both" }, P, S);
    for (let y = 0; y < 14; y++) {
      for (let x = 0; x < 20; x++) {
        const lx = x - 1;
        const ly = y - 1;
        let want = 0;
        if (lx >= 0 && ly >= 0 && lx < 14 && ly < 9) {
          if (ring.data[ly * 14 + lx] === 1) want = P;
          else if (outer.data[ly * 14 + lx] === 1) want = S;
        }
        assert.equal(wBoth.grid[y][x], want, `both ${x},${y}`);
      }
    }
  });

  it("clips at canvas edges and draws lines with primary", () => {
    const w = makeCanvasWriter(10, 6);
    const count = applyShape(
      w,
      { kind: "line", p0: [-3, 2], p1: [6, 2], strokeWidth: 3 },
      5,
      6,
    );
    assert.ok(count > 0);
    const flat = w.grid.flat();
    assert.ok(flat.every((v) => v === 0 || v === 5));
    assert.ok(flat.some((v) => v === 5));
  });

  it("oversized strokeWidth fills the shape solid", () => {
    const w = makeCanvasWriter(12, 12);
    applyShape(w, { kind: "rect", bbox: { x: 2, y: 2, w: 8, h: 8 }, strokeWidth: 5, fillMode: "outline" }, 4, 0);
    for (let y = 2; y < 10; y++) {
      for (let x = 2; x < 10; x++) assert.equal(w.grid[y][x], 4, `solid ${x},${y}`);
    }
  });
});
