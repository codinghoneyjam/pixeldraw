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
  const fillToggle = q("#dt-shape-fill-toggle");
  const radius = q("#dt-shape-radius");
  const lock = q("#dt-shape-lock");
  const snap = q("#dt-snap-unit");
  const boxInputs = { x: q("#dt-shape-x"), y: q("#dt-shape-y"), w: q("#dt-shape-w"), h: q("#dt-shape-h") };
  const vertexHost = q("#dt-polygon-vertices");
  const vertexAdd = q("#dt-polygon-add");
  const commitBtn = q("#dt-shape-commit");
  const cancelBtn = q("#dt-shape-cancel");
  if (!fillToggle && !radius && !lock && !snap && !commitBtn) return null;

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
  let vertexRows = [];
  let vertexOffs = [];

  function clearVertexRows() {
    for (const off of vertexOffs) off();
    vertexOffs = [];
    vertexRows = [];
    vertexHost?.replaceChildren();
  }

  function writePoints(points) {
    const tool = shapeTool();
    if (!tool || typeof tool.setPending !== "function") return;
    tool.setPending({ points });
  }

  function buildVertexRows(points) {
    clearVertexRows();
    if (!vertexHost || typeof document === "undefined") return;
    for (let i = 0; i < points.length; i++) {
      const row = document.createElement("div");
      row.className = "dt-polygon-vertex";
      const number = document.createElement("span");
      number.textContent = String(i + 1);
      const x = document.createElement("input");
      x.type = "number";
      x.setAttribute("aria-label", `정점 ${i + 1} X`);
      const y = document.createElement("input");
      y.type = "number";
      y.setAttribute("aria-label", `정점 ${i + 1} Y`);
      const remove = document.createElement("button");
      remove.type = "button";
      remove.textContent = "−";
      remove.setAttribute("aria-label", `정점 ${i + 1} 삭제`);
      const change = () => {
        const next = vertexRows.map((entry) => ({ x: Number(entry.x.value), y: Number(entry.y.value) }));
        if (!next.every((point) => Number.isInteger(point.x) && Number.isInteger(point.y))) {
          fill(pendingOf());
          return;
        }
        safe(() => writePoints(next));
      };
      const drop = () => {
        const current = vertexRows.map((entry) => ({ x: Number(entry.x.value), y: Number(entry.y.value) }));
        if (current.length <= 3) return;
        current.splice(i, 1);
        safe(() => writePoints(current));
      };
      for (const input of [x, y]) {
        input.addEventListener("change", change);
        vertexOffs.push(() => input.removeEventListener("change", change));
      }
      remove.addEventListener("click", drop);
      vertexOffs.push(() => remove.removeEventListener("click", drop));
      row.append(number, x, y, remove);
      vertexHost.append(row);
      vertexRows.push({ x, y, remove });
    }
  }

  function paintVertices(points, editable) {
    if (!vertexHost) return;
    vertexHost.hidden = !Array.isArray(points) || points.length === 0;
    if (vertexAdd) vertexAdd.hidden = !Array.isArray(points) || points.length === 0;
    if (!Array.isArray(points) || points.length === 0) {
      clearVertexRows();
      return;
    }
    if (vertexRows.length !== points.length) buildVertexRows(points);
    for (let i = 0; i < points.length; i++) {
      const row = vertexRows[i];
      row.x.disabled = !editable;
      row.y.disabled = !editable;
      row.remove.disabled = !editable || points.length <= 3;
      if (document.activeElement !== row.x) row.x.value = String(points[i].x);
      if (document.activeElement !== row.y) row.y.value = String(points[i].y);
    }
    if (vertexAdd) vertexAdd.disabled = !editable;
  }

  // <META - ROLE : Enable/disable from the active tool and its pending state | L41-57>
  function sync() {
    const id = activeToolId();
    const isLine = id === "line";
    const isShape = SHAPE_TOOLS.includes(id);
    if (fillToggle) fillToggle.disabled = !isShape || isLine;
    if (radius) radius.disabled = id !== "rrect";
    const pending = pendingOf();
    // Polygon pending is vertex-based, so the bbox number fields do not apply.
    const numericOff = !pending || id === "polygon";
    for (const k of Object.keys(boxInputs)) {
      if (boxInputs[k]) boxInputs[k].disabled = numericOff;
    }
    if (commitBtn) commitBtn.disabled = !pending;
    if (cancelBtn) cancelBtn.disabled = !pending;
    const polyTool = id === "polygon" ? shapeTool() : null;
    paintVertices(polyTool?.getVertices?.() ?? null, !!pending && !polyTool?.isPlacing?.());
  }

  // <META - ROLE : Write the pending geometry into the bbox fields | L59-82>
  function fill(pending) {
    const polyTool = activeToolId() === "polygon" ? shapeTool() : null;
    const points = Array.isArray(pending?.points) ? pending.points : polyTool?.getVertices?.();
    if (Array.isArray(points)) {
      clearBox();
      paintVertices(points, !!pending && !polyTool?.isPlacing?.());
      return;
    }
    paintVertices(null, false);
    if (!pending || typeof pending !== "object") return clearBox();
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
    if (fillToggle) {
      const filled = s.shapeFill === "fill";
      fillToggle.textContent = filled ? "채움" : "테두리";
      fillToggle.setAttribute("aria-pressed", filled ? "true" : "false");
      fillToggle.setAttribute("aria-label", `도형 채움: ${filled ? "채움" : "테두리"}`);
    }
    if (radius && document.activeElement !== radius) radius.value = String(s.shapeRadius);
    if (lock) lock.checked = s.shapeLockAspect === true;
    if (snap) snap.checked = s.snapUnit === true;
  }

  if (fillToggle) fillToggle.addEventListener("click", () => safe(() => {
    session.setSetting("shapeFill", session.settings.shapeFill === "fill" ? "outline" : "fill");
  }));
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
  if (vertexAdd) vertexAdd.addEventListener("click", () => safe(() => {
    const tool = shapeTool();
    const points = tool?.getVertices?.();
    if (!Array.isArray(points) || !tool.hasPending()) return;
    const last = points[points.length - 1];
    const first = points[0];
    points.push({ x: Math.round((last.x + first.x) / 2), y: Math.round((last.y + first.y) / 2) });
    writePoints(points);
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
