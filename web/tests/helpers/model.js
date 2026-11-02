// <META - FILE SUMMARY - Shared assertions and document fixtures for the model test files>

import assert from "node:assert/strict";
import { PixelWriter } from "../../src/core/chunkstore.js";
import { PaintCommand } from "../../src/features/layers/commands.js";

// <META - ROLE : assert a DrawToolError with an exact code | L8-17>
export function assertDrawError(fn, code) {
  try {
    fn();
  } catch (err) {
    assert.equal(err.name, "DrawToolError", `expected DrawToolError, got ${err}`);
    assert.equal(err.code, code, `expected code ${code}, got ${err.code}`);
    return;
  }
  assert.fail(`expected DrawToolError(${code})`);
}

// <META - ROLE : deep copy of every populated chunk, for byte-restore assertions | L19-22>
export function snapshotStore(store) {
  const out = [];
  store.forEachChunk((cx, cy, data) => out.push([cx, cy, [...data]]));
  return out;
}

// <META - ROLE : one-pixel PaintCommand against a layer | L24-31>
export function paintPixel(doc, layerId, x, y, packed) {
  const layer = doc.getLayer(layerId);
  const writer = new PixelWriter(layer.store, layerId);
  writer.set(x, y, packed);
  const cs = writer.finish();
  assert.ok(cs, "expected non-null changeset");
  return new PaintCommand(cs, "연필");
}

// <META - ROLE : session event recorder with an explicit teardown | L33-41>
export function collectEvents(session, names) {
  const log = [];
  const offs = names.map((n) => {
    const fn = (e) => log.push({ type: n, detail: e.detail });
    session.addEventListener(n, fn);
    return () => session.removeEventListener(n, fn);
  });
  return { log, done: () => offs.forEach((f) => f()) };
}