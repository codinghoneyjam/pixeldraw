// <META - FILE SUMMARY - Option bar: grid/zoom + per-tool sections + shape bbox + collapsibles>
import { ZOOM_LEVELS } from "../core/constants.js";
import { EVENTS } from "../core/events.js";
import { DrawToolError } from "../core/errors.js";
import { createTooltips } from "./tooltip.js";

const SHAPE_TOOLS = ["line", "rect", "rrect", "ellipse"];

// Collapsed panel state reuses the existing settings key/JSON convention
// (`dt.settings.v1`) rather than introducing a parallel storage system, so a
// schema change to either travels in the same place.
const COLLAPSE_STORE_KEY = "dt.settings.v1";
const COLLAPSE_FIELD = "collapsedPanels";

function readJson(key) {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return null;
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function writeJson(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch { /* ignore */ }
}

// <META - ROLE : Load the collapsed-section id map, tolerate a missing/corrupt store | L28-36>
function loadCollapsed() {
  const saved = readJson(COLLAPSE_STORE_KEY);
  const field = saved && typeof saved === "object" ? saved[COLLAPSE_FIELD] : null;
  return field && typeof field === "object" && !Array.isArray(field) ? { ...field } : {};
}

// <META - ROLE : Merge one id into the collapsed map, keeping every other setting | L38-45>
function persistCollapsed(id, collapsed) {
  const saved = readJson(COLLAPSE_STORE_KEY);
  const base = saved && typeof saved === "object" && !Array.isArray(saved) ? saved : {};
  const next = { ...base, [COLLAPSE_FIELD]: { ...loadCollapsed(), [id]: collapsed } };
  writeJson(COLLAPSE_STORE_KEY, next);
}

// <META - ROLE : Apply collapsed state to a section, syncing data attr + aria | L47-54>
function applyCollapsed(section, head, collapsed) {
  section.dataset.collapsed = collapsed ? "true" : "false";
  head.setAttribute("aria-expanded", collapsed ? "false" : "true");
}

// <META - ROLE : Wire collapsible panel sections, persist state, return dispose | L56-105>
// Only sections that already carry a `.dt-panel-head` button participate; the
// colour panel is untouched (its h2/h3 are a separate agent's contract). Hiding is
// pure CSS (`display:none` on `.dt-panel-body`), so nothing unmounts: every panel
// keeps its nodes and listeners, and `onExpand` lets the caller repaint canvases
// that were painted while hidden.
export function mountCollapsibleSections(roots, deps = {}) {
  if (typeof document === "undefined") return () => {};
  const { onExpand = null } = deps;
  const list = Array.isArray(roots) ? roots : [roots];
  const disposers = [];
  const stored = loadCollapsed();

  for (const root of list) {
    if (!root || typeof root.querySelectorAll !== "function") continue;
    for (const section of root.querySelectorAll("section[data-collapsed]")) {
      const head = section.querySelector(".dt-panel-head");
      const body = section.querySelector(".dt-panel-body");
      if (!head || !body) continue;
      const id = section.id || "";
      const onClick = () => {
        const collapsed = section.dataset.collapsed !== "true";
        applyCollapsed(section, head, collapsed);
        if (id) persistCollapsed(id, collapsed);
        if (!collapsed && typeof onExpand === "function") {
          try {
            onExpand(id, section);
          } catch { /* ignore */ }
        }
      };
      head.addEventListener("click", onClick);
      disposers.push(() => head.removeEventListener("click", onClick));
      applyCollapsed(section, head, stored[id] === true);
    }
  }
  return () => {
    for (const d of disposers) {
      try { d(); } catch { /* ignore */ }
    }
    disposers.length = 0;
  };
}

// <META - ROLE : Mount option bar, return dispose | L9-260>
export function mountOptions(root, deps = {}) {
  if (!root || typeof document === "undefined") return () => {};
  const { session, toolManager = null, view = null } = deps;
  const disposers = [];
  const listen = (t, type, fn) => {
    t.addEventListener(type, fn);
    disposers.push(() => t.removeEventListener(type, fn));
  };
  const q = (sel) => root.querySelector(sel);
  const gridSel = q("#dt-grid-mode");
  const zoomSel = q("#dt-zoom-select");
  const zoomFit = q("#dt-zoom-fit");
  const zoomActual = q("#dt-zoom-actual");
  const penSizeLabel = q("#dt-option-pen-size");
  const radios = {
    outline: q("#dt-shape-fill-outline"),
    fill: q("#dt-shape-fill-fill"),
  };
  const radius = q("#dt-shape-radius");
  const lock = q("#dt-shape-lock");
  const snap = q("#dt-snap-unit");
  const boxInputs = { x: q("#dt-shape-x"), y: q("#dt-shape-y"), w: q("#dt-shape-w"), h: q("#dt-shape-h") };
  const commitBtn = q("#dt-shape-commit");
  const cancelBtn = q("#dt-shape-cancel");
  const tips = createTooltips(root);
  disposers.push(() => tips.dispose());

  function safe(fn) {
    try {
      fn();
    } catch (e) {
      if (e instanceof DrawToolError) session.notify("error", e.message, e.code);
      else console.error(e);
    }
  }
  function activeTool() {
    try {
      return session.settings.activeTool;
    } catch {
      return "pen";
    }
  }
  function shapeTool() {
    const id = activeTool();
    if (!SHAPE_TOOLS.includes(id)) return null;
    try {
      return toolManager ? toolManager.get(id) : null;
    } catch {
      return null;
    }
  }
  function syncSections() {
    const id = activeTool();
    for (const sec of root.querySelectorAll("[data-for-tools]")) {
      const tools = String(sec.dataset.forTools || "").split(/\s+/).filter(Boolean);
      const show = tools.includes(id);
      sec.toggleAttribute("hidden", !show);
      if (!show) tips.closeWithin(sec);
    }
    const isLine = id === "line";
    const isShape = SHAPE_TOOLS.includes(id);
    for (const r of Object.values(radios)) {
      if (r) r.disabled = !isShape || isLine;
    }
    if (radius) radius.disabled = id !== "rrect";
    const pending = shapeTool() && typeof shapeTool().hasPending === "function" && shapeTool().hasPending();
    for (const k of Object.keys(boxInputs)) {
      if (boxInputs[k]) boxInputs[k].disabled = !pending;
    }
    if (commitBtn) commitBtn.disabled = !pending;
    if (cancelBtn) cancelBtn.disabled = !pending;
  }
  function syncSettings() {
    const s = session.settings;
    if (gridSel && document.activeElement !== gridSel) gridSel.value = s.gridMode;
    for (const [k, r] of Object.entries(radios)) {
      if (r) r.checked = s.shapeFill === k;
    }
    if (radius && document.activeElement !== radius) radius.value = String(s.shapeRadius);
    if (lock) lock.checked = s.shapeLockAspect === true;
    if (snap) snap.checked = s.snapUnit === true;
    if (penSizeLabel) penSizeLabel.textContent = `${s.penSize}px`;
  }
  function syncZoom() {
    if (!zoomSel || !view) return;
    const z = view.get().zoom;
    const best = ZOOM_LEVELS.reduce((a, b) => (Math.abs(b - z) < Math.abs(a - z) ? b : a), ZOOM_LEVELS[0]);
    zoomSel.value = String(best);
  }
  function fillBox(pending) {
    if (!pending || typeof pending !== "object") {
      for (const k of Object.keys(boxInputs)) {
        if (boxInputs[k]) boxInputs[k].value = "";
      }
      return;
    }
    if ("x0" in pending) {
      if (boxInputs.x) boxInputs.x.value = String(pending.x0);
      if (boxInputs.y) boxInputs.y.value = String(pending.y0);
      if (boxInputs.w) boxInputs.w.value = String(pending.x1);
      if (boxInputs.h) boxInputs.h.value = String(pending.y1);
    } else {
      if (boxInputs.x) boxInputs.x.value = String(pending.x);
      if (boxInputs.y) boxInputs.y.value = String(pending.y);
      if (boxInputs.w) boxInputs.w.value = String(pending.w);
      if (boxInputs.h) boxInputs.h.value = String(pending.h);
      if (radius && document.activeElement !== radius && pending.radius !== undefined) {
        radius.value = String(pending.radius);
      }
    }
  }
  if (zoomSel) {
    zoomSel.replaceChildren();
    for (const z of ZOOM_LEVELS) {
      const o = document.createElement("option");
      o.value = String(z);
      o.textContent = `${Math.round(z * 100)}%`;
      zoomSel.append(o);
    }
    zoomSel.addEventListener("change", () => safe(() => {
      if (view) view.zoomTo(Number(zoomSel.value));
    }));
  }
  if (gridSel) gridSel.addEventListener("change", () => safe(() => session.setSetting("gridMode", gridSel.value)));
  if (zoomFit) zoomFit.addEventListener("click", () => safe(() => view && view.fit()));
  if (zoomActual) zoomActual.addEventListener("click", () => safe(() => view && view.actual()));
  for (const [k, r] of Object.entries(radios)) {
    if (r) r.addEventListener("change", () => safe(() => session.setSetting("shapeFill", k)));
  }
  if (radius) radius.addEventListener("change", () => safe(() => {
    const n = Number(radius.value);
    if (Number.isInteger(n) && n >= 0) session.setSetting("shapeRadius", n);
    else radius.value = String(session.settings.shapeRadius);
  }));
  if (lock) lock.addEventListener("change", () => safe(() => session.setSetting("shapeLockAspect", lock.checked)));
  if (snap) snap.addEventListener("change", () => safe(() => session.setSetting("snapUnit", snap.checked)));
  const readBox = () => {
    const n = (input) => Number(input.value);
    const tool = shapeTool();
    const cur = tool && typeof tool.getPending === "function" ? tool.getPending() : null;
    if (cur && "x0" in cur) {
      return { x0: n(boxInputs.x), y0: n(boxInputs.y), x1: n(boxInputs.w), y1: n(boxInputs.h) };
    }
    return { x: n(boxInputs.x), y: n(boxInputs.y), w: n(boxInputs.w), h: n(boxInputs.h) };
  };
  for (const k of Object.keys(boxInputs)) {
    const input = boxInputs[k];
    if (!input) continue;
    input.addEventListener("change", () => safe(() => {
      const tool = shapeTool();
      if (!tool || typeof tool.setPending !== "function") return;
      const vals = readBox();
      for (const v of Object.values(vals)) {
        if (!Number.isInteger(v)) return;
      }
      tool.setPending(vals);
    }));
  }
  if (commitBtn) commitBtn.addEventListener("click", () => safe(() => {
    const tool = shapeTool();
    if (tool && typeof tool.commit === "function") tool.commit();
  }));
  if (cancelBtn) cancelBtn.addEventListener("click", () => safe(() => {
    const tool = shapeTool();
    if (tool && typeof tool.discardPending === "function") tool.discardPending();
  }));
  if (session) {
    listen(session, EVENTS.SETTINGS_CHANGED, (e) => {
      const k = e.detail?.key;
      if (k === "activeTool") { syncSections(); syncSettings(); }
      else if (k === "penSize" || k === "shapeFill" || k === "shapeRadius" || k === "shapeLockAspect" || k === "snapUnit" || k === "gridMode") syncSettings();
    });
    listen(session, EVENTS.TOOL_STATE, (e) => {
      const current = shapeTool();
      if (e.detail && current && e.detail.tool === current.id) {
        fillBox(e.detail.pending);
        syncSections();
      }
    });
  }
  if (view && typeof view.subscribe === "function") {
    const unsub = view.subscribe(syncZoom);
    disposers.push(unsub);
  }
  syncSections();
  syncSettings();
  syncZoom();
  return () => {
    for (const d of disposers) {
      try { d(); } catch { /* ignore */ }
    }
  };
}
