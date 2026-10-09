// Blend-mode semantics + the display/export/eyedropper equivalence invariant.
//
// The equivalence half is the point of this file: three separate compositors read
// core/blend.js - render/composite.js for the screen AND for samplePixel (what the
// eyedropper and pen Alt-pick return), and io/export_png.js for the file. If they
// ever disagree, the user sees one colour on canvas, another in the PNG, and a
// third under the eyedropper. contract.md §1-1 makes that a hard invariant, so it
// is asserted here for every mode.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { BLEND_MODES, over, overBlend, rnd } from "../src/core/blend.js";
import { compositeChunk, samplePixel } from "../src/render/composite.js";
import { flattenToRgba } from "../src/io/export_png.js";
import { PixelWriter } from "../src/core/chunkstore.js";
import { Document, IdGen } from "../src/features/document/document.js";
import { Layer } from "../src/features/layers/layer.js";
import { SetLayerPropCommand } from "../src/features/layers/commands.js";
import { MergeDownCommand } from "../src/features/layers/commands_pixel.js";
import { Session } from "../src/features/document/session.js";

const CHUNK = 32;

// <META - ROLE : deterministic pseudo-random, no dependency | L20-23>
function lcg(seed) {
  let s = seed >>> 0;
  return () => (s = (s * 1664525 + 1013904223) >>> 0) / 4294967296;
}

// <META - ROLE : n layers of random straight-alpha pixels, bottom-first | L25-49>
// Partial alpha is deliberate: it is where a naive blend formula goes wrong, and
// the transparent-backdrop rule (contract §1-1 rule 3) only fires when the whole
// accumulated backdrop is transparent.
function makeLayers(n, seed = 7, w = CHUNK, h = CHUNK) {
  const rand = lcg(seed);
  const layers = [];
  for (let l = 0; l < n; l++) {
    const layer = Layer.create({ id: `L${l}`, name: `L${l}`, widthPx: w, heightPx: h });
    const wr = new PixelWriter(layer.store, layer.id);
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const pick = Math.floor(rand() * 4);
        const a = pick === 0 ? 0 : pick === 1 ? 128 : pick === 2 ? 255 : Math.floor(rand() * 256);
        wr.set(x, y, (Math.floor(rand() * 256) << 24) | (Math.floor(rand() * 256) << 16)
          | (Math.floor(rand() * 256) << 8) | a);
      }
    }
    wr.finish();
    layers.push(layer);
  }
  return layers;
}

// <META - ROLE : document built through the public Document constructor | L51-58>
// newDocument() would fabricate its own single layer, so build the Document
// directly with the exact layer list the test wants.
function makeDoc(layers) {
  return new Document({
    id: "doc-blend",
    name: "blend",
    canvas: { widthPx: CHUNK, heightPx: CHUNK, background: "transparent", viewport: null },
    layers,
    activeLayerId: layers[layers.length - 1].id,
    ids: new IdGen(),
  });
}

describe("blend formulas", () => {
  it("multiply and screen are complementary", () => {
    // screen(a,b) = 255 - multiply(255-a,255-b) exactly, up to the single rnd()
    // each side takes. So allow 1 LSB of slack rather than an exact identity.
    for (const s of [0, 1, 64, 127, 128, 200, 255]) {
      for (const d of [0, 1, 64, 127, 128, 200, 255]) {
        const mul = overBlend([d, d, d, 255], [s, s, s, 255], 1, "multiply")[0];
        const sc = overBlend([255 - d, 255 - d, 255 - d, 255], [255 - s, 255 - s, 255 - s, 255], 1, "screen")[0];
        assert.ok(Math.abs(mul + sc - 255) <= 1, `screen != 255-multiply: ${mul}+${sc} at s=${s} d=${d}`);
      }
    }
  });

  it("darken/lighten pick the extreme channel", () => {
    assert.equal(overBlend([10, 200, 128, 255], [90, 60, 128, 255], 1, "darken")[0], 10);
    assert.equal(overBlend([10, 200, 128, 255], [90, 60, 128, 255], 1, "darken")[1], 60);
    assert.equal(overBlend([10, 200, 128, 255], [90, 60, 128, 255], 1, "lighten")[0], 90);
    assert.equal(overBlend([10, 200, 128, 255], [90, 60, 128, 255], 1, "lighten")[1], 200);
  });

  it("multiply commutes; overlay does not", () => {
    assert.deepEqual(
      overBlend([200, 0, 0, 255], [100, 0, 0, 255], 1, "multiply"),
      overBlend([100, 0, 0, 255], [200, 0, 0, 255], 1, "multiply"),
      "multiply must commute",
    );
    assert.notDeepEqual(
      overBlend([200, 0, 0, 255], [100, 0, 0, 255], 1, "overlay"),
      overBlend([100, 0, 0, 255], [200, 0, 0, 255], 1, "overlay"),
      "overlay is asymmetric",
    );
  });

  it("overlay follows the stated table, seam included", () => {
    // Assert the documented formula verbatim rather than an ideal property.
    // The seam is dc=128: below it the multiply branch runs, at/above it the
    // screen branch, and single-rounding puts a few results 1 LSB off the exact
    // midpoint (e.g. overlay(128,127)=127). That is the formula, not a bug, and
    // pinning the table keeps doc and code coupled.
    const cases = [
      // multiply branch: dc < 128
      [255, 127, 254], [128, 127, 127], [64, 127, 64], [1, 127, 1], [0, 127, 0],
      // screen branch: dc >= 128
      [255, 128, 255], [128, 128, 128], [0, 128, 1],
    ];
    for (const [s, d, want] of cases) {
      assert.equal(overBlend([d, 0, 0, 255], [s, 0, 0, 255], 1, "overlay")[0], want, `overlay(${s},${d})`);
    }
  });

  it("a transparent backdrop is never blended against (rule 3)", () => {
    // Multiplying against black would turn a transparent region black.
    for (const mode of BLEND_MODES) {
      assert.deepEqual(
        overBlend([0, 0, 0, 0], [200, 100, 50, 255], 1, mode),
        [200, 100, 50, 255],
        `${mode} must leave a transparent backdrop alone`,
      );
    }
  });

  it("a zero-alpha source never changes the backdrop", () => {
    for (const mode of BLEND_MODES) {
      assert.deepEqual(
        overBlend([10, 20, 30, 5], [200, 100, 50, 0], 0, mode),
        [10, 20, 30, 5],
        `${mode} must not touch a zero-alpha source`,
      );
    }
  });

  it("normal is byte-identical to over() over a wide sample", () => {
    const rand = lcg(1234);
    for (let i = 0; i < 4000; i++) {
      const dst = Array.from({ length: 4 }, () => Math.floor(rand() * 256));
      const src = Array.from({ length: 4 }, () => Math.floor(rand() * 256));
      const op = Math.floor(rand() * 101) / 100;
      assert.deepEqual(overBlend(dst, src, op, "normal"), over(dst, src, op), `normal diverged at ${i}`);
      assert.deepEqual(overBlend(dst, src, op, "bogus"), over(dst, src, op), "an unknown mode must fall back to normal");
    }
  });

  it("rnd is floor(v+0.5)", () => {
    assert.equal(rnd(0.5), 1);
    assert.equal(rnd(1.5), 2);
    assert.equal(rnd(254.5), 255);
    assert.equal(rnd(0.4), 0);
  });
});

describe("display == export == eyedropper (contract §1-1)", () => {
  for (const mode of BLEND_MODES) {
    it(`all three compositors agree for mode=${mode}`, () => {
      const layers = makeLayers(3, mode.length * 31 + 5);
      layers[1].blend = mode;
      const doc = makeDoc(layers);

      const screen = new Uint8ClampedArray(CHUNK * CHUNK * 4);
      compositeChunk(doc, 0, 0, screen);
      const flat = flattenToRgba(doc, { includeBackground: false });

      assert.equal(flat.length, screen.length, "flatten length");
      for (let i = 0; i < screen.length; i += 4) {
        const s = [screen[i], screen[i + 1], screen[i + 2], screen[i + 3]];
        const f = [flat[i], flat[i + 1], flat[i + 2], flat[i + 3]];
        assert.deepEqual(f, s, `export != screen at byte ${i} for ${mode}`);
        const px = (i / 4) % CHUNK;
        const py = Math.floor(i / 4 / CHUNK);
        assert.deepEqual(samplePixel(doc, px, py), s, `samplePixel != screen at (${px},${py}) for ${mode}`);
      }
    });
  }

  it("a non-normal mode actually changes the composite", () => {
    // Guards the other direction: if multiply ever degenerated to normal, every
    // equivalence test above would still pass.
    const layers = makeLayers(2, 99);
    const doc = makeDoc(layers);
    const a = new Uint8ClampedArray(CHUNK * CHUNK * 4);
    compositeChunk(doc, 0, 0, a);
    layers[1].blend = "multiply";
    const b = new Uint8ClampedArray(CHUNK * CHUNK * 4);
    compositeChunk(doc, 0, 0, b);
    assert.ok(Buffer.compare(Buffer.from(a), Buffer.from(b)) !== 0, "multiply matched normal exactly");
  });

  it("the opaque fast path is skipped for non-normal modes", () => {
    // A fully opaque source over an opaque backdrop must still run the math:
    // normal replaces, multiply must scale by the source.
    const layers = makeLayers(2, 5);
    // Force both layers fully opaque with known colours.
    for (const layer of layers) {
      const wr = new PixelWriter(layer.store, layer.id);
      for (let y = 0; y < CHUNK; y++) {
        for (let x = 0; x < CHUNK; x++) {
          const c = layer.id === "L1" ? 128 : 200;
          wr.set(x, y, (255 << 24) | (c << 16) | (c << 8) | c);
        }
      }
      wr.finish();
    }
    const doc = makeDoc(layers);
    const normal = new Uint8ClampedArray(CHUNK * CHUNK * 4);
    compositeChunk(doc, 0, 0, normal);
    assert.equal(normal[0], 128, "normal replaces the backdrop");

    layers[1].blend = "multiply";
    const mul = new Uint8ClampedArray(CHUNK * CHUNK * 4);
    compositeChunk(doc, 0, 0, mul);
    assert.equal(mul[0], rnd(128 * (200 / 255)), "multiply must run despite the opaque source");

    // And export must agree, or the fast paths have drifted apart.
    const flat = flattenToRgba(doc, { includeBackground: false });
    assert.equal(flat[0], mul[0], "export used a different fast path than the screen");
  });
});

