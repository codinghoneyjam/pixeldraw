// <META - FILE SUMMARY - Shape tool option section: fill/radius/lock/snap + bbox editing>
// Owns every control inside the option bar's shape block and the rules that
// enable them. The shell only decides which section is visible; the enable
// state depends on the active tool AND on its pending geometry, so it lives here.
import { EVENTS } from "../../core/events.js";

const SHAPE_TOOLS = ["line", "rect", "rrect", "ellipse", "polygon"];
const SHAPE_KEYS = ["shapeFill", "shapeRadius", "shapeLockAspect", "snapUnit"];

// <META - ROLE : Mount the shape controls; returns {sync, fill, dispose} or null | L12-..>
export function mountShapeOptions(root, deps = {}) {
  const { session, toolManager = null, safe = (fn) => fn() } = deps;
  const q = (sel) => root?.querySelector(sel) ?? null;
  const radios = { outline: q("#dt-shape-fill-outline"), fill: q("#dt-shape-fill-fill") };
  const radius = q("#dt-shape-radius");
  const lock = q("#dt-shape-lock");
  const snap = q("#dt-snap-unit");
  const boxInputs = { x: q("#dt-shape-x"), y: q("#dt-shape-y"), w: q("#dt-shape-w"), h: q("#dt-shape-h") };
  const commitBtn = q("#dt-shape-commit");
  const cancelBtn = q("#dt-shape-cancel");
  if (!radius && !lock && !snap && !commitBtn) return null;

  const disposers = [];
  const listen = (t, type, fn) => {
    t.addEventListener(type, fn);
    disposers.push(() => t.removeEventListener(type, fn));
  };
  const activeToolId = () => {
    try { return session.settings.activeTool; } catch { return "pen"; }
  };
  const shapeTool = () => {
    const id = activeToolId();
    if (!SHAPE_TOOLS.includes(id)) return null;
    try { return toolManager ? toolManager.get(id) : null; } catch { return null; }
  };
  // The pending object doubles as the "there is something to commit" flag, so
  // callers can treat it as void: commit/cancel stay disabled without one.
  const pendingOf = () => {
    const tool = shapeTool();
    if (!tool || typeof tool.hasPending !== "function" || !tool.hasPending()) return null;
    return typeof tool.getPending === "function" ? tool.getPending() : null;
  };

  // <META - ROLE : Enable/disable from the active tool and its pending state | L41-57>
  function sync() {
    const id = activeToolId();
    const isLine = id === "line";
    const isShape = SHAPE_TOOLS.includes(id);
    for (const r of Object.values(radios)) {
      if (r) r.disabled = !isShape || isLine;
    }
    if (radius) radius.disabled = id !== "rrect";
    const pending = pendingOf();
    // Polygon pending is vertex-based, so the bbox number fields do not apply.
    const numericOff = !pending || id === "polygon";
    for (const k of Object.keys(boxInputs)) {
      if (boxInputs[k]) boxInputs[k].disabled = numericOff;
    }
    if (commitBtn) commitBtn.disabled = !pending;
    if (cancelBtn) cancelBtn.disabled = !pending;
  }

  // <META - ROLE : Write the pending geometry into the bbox fields | L59-82>
  function fill(pending) {
    if (!pending || typeof pending !== "object") return clearBox();
    if (Array.isArray(pending.points)) return clearBox();
    if ("x0" in pending) {
      if (boxInputs.x) boxInputs.x.value = String(pending.x0);
      if (boxInputs.y) boxInputs.y.value = String(pending.y0);
      if (boxInputs.w) boxInputs.w.value = String(pending.x1);
      if (boxInputs.h) boxInputs.h.value = String(pending.y1);
      return;
    }
    if (boxInputs.x) boxInputs.x.value = String(pending.x);
    if (boxInputs.y) boxInputs.y.value = String(pending.y);
    if (boxInputs.w) boxInputs.w.value = String(pending.w);
    if (boxInputs.h) boxInputs.h.value = String(pending.h);
    if (radius && document.activeElement !== radius && pending.radius !== undefined) {
      radius.value = String(pending.radius);
    }
  }

  function clearBox() {
    for (const k of Object.keys(boxInputs)) {
      if (boxInputs[k]) boxInputs[k].value = "";
    }
  }

  // <META - ROLE : Reflect the shape settings, never stealing a focused field | L84-91>
  function syncValues() {
    const s = session.settings;
    for (const [k, r] of Object.entries(radios)) {
      if (r) r.checked = s.shapeFill === k;
    }
    if (radius && document.activeElement !== radius) radius.value = String(s.shapeRadius);
    if (lock) lock.checked = s.shapeLockAspect === true;
    if (snap) snap.checked = s.snapUnit === true;
  }

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
    const cur = pendingOf();
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
      if (SHAPE_KEYS.includes(e.detail?.key)) syncValues();
    });
  }

  clearBox();
  syncValues();
  return {
    sync,
    fill,
    dispose: () => {
      for (const d of disposers) {
        try { d(); } catch { /* ignore */ }
      }
    },
  };
}
