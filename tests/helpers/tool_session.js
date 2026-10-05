// <META - FILE SUMMARY - Shared session fixtures and cursor assertion for the tool test files>

import assert from "node:assert/strict";
import { Session } from "../../src/model/session.js";

// Cursors are inline SVG data URLs with a CSS keyword fallback
// (`url("data:image/svg+xml,...") <hx> <hy>, <fallback>`). Docs treat SVG cursors
// as normative, so assert the fallback keyword and URL shape rather than the
// exact payload, which would break on any visual tweak.
// <META - ROLE : trailing CSS keyword of an SVG-data-URL cursor | L9-12>
export function cursorFallback(cursor) {
  assert.match(cursor, /^url\("data:image\/svg\+xml,/, "cursor must be an SVG data URL");
  return cursor.slice(cursor.lastIndexOf(",") + 1).trim();
}

// <META - ROLE : fresh single-layer session | L15-18>
export function makeSession(w = 64, h = 64) {
  const s = new Session();
  s.newDocument({ widthPx: w, heightPx: h });
  return s;
}

// <META - ROLE : tool env with a fixed identity view | L21-23>
export function envFor(session) {
  return { session, getView: () => ({ zoom: 1, offsetX: 0, offsetY: 0 }), requestRender: () => {} };
}

// <META - ROLE : JSON fingerprint of every layer chunk | L26-33>
export function snapshotDoc(doc) {
  const out = [];
  for (const layer of doc.layers) {
    const chunks = [];
    layer.store.forEachChunk((cx, cy, data) => chunks.push([cx, cy, [...data]]));
    out.push([layer.id, chunks]);
  }
  return JSON.stringify(out);
}

// <META - ROLE : unwind the whole history and report the entry count | L36-42>
export function undoCount(session) {
  let n = 0;
  while (session.history.canUndo()) {
    session.undo();
    n++;
  }
  return n;
}