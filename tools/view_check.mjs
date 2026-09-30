// Asserts the worked examples documented in docs/render.md against reference/view_ref.mjs.
import assert from "node:assert/strict";
import * as V from "../reference/view_ref.mjs";
const v = V.createView({ zoom: 4, offsetX: 100, offsetY: 50 });
assert.deepEqual(V.screenToCanvas(v, 140, 90), { x: 10, y: 10 });
assert.deepEqual(V.pixelAt(v, 143.9, 93.9), { x: 10, y: 10 });
assert.deepEqual(V.pixelAt(v, 99, 49), { x: -1, y: -1 });                       // left/above canvas -> negative, never clamped
assert.deepEqual(V.canvasToScreen(v, 10, 10), { x: 140, y: 90 });
assert.deepEqual(V.zoomAt(V.createView({ zoom: 2 }), 100, 100, 4), { zoom: 4, offsetX: -100, offsetY: -100 });
const z = V.zoomAt(v, 300, 200, 8); assert.deepEqual(V.screenToCanvas(z, 300, 200), V.screenToCanvas(v, 300, 200)); // anchor stays fixed
assert.equal(V.stepZoom(1, 1), 2); assert.equal(V.stepZoom(32, 1), 32); assert.equal(V.stepZoom(0.25, -1), 0.25);
assert.equal(V.stepZoom(5, 1), 6); assert.equal(V.stepZoom(5, -1), 4);
assert.deepEqual(V.fitView(512, 512, 800, 600), { zoom: 1, offsetX: 144, offsetY: 44 });
assert.deepEqual(V.fitView(1920, 1088, 1200, 700), { zoom: 0.5, offsetX: 120, offsetY: 78 });
assert.deepEqual(V.clampView(V.createView({ offsetX: 5000, offsetY: -5000 }), 512, 512, 800, 600), { zoom: 1, offsetX: 768, offsetY: -480 });
assert.deepEqual(V.visibleChunkRange(V.createView({ zoom: 1 }), 512, 512, 800, 600), { cx0: 0, cy0: 0, cx1: 15, cy1: 15 });
assert.deepEqual(V.visibleChunkRange(V.createView({ zoom: 4, offsetX: -64, offsetY: 0 }), 512, 512, 256, 256), { cx0: 0, cy0: 0, cx1: 2, cy1: 1 });
assert.equal(V.visibleChunkRange(V.createView({ zoom: 1, offsetX: 900 }), 512, 512, 800, 600), null);
console.log("view worked examples OK");
