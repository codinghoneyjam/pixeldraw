// <META - FILE SUMMARY - Brush panel: pen size inputs, wheel stepping, preset grid, footprint preview>
import { brushFootprint } from "../core/brush.js";
import { PEN_MAX, PEN_MIN } from "../core/constants.js";
import { EVENTS } from "../core/events.js";
import { DrawToolError } from "../core/errors.js";
import { parseHex } from "../core/pixel.js";

export const SIZE_PRESETS = Object.freeze([1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64]);

// <META - ROLE : Grid contract constants (CSS hook names, column defaults) | L10-13>
export const PRESET_COLUMNS = 4;
const PRESET_MAX_COLUMNS = 8;
const GRID_CLASS = "dt-preset-grid";
const CELL_CLASS = "dt-preset";

// <META - ROLE : Derive grid column count from preset count (4-column default) | L16-23>
export function presetColumns(count) {
  if (!Number.isInteger(count) || count <= 0) return PRESET_COLUMNS;
  const limit = Math.min(PRESET_MAX_COLUMNS, count);
  for (let c = PRESET_COLUMNS; c <= limit; c++) {
    if (count % c === 0) return c;
  }
  return PRESET_COLUMNS;
}

// <META - ROLE : Derive row count, always >= 1, never drops trailing buttons | L26-30>
export function presetRows(count, columns = PRESET_COLUMNS) {
  const cols = columns > 0 ? columns : PRESET_COLUMNS;
  if (!Number.isInteger(count) || count <= 0) return 1;
  return Math.max(1, Math.ceil(count / cols));
}

// <META - ROLE : Paint 1:1 footprint into 64x64 preview | L33-55>
export function paintPreview(canvas, size, hex) {
  if (!canvas || typeof canvas.getContext !== "function") return false;
  const ctx = canvas.getContext("2d");
  if (!ctx) return false;
  const c = parseHex(hex ?? "#000000") ?? { r: 0, g: 0, b: 0 };
  ctx.clearRect(0, 0, 64, 64);
  let fp = null;
  try {
    fp = brushFootprint(size);
  } catch {
    return false;
  }
  ctx.fillStyle = `rgb(${c.r},${c.g},${c.b})`;
  const ox = Math.floor((64 - size) / 2);
  const oy = Math.floor((64 - size) / 2);
  for (let by = 0; by < size; by++) {
    for (let bx = 0; bx < size; bx++) {
      if (fp.mask[by * size + bx] !== 1) continue;
      ctx.fillRect(ox + bx, oy + by, 1, 1);
    }
  }
  return true;
}

// <META - ROLE : Build one preset button (class, dataset, label, click) | L58-67>
function createPresetButton(size, onPick) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = CELL_CLASS;
  b.dataset.size = String(size);
  b.textContent = String(size);
  b.setAttribute("aria-label", `굵기 ${size}px`);
  b.addEventListener("click", () => onPick(size));
  return b;
}

// <META - ROLE : Emit presets row-major into container + publish grid sizing | L70-88>
function renderPresets(container, onPick) {
  if (!container || typeof container.replaceChildren !== "function") return null;
  const count = SIZE_PRESETS.length;
  const cols = presetColumns(count);
  const rows = presetRows(count, cols);
  if (container.classList && typeof container.classList.add === "function") container.classList.add(GRID_CLASS);
  if (container.dataset) {
    container.dataset.presetCols = String(cols);
    container.dataset.presetRows = String(rows);
  }
  const st = container.style;
  if (st && typeof st.setProperty === "function") {
    st.setProperty("--dt-preset-cols", String(cols));
    st.setProperty("--dt-preset-rows", String(rows));
  }
  container.replaceChildren();
  for (const s of SIZE_PRESETS) container.append(createPresetButton(s, onPick));
  return { cols, rows };
}

// <META - ROLE : Wheel -> next pen size, null when the gesture is ignored | L92-103>
export function stepPenSize(current, deltaY, shiftKey) {
  const dy = Number(deltaY);
  if (!Number.isFinite(dy) || dy === 0) return null;
  const raw = Number(current);
  const base = Number.isFinite(raw) ? Math.round(raw) : PEN_MIN;
  const start = Math.min(PEN_MAX, Math.max(PEN_MIN, base));
  const step = shiftKey === true ? 5 : 1;
  return Math.min(PEN_MAX, Math.max(PEN_MIN, start + (dy < 0 ? step : -step)));
}

// <META - ROLE : Non-passive wheel binding on number + range inputs | L105-121>
function bindWheelInputs(nodes, getCurrent, sync, setSize, disposers) {
  const targets = [nodes.num, nodes.range].filter((t) => t && typeof t.addEventListener === "function");
  for (const t of targets) {
    const onWheel = (ev) => {
      if (!ev || ev.ctrlKey === true) return;
      const next = stepPenSize(getCurrent(), ev.deltaY, ev.shiftKey);
      if (next === null) return;
      if (typeof ev.preventDefault === "function") ev.preventDefault();
      if (setSize(next) !== true) sync();
    };
    t.addEventListener("wheel", onWheel, { passive: false });
    disposers.push(() => t.removeEventListener("wheel", onWheel));
  }
}

// <META - ROLE : Wire number field, range slider and preset grid | L120-133>
function bindSizeInputs(nodes, sync, setSize, disposers) {
  const { num, range, presets } = nodes;
  if (num) {
    const onChange = () => {
      const n = Number(num.value);
      if (!Number.isInteger(n) || n < PEN_MIN || n > PEN_MAX) { sync(); return; }
      setSize(n);
    };
    num.addEventListener("change", onChange);
    disposers.push(() => num.removeEventListener("change", onChange));
  }
  if (range) {
    const onInput = () => setSize(Number(range.value));
    range.addEventListener("input", onInput);
    disposers.push(() => range.removeEventListener("input", onInput));
  }
  renderPresets(presets, setSize);
}

// <META - ROLE : Mount brush panel (inputs, wheel, presets), return dispose | L139-190>
export function mountBrush(root, deps = {}) {
  if (!root || typeof document === "undefined") return () => {};
  const { session } = deps;
  const disposers = [];
  const listen = (t, type, fn) => {
    t.addEventListener(type, fn);
    disposers.push(() => t.removeEventListener(type, fn));
  };
  const q = (sel) => root.querySelector(sel);
  const num = q("#dt-pen-size");
  const range = q("#dt-pen-size-range");
  const presets = q("#dt-size-presets");
  const preview = q("#dt-brush-preview");

  function sync(from) {
    const v = session.settings.penSize;
    if (num && from !== num) num.value = String(v);
    if (range && from !== range) range.value = String(v);
    if (presets) {
      for (const b of presets.querySelectorAll("button[data-size]")) {
        b.setAttribute("aria-pressed", Number(b.dataset.size) === v ? "true" : "false");
      }
    }
    if (preview) paintPreview(preview, v, session.settings.primaryColor);
  }
  function setSize(v) {
    try {
      return session.setSetting("penSize", v);
    } catch (e) {
      if (e instanceof DrawToolError) session.notify("error", e.message, e.code);
      else console.error(e);
      sync();
      return false;
    }
  }
  bindSizeInputs({ num, range, presets }, sync, setSize, disposers);
  bindWheelInputs({ num, range }, () => session.settings.penSize, sync, setSize, disposers);
  if (session) {
    listen(session, EVENTS.SETTINGS_CHANGED, (e) => {
      const k = e.detail?.key;
      if (k === "penSize" || k === "primaryColor") sync();
    });
  }
  sync();
  return () => {
    for (const d of disposers) {
      try { d(); } catch { /* ignore */ }
    }
  };
}