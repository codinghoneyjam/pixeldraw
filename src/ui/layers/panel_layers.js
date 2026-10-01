// <META - FILE SUMMARY - Layer panel: opacity, top-first listbox, thumbs, reorder>
import { EVENTS } from "../../core/events.js";
import { DrawToolError } from "../../core/errors.js";
import { unpackRGBA } from "../../core/pixel.js";

const THUMB = 32;
const THUMB_THROTTLE_MS = 250;

// <META - ROLE : Nearest-neighbor layer thumbnail | L11-40>
export function paintThumb(canvas, layer) {
  if (!canvas || typeof canvas.getContext !== "function" || !layer || !layer.store) return false;
  const doc = layer.store;
  const w = doc.widthPx;
  const h = doc.heightPx;
  if (!Number.isInteger(w) || !Number.isInteger(h) || w <= 0 || h <= 0) return false;
  canvas.width = THUMB;
  canvas.height = THUMB;
  const ctx = canvas.getContext("2d");
  if (!ctx) return false;
  const img = ctx.createImageData(THUMB, THUMB);
  const d = img.data;
  for (let ty = 0; ty < THUMB; ty++) {
    const sy = Math.min(h - 1, Math.floor((ty * h) / THUMB));
    for (let tx = 0; tx < THUMB; tx++) {
      const sx = Math.min(w - 1, Math.floor((tx * w) / THUMB));
      const [r, g, b, a] = unpackRGBA(doc.getPixel(sx, sy));
      const o = (ty * THUMB + tx) * 4;
      d[o] = r;
      d[o + 1] = g;
      d[o + 2] = b;
      d[o + 3] = a;
    }
  }
  ctx.putImageData(img, 0, 0);
  return true;
}

// <META - ROLE : Mount layer panel, return dispose | L42-280>
export function mountLayers(root, deps = {}) {
  if (!root || typeof document === "undefined") return () => {};
  const { session } = deps;
  const disposers = [];
  const listen = (t, type, fn) => {
    t.addEventListener(type, fn);
    disposers.push(() => t.removeEventListener(type, fn));
  };
  const q = (sel) => root.querySelector(sel);
  const opacity = q("#dt-layer-opacity");
  const opacityNum = q("#dt-layer-opacity-number");
  const list = q("#dt-layer-list");
  const btnAdd = q("#dt-layer-add");
  const btnDup = q("#dt-layer-duplicate");
  const btnRemove = q("#dt-layer-remove");
  const btnMerge = q("#dt-layer-merge");
  const btnUp = q("#dt-layer-up");
  const btnDown = q("#dt-layer-down");
  let dragging = false;
  const thumbTimers = new Map();
  const thumbEls = new Map();

  function safe(fn) {
    try {
      fn();
    } catch (e) {
      if (e instanceof DrawToolError) session.notify("error", e.message, e.code);
      else console.error(e);
    }
  }
  function doc() {
    return session.doc;
  }
  function activeLayer() {
    const d = doc();
    if (!d) return null;
    try {
      return d.getLayer(d.activeLayerId);
    } catch {
      return null;
    }
  }
  function syncOpacity() {
    const layer = activeLayer();
    const pct = layer ? Math.round(layer.opacity * 100) : 100;
    if (opacity && !dragging) opacity.value = String(pct);
    if (opacityNum && document.activeElement !== opacityNum) opacityNum.value = String(pct);
    if (opacity) opacity.disabled = !layer;
    if (opacityNum) opacityNum.disabled = !layer;
  }
  function syncButtons() {
    const d = doc();
    const n = d ? d.layers.length : 0;
    const idx = d ? d.indexOf(d.activeLayerId) : -1;
    if (btnRemove) btnRemove.disabled = n <= 1;
    if (btnMerge) btnMerge.disabled = !(d && idx > 0);
    if (btnUp) btnUp.disabled = !(d && idx >= 0 && idx < n - 1);
    if (btnDown) btnDown.disabled = !(d && idx > 0);
    const none = !d || idx < 0;
    if (btnDup) btnDup.disabled = none;
    if (btnAdd) btnAdd.disabled = !d;
  }
  function scheduleThumb(layerId) {
    if (thumbTimers.has(layerId)) return;
    const id = setTimeout(() => {
      thumbTimers.delete(layerId);
      const cv = thumbEls.get(layerId);
      const d = doc();
      const layer = d ? d.findLayer(layerId) : null;
      if (cv && layer) paintThumb(cv, layer);
    }, THUMB_THROTTLE_MS);
    thumbTimers.set(layerId, id);
  }
  function startRename(li, layer) {
    const span = li.querySelector(".dt-layer-name");
    if (!span || li.querySelector("input.dt-rename")) return;
    const input = document.createElement("input");
    input.type = "text";
    input.className = "dt-rename";
    input.value = layer.name;
    input.setAttribute("aria-label", "레이어 이름");
    span.replaceWith(input);
    input.focus();
    input.select();
    let done = false;
    const finish = (commit) => {
      if (done) return;
      done = true;
      if (commit) {
        safe(() => {
          const v = input.value.trim();
          if (v.length > 0) session.renameLayer(layer.id, v);
        });
      }
      renderList();
    };
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") { e.preventDefault(); finish(true); }
      else if (e.key === "Escape") { e.preventDefault(); finish(false); }
      e.stopPropagation();
    });
    input.addEventListener("blur", () => finish(false));
  }
  function renderList() {
    thumbEls.clear();
    if (!list) return;
    list.replaceChildren();
    const d = doc();
    if (!d) { syncButtons(); return; }
    const topFirst = d.layers.slice().reverse();
    for (const layer of topFirst) {
      const li = document.createElement("li");
      li.className = "dt-layer";
      li.dataset.layerId = layer.id;
      li.setAttribute("role", "option");
      li.setAttribute("aria-selected", layer.id === d.activeLayerId ? "true" : "false");
      if (layer.locked) li.classList.add("is-locked");
      if (!layer.visible) li.classList.add("is-hidden");
      const eye = document.createElement("button");
      eye.type = "button";
      eye.className = "dt-layer-eye";
      eye.textContent = layer.visible ? "👁" : "🚫";
      eye.setAttribute("aria-label", layer.visible ? "숨기기" : "표시");
      eye.setAttribute("aria-pressed", layer.visible ? "true" : "false");
      eye.addEventListener("click", (e) => {
        e.stopPropagation();
        safe(() => session.setLayerVisible(layer.id, !layer.visible));
      });
      const thumb = document.createElement("canvas");
      thumb.className = "dt-layer-thumb";
      thumb.width = THUMB;
      thumb.height = THUMB;
      thumb.setAttribute("aria-hidden", "true");
      paintThumb(thumb, layer);
      thumbEls.set(layer.id, thumb);
      const name = document.createElement("span");
      name.className = "dt-layer-name";
      name.textContent = layer.name;
      name.title = "더블클릭으로 이름 변경";
      name.addEventListener("dblclick", (e) => {
        e.stopPropagation();
        startRename(li, layer);
      });
      const lock = document.createElement("button");
      lock.type = "button";
      lock.className = "dt-layer-lock";
      lock.textContent = layer.locked ? "🔒" : "🔓";
      lock.setAttribute("aria-label", layer.locked ? "잠금 해제" : "잠금");
      lock.setAttribute("aria-pressed", layer.locked ? "true" : "false");
      lock.addEventListener("click", (e) => {
        e.stopPropagation();
        safe(() => session.setLayerLocked(layer.id, !layer.locked));
      });
      li.append(eye, thumb, name, lock);
      li.addEventListener("click", () => safe(() => session.setActiveLayer(layer.id)));
      list.append(li);
    }
    syncButtons();
  }
  function refresh() {
    renderList();
    syncOpacity();
  }
  if (opacity) {
    opacity.addEventListener("input", () => safe(() => {
      const layer = activeLayer();
      if (!layer) return;
      session.setLayerOpacity(layer.id, Number(opacity.value) / 100, { final: false });
    }));
    opacity.addEventListener("change", () => {
      dragging = false;
      safe(() => {
        const layer = activeLayer();
        if (!layer) return;
        session.setLayerOpacity(layer.id, Number(opacity.value) / 100, { final: true });
      });
    });
    opacity.addEventListener("pointerdown", () => { dragging = true; });
    opacity.addEventListener("pointerup", () => { dragging = false; });
  }
  if (opacityNum) {
    opacityNum.addEventListener("change", () => safe(() => {
      const layer = activeLayer();
      if (!layer) return;
      const n = Number(opacityNum.value);
      if (!Number.isFinite(n)) { syncOpacity(); return; }
      session.setLayerOpacity(layer.id, Math.max(0, Math.min(100, n)) / 100, { final: true });
    }));
  }
  if (btnAdd) btnAdd.addEventListener("click", () => safe(() => session.addLayer()));
  if (btnDup) btnDup.addEventListener("click", () => safe(() => {
    const l = activeLayer();
    if (l) session.duplicateLayer(l.id);
  }));
  if (btnRemove) btnRemove.addEventListener("click", () => safe(() => {
    const l = activeLayer();
    if (l) session.removeLayer(l.id);
  }));
  if (btnMerge) btnMerge.addEventListener("click", () => safe(() => {
    const l = activeLayer();
    if (l) session.mergeDown(l.id);
  }));
  if (btnUp) btnUp.addEventListener("click", () => safe(() => {
    const d = doc();
    const l = activeLayer();
    if (d && l) session.moveLayer(l.id, d.indexOf(l.id) + 1);
  }));
  if (btnDown) btnDown.addEventListener("click", () => safe(() => {
    const d = doc();
    const l = activeLayer();
    if (d && l) session.moveLayer(l.id, d.indexOf(l.id) - 1);
  }));
  if (session) {
    listen(session, EVENTS.DOCUMENT_REPLACED, refresh);
    listen(session, EVENTS.LAYERS_CHANGED, () => { renderList(); syncOpacity(); });
    listen(session, EVENTS.HISTORY_CHANGED, () => { renderList(); syncOpacity(); });
    listen(session, EVENTS.PIXELS_CHANGED, (e) => {
      const id = e.detail?.layerId;
      if (typeof id === "string") scheduleThumb(id);
    });
  }
  refresh();
  return () => {
    for (const id of thumbTimers.values()) clearTimeout(id);
    thumbTimers.clear();
    for (const d of disposers) {
      try { d(); } catch { /* ignore */ }
    }
  };
}
