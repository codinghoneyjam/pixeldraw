import { visibleChunkRange } from "./view.js";
import { compositeChunk } from "./composite.js";
import { CHUNK_PX, chunkKey, chunkCoords } from "../core/constants.js";

const FRAME_BUDGET_MS = 6;

function nowMs() {
  if (typeof performance !== "undefined" && typeof performance.now === "function") return performance.now();
  return Date.now();
}

export function ensureComp(r) {
  const doc = r._doc;
  if (!doc) return;
  const w = doc.canvas.widthPx;
  const h = doc.canvas.heightPx;
  if (r._comp && r._compW === w && r._compH === h) return;
  let c = null;
  let ctx = null;
  if (typeof OffscreenCanvas !== "undefined") {
    c = new OffscreenCanvas(w, h);
    ctx = c.getContext("2d");
  }
  if (!ctx && typeof document !== "undefined") {
    c = document.createElement("canvas");
    c.width = w;
    c.height = h;
    ctx = c.getContext("2d");
  }
  if (!ctx) return;
  c.width = w;
  c.height = h;
  r._comp = c;
  r._cctx = ctx;
  r._compW = w;
  r._compH = h;
  r._img = ctx.createImageData(CHUNK_PX, CHUNK_PX);
  r._dirtyAll = true;
}

export function paintDirty(r, doc, view, w, h) {
  const vis = visibleChunkRange(view, w, h, r._cssW, r._cssH);
  const inVis = (cx, cy) => vis !== null
    && cx >= vis.cx0 && cx <= vis.cx1 && cy >= vis.cy0 && cy <= vis.cy1;
  const paint = (key) => {
    const { cx, cy } = chunkCoords(key);
    compositeChunk(doc, cx, cy, r._img.data);
    r._cctx.putImageData(r._img, cx * CHUNK_PX, cy * CHUNK_PX);
    r._dirty.delete(key);
  };
  const keys = [...r._dirty];
  for (const key of keys) {
    const { cx, cy } = chunkCoords(key);
    if (inVis(cx, cy)) paint(key);
  }
  const t0 = nowMs();
  for (const key of keys) {
    if (!r._dirty.has(key)) continue;
    if (nowMs() - t0 > FRAME_BUDGET_MS) break;
    paint(key);
  }
}