describe("layer blend model", () => {  it("setProp accepts every mode and rejects an unknown one", () => {
    const [layer] = makeLayers(1);
    for (const mode of BLEND_MODES) {
      layer.setProp("blend", mode);
      assert.equal(layer.blend, mode);
    }
    assert.throws(() => layer.setProp("blend", "dissolve"), (e) => e.code === "INVALID_STATE");
  });

  it("blend is a settable prop field, so undo/redo can carry it", () => {
    const layers = makeLayers(1);
    const doc = makeDoc(layers);
    const layer = layers[0];
    const cmd = new SetLayerPropCommand(layer.id, "blend", "normal", "multiply", "레이어 블렌드 변경");
    cmd.do(doc);
    assert.equal(layer.blend, "multiply");
    cmd.undo(doc);
    assert.equal(layer.blend, "normal");
  });

  it("BLEND_MODES is the single list, re-exported by the layer module", () => {
    assert.deepEqual([...BLEND_MODES], ["normal", "multiply", "screen", "overlay", "darken", "lighten"]);
  });
});

describe("merge-down and blend", () => {
  // The UI's actual write path is session.setLayerBlend(), which used to throw
  // ReferenceError because layer_operations.js called validateLayerBlend without
  // importing it. The tests only ever touched layer.setProp() and the command
  // directly, so the gap went unnoticed - the panel's safe() wrapper swallowed
  // it and the dropdown just failed silently. This block pins the entry point
  // the panel really uses.
  it("session.setLayerBlend() writes through to the layer and the composite", () => {
    const session = new Session();
    session.newDocument({ widthPx: CHUNK, heightPx: CHUNK, background: "transparent" });
    const doc = session.doc;
    session.addLayer("upper");
    const [lower, upper] = doc.layers;

    for (const layer of [lower, upper]) {
      const wr = new PixelWriter(layer.store, layer.id);
      for (let y = 0; y < CHUNK; y++) {
        for (let x = 0; x < CHUNK; x++) {
          const c = layer.id === upper.id ? 128 : 200;
          wr.set(x, y, (255 << 24) | (c << 16) | (c << 8) | c);
        }
      }
      wr.finish();
    }

    const read = () => {
      const out = new Uint8ClampedArray(CHUNK * CHUNK * 4);
      compositeChunk(doc, 0, 0, out);
      return out[0];
    };

    const before = read();
    assert.equal(before, 128, "normal replaces");

    session.setLayerBlend(upper.id, "multiply");
    assert.equal(upper.blend, "multiply", "the layer really changed");
    assert.equal(read(), rnd(128 * (200 / 255)), "and the composite followed");

    session.setLayerBlend(upper.id, "screen");
    assert.equal(upper.blend, "screen");
    assert.notEqual(read(), before, "a different mode gives a different result");

    // A rejected value must throw the coded error, not a ReferenceError.
    assert.throws(() => session.setLayerBlend(upper.id, "dissolve"), (e) => e.code === "INVALID_STATE");

    // No-op writes are refused rather than pushed onto the history.
    const steps = session.history.canUndo();
    session.setLayerBlend(upper.id, "screen");
    assert.equal(session.history.canUndo(), steps, "an unchanged blend is not a history entry");
  });
  // Two separate facts, both worth pinning:
  //   1. UNDO IS COMPLETE. do() snapshots every lower chunk, the lower opacity
  //      and the removed upper Layer OBJECT (blend included), so undo restores
  //      both layers exactly - pixels, blend, opacity and stack position.
  //   2. The BLEND RELATION is what ends. After merging, the upper's blended
  //      pixels are ordinary pixels in the lower layer, so re-blending the lower
  //      later re-blends that region too and cannot reproduce the original
  //      two-layer look. Only undoing the merge recovers it.
  it("the merged pixels equal what the blend produced", () => {
    const layers = makeLayers(2, 5);
    for (const layer of layers) {
      const wr = new PixelWriter(layer.store, layer.id);
      for (let y = 0; y < CHUNK; y++) {
        for (let x = 0; x < CHUNK; x++) {
          const c = layer.id === "L1" ? 128 : 200;
          wr.set(x, y, (255 << 24) | (c << 16) | (c << 8) | c);
        }
      }
      wr.finish();
    }
    const doc = makeDoc(layers);

    // What the screen shows BEFORE merging, with the upper in multiply.
    layers[1].blend = "multiply";
    const before = new Uint8ClampedArray(CHUNK * CHUNK * 4);
    compositeChunk(doc, 0, 0, before);
    const expected = rnd(128 * (200 / 255));

    // The command must bake exactly those pixels into the lower layer.
    const cmd = new MergeDownCommand(layers[1].id);
    cmd.do(doc);
    const after = new Uint8ClampedArray(CHUNK * CHUNK * 4);
    compositeChunk(doc, 0, 0, after);
    assert.equal(after[0], expected, "merge must bake the blended result");
    assert.equal(before[0], expected, "pre-merge composite must already be that value");

    // The baked pixels are now plain: flipping the lower's blend away from
    // normal cannot change them, because the relation was already flattened.
    layers[0].blend = "screen";
    const flipped = new Uint8ClampedArray(CHUNK * CHUNK * 4);
    compositeChunk(doc, 0, 0, flipped);
    assert.equal(flipped[0], after[0], "the merge already flattened the relation");
  });

  it("undo restores both layers exactly, blend included", () => {
    const layers = makeLayers(2, 5);
    for (const layer of layers) {
      const wr = new PixelWriter(layer.store, layer.id);
      for (let y = 0; y < CHUNK; y++) {
        for (let x = 0; x < CHUNK; x++) {
          const c = layer.id === "L1" ? 128 : 200;
          wr.set(x, y, (255 << 24) | (c << 16) | (c << 8) | c);
        }
      }
      wr.finish();
    }
    layers[0].opacity = 0.5;
    layers[1].blend = "screen";
    const doc = makeDoc(layers);

    const pixelsBefore = layers.map((l) => Array.from(l.store.getChunk(0, 0) ?? []));
    const screenBefore = new Uint8ClampedArray(CHUNK * CHUNK * 4);
    compositeChunk(doc, 0, 0, screenBefore);

    const cmd = new MergeDownCommand(layers[1].id);
    cmd.do(doc);
    assert.equal(doc.layers.length, 1, "merge removes the upper layer");

    cmd.undo(doc);
    assert.equal(doc.layers.length, 2, "undo puts the upper layer back");

    // Every observable property must be back, not just the pixels.
    assert.equal(doc.layers[0].blend, "normal");
    assert.equal(doc.layers[1].blend, "screen", "upper blend is restored");
    assert.equal(doc.layers[0].opacity, 0.5, "lower opacity is restored");
    // undo restores the state from BEFORE the command, so the active layer is
    // the one that was selected when the merge happened (L1 here), not the lower
    // layer that `do` had switched to.
    assert.equal(doc.activeLayerId, layers[1].id, "pre-merge active layer is restored");
    assert.deepEqual(Array.from(doc.layers[0].store.getChunk(0, 0) ?? []), pixelsBefore[0], "lower pixels");
    assert.deepEqual(Array.from(doc.layers[1].store.getChunk(0, 0) ?? []), pixelsBefore[1], "upper pixels");
    const screenAfter = new Uint8ClampedArray(CHUNK * CHUNK * 4);
    compositeChunk(doc, 0, 0, screenAfter);
    assert.deepEqual([...screenAfter], [...screenBefore], "the composite is byte-identical after undo");
  });
});
