// Autosave round-trip: the IndexedDB meta record must carry every layer property
// the Layer constructor takes. Before this fix `blend` was neither written nor
// read, so a multiply layer silently came back as normal after a reload.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { buildMetaRecord } from "../src/io/idb_record_builder.js";
import { isMetaValid } from "../src/io/idb_loader.js";
import { Layer } from "../src/features/layers/layer.js";
import { BLEND_MODES } from "../src/core/blend.js";

const CHUNK = 32;

function makeLayers(specs) {
  return specs.map(([id, blend]) => {
    const l = Layer.create({ id, name: id, widthPx: CHUNK, heightPx: CHUNK });
    l.blend = blend;
    return l;
  });
}

describe("autosave meta carries the blend mode", () => {
  it("every mode survives buildMetaRecord", () => {
    for (const mode of BLEND_MODES) {
      const [layer] = makeLayers([["L0", mode]]);
      const meta = buildMetaRecord({ id: "d", name: "n", canvas: { widthPx: CHUNK, heightPx: CHUNK, background: "transparent" }, layers: [layer], activeLayerId: "L0" });
      assert.equal(meta.layers[0].blend, mode, `meta lost blend ${mode}`);
      assert.equal(isMetaValid(meta), true, `meta with blend ${mode} must validate`);
    }
  });

  it("meta carries id/name/visible/locked/opacity/blend, matching the Layer arity", () => {
    const [layer] = makeLayers([["L0", "screen"]]);
    const meta = buildMetaRecord({ id: "d", name: "n", canvas: { widthPx: CHUNK, heightPx: CHUNK, background: "transparent" }, layers: [layer], activeLayerId: "L0" });
    assert.deepEqual(
      Object.keys(meta.layers[0]).sort(),
      ["blend", "id", "locked", "name", "opacity", "visible"],
      "the meta layer entry must mirror every Layer field that is not pixel data",
    );
  });

  it("a legacy meta without blend still validates and restores as normal", () => {
    // Rows written before this fix have no `blend` key. They must stay loadable
    // rather than being discarded, and default to normal.
    const legacy = {
      key: "current", schema: 2, documentId: "d", name: "n",
      canvas: { widthPx: CHUNK, heightPx: CHUNK, background: "transparent" },
      layers: [{ id: "L0", name: "L0", visible: true, locked: false, opacity: 1 }],
      activeLayerId: "L0", updatedAt: 0,
    };
    assert.equal(isMetaValid(legacy), true, "a pre-blend meta row must still load");
    assert.equal(legacy.layers[0].blend, undefined, "and it carries no blend");
  });
});
