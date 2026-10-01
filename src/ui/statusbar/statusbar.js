// <META - FILE SUMMARY - Status bar: message timers, pixel/grid cursor, zoom, canvas, tool, dirty>
import { CHUNK_PX, TILE_PX } from "../../core/constants.js";
import { EVENTS } from "../../core/events.js";
import { STRINGS } from "../strings.js";

const INFO_MS = 4000;
const WARN_MS = 8000;

// <META - ROLE : Placeholder shown when the pointer is off-canvas | L9-9>
export const OUTSIDE_POS = "- , -";

// <META - ROLE : Pure integer pixel + 32/64px grid readout, both from one px pair | L13-24>
export function formatGridCoords(x, y) {
  const px = Math.floor(x);
  const py = Math.floor(y);
  const cx = Math.floor(px / CHUNK_PX);
  const cy = Math.floor(py / CHUNK_PX);
  const tx = Math.floor(px / TILE_PX);
  const ty = Math.floor(py / TILE_PX);
  return {
    px: `${px}, ${py} px`,
    cells: `셀 ${cx}, ${cy} (${CHUNK_PX}px) · 타일 ${tx}, ${ty} (${TILE_PX}px)`,
  };
}

// <META - ROLE : Pure null-safe dispatcher; off-canvas yields the dash placeholder | L27-30>
export function formatPosition(pos) {
  if (!pos || !Number.isFinite(pos.x) || !Number.isFinite(pos.y)) {
    return { px: OUTSIDE_POS, cells: OUTSIDE_POS };
  }
  return formatGridCoords(pos.x, pos.y);
}

// <META - ROLE : Transient message sink: 4s/8s timers, sticky error level | L35-53>
function createMessageSink(el) {
  let timer = null;
  const clear = () => {
    if (timer === null) return;
    clearTimeout(timer);
    timer = null;
  };
  const show = (level, text) => {
    if (!el) return;
    el.textContent = text;
    el.dataset.level = level;
    clear();
    if (level === "error") return;
    timer = setTimeout(() => {
      timer = null;
      if (el.dataset.level === level) {
        el.textContent = "";
        el.dataset.level = "";
      }
    }, level === "warn" ? WARN_MS : INFO_MS);
  };
  return { show, clear };
}

// <META - ROLE : Pointer readout; off-canvas falls back to the dash placeholder | L60-67>
function createCursorSink(refs) {
  return (pos) => {
    if (!refs.cursor || !refs.tile) return;
    const out = formatPosition(pos);
    refs.cursor.textContent = out.px;
    refs.tile.textContent = out.cells;
  };
}

// <META - ROLE : Query the seven status spans the bar binds | L70-81>
function queryRefs(root) {
  const q = (sel) => root.querySelector(sel);
  return {
    message: q("#dt-status-message"),
    cursor: q("#dt-status-cursor"),
    tile: q("#dt-status-tile"),
    zoom: q("#dt-status-zoom"),
    canvas: q("#dt-status-canvas"),
    tool: q("#dt-status-tool"),
    dirty: q("#dt-status-dirty"),
  };
}

// <META - ROLE : Repaint persistent default info: zoom, canvas size, tool, dirty dot | L84-96>
function createInfoSink(refs, deps) {
  const { session, view } = deps;
  return () => {
    const d = session ? session.doc : null;
    const v = view ? view.get() : null;
    if (refs.zoom) refs.zoom.textContent = v ? `${Math.round(v.zoom * 100)}%` : "";
    if (refs.canvas) {
      if (!d) refs.canvas.textContent = "";
      else {
        const w = d.canvas.widthPx;
        const h = d.canvas.heightPx;
        refs.canvas.textContent = `${w} × ${h} px (${w / TILE_PX} × ${h / TILE_PX} 타일)`;
      }
    }
    if (refs.tool) {
      let id = "";
      try { id = session.settings.activeTool; } catch { /* ignore */ }
      refs.tool.textContent = STRINGS.tools[id] ?? id;
    }
    if (refs.dirty) {
      let dirty = false;
      try { dirty = session.history.isDirty(); } catch { /* ignore */ }
      refs.dirty.textContent = dirty ? STRINGS.status.dirty : "";
    }
  };
}

// <META - ROLE : Mount status bar; dispose carries setCursor/refresh and kills the timer | L112-154>
export function mountStatus(root, deps = {}) {
  const noop = () => {};
  const disposers = [];
  let clearTimer = noop;
  const dispose = () => {
    for (const d of disposers) {
      try { d(); } catch { /* ignore */ }
    }
    clearTimer();
  };
  dispose.setCursor = noop;
  dispose.refresh = noop;
  if (!root || typeof document === "undefined") return dispose;
  const { session, view = null } = deps;
  const listen = (t, type, fn) => {
    t.addEventListener(type, fn);
    disposers.push(() => t.removeEventListener(type, fn));
  };
  const refs = queryRefs(root);
  const messages = createMessageSink(refs.message);
  const setCursor = createCursorSink(refs);
  const refresh = createInfoSink(refs, { session, view });
  clearTimer = messages.clear;
  dispose.setCursor = setCursor;
  dispose.refresh = refresh;
  if (session) {
    listen(session, EVENTS.STATUS_MESSAGE, (e) => {
      const { level = "info", text = "" } = e.detail ?? {};
      messages.show(level, String(text));
    });
    listen(session, EVENTS.DOCUMENT_REPLACED, refresh);
    listen(session, EVENTS.HISTORY_CHANGED, refresh);
    listen(session, EVENTS.LAYERS_CHANGED, refresh);
    listen(session, EVENTS.SETTINGS_CHANGED, (e) => {
      if (e.detail?.key === "activeTool") refresh();
    });
  }
  if (view && typeof view.subscribe === "function") {
    disposers.push(view.subscribe(refresh));
  }
  // Seed the position fields. At mount the pointer is by definition not over the
  // canvas, so the bar must already show the `- , -` placeholder instead of two
  // blank spans that only fill in once the pointer first enters the canvas.
  setCursor(null);
  refresh();
  return dispose;
}
