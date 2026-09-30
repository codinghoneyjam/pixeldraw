// <META - FILE SUMMARY - Color panel: one canonical slot colour, slots, palette, recent grid>
import { EVENTS } from "../core/events.js";
import { DrawToolError } from "../core/errors.js";
import { parseHex } from "../core/pixel.js";
import { createColorWheel } from "./panel_color_wheel.js";
import {
  isFocused,
  makeSwatchButton,
  mountColorFields,
  normHex,
  paintColorFields,
  resolveColorFields,
} from "./panel_color_fields.js";

export const DEFAULT_PALETTE = Object.freeze([
  "#000000", "#1d2b53", "#7e2553", "#008751", "#ab5236", "#5f574f", "#c2c3c7", "#fff1e8",
  "#ff004d", "#ffa300", "#ffec27", "#00e436", "#29adff", "#83769c", "#ff77a8", "#ffccaa",
  "#ffffff", "#e0e0e0", "#c0c0c0", "#a0a0a0", "#808080", "#606060", "#404040", "#202020",
  "#4cc2ff", "#123a52", "#8affff", "#ff8800", "#a4ff5c", "#b04cff", "#ff4c9a", "#3a2a1a",
]);

const PAINT_LABELS = new Set(["연필", "지우개", "페인트통", "직선", "사각형", "둥근 사각형", "타원"]);
const RECENT_CELLS = DEFAULT_PALETTE.length;
const PRIMARY = "primaryColor";
const SECONDARY = "secondaryColor";
const EMPTY_LABEL = "빈 칸";

// <META - ROLE : Run fn, routing DrawToolError to the session notifier | L28-38>
function makeSafe(session) {
  return function safe(fn) {
    try {
      fn();
    } catch (e) {
      if (e instanceof DrawToolError && session) session.notify("error", e.message, e.code);
      else console.error(e);
    }
  };
}

// <META - ROLE : SSOT READ: the canonical hex of the active slot, always #rrggbb | L40-44>
function canonicalHex(ui) {
  const raw = ui.session ? ui.session.settings[ui.slot] : "#000000";
  return normHex(raw) ?? "#000000";
}

// <META - ROLE : SSOT WRITE: the ONLY path that commits a colour, repaints and records | L46-62>
function applyColor(ui, key, hex, opts = {}) {
  const v = normHex(hex);
  if (!v) {
    paint(ui, true);
    return false;
  }
  ui.lock += 1;
  try {
    if (ui.session && v !== normHex(ui.session.settings[key])) ui.session.setSetting(key, v);
  } finally {
    ui.lock -= 1;
  }
  paint(ui, true);
  if (opts.record) pushRecent(ui, v);
  return true;
}

// <META - ROLE : Repaint every control from the canonical colour; lock suppresses recursion | L64-80>
function paint(ui, force = false) {
  if (ui.lock > 0) return canonicalHex(ui);
  const hex = canonicalHex(ui);
  const { els, wheel, session } = ui;
  for (const [btn, key] of [[els.primary, PRIMARY], [els.secondary, SECONDARY]]) {
    if (!btn) continue;
    btn.style.background = session ? session.settings[key] : hex;
    btn.setAttribute("aria-pressed", key === ui.slot ? "true" : "false");
  }
  // (no native <input type="color"> any more — the hue ring, SV square, HEX and R/G/B
  // are the only colour inputs, so there is nothing to mirror the value into.)
  paintColorFields(ui.fields, hex, force);
  const c = parseHex(hex);
  if (wheel && c) wheel.setFromRgb(c);
  syncActiveSwatches(ui);
  return hex;
}

// <META - ROLE : Switch the active slot; force repaints every field to that slot | L82-87>
function setSlot(ui, name) {
  if (!ui.session || ui.slot === name) return;
  ui.slot = name;
  paint(ui, true);
}

// <META - ROLE : Swatch click: writes the active slot, Shift targets the other slot | L89-95>
function pickSwatch(ui, e, hex) {
  const v = normHex(hex);
  if (!v) return;
  const key = e && e.shiftKey ? (ui.slot === PRIMARY ? SECONDARY : PRIMARY) : ui.slot;
  ui.safe(() => applyColor(ui, key, v, { record: true }));
}

// <META - ROLE : Build the fixed 32-cell recent grid once; it is never rebuilt | L97-116>
function buildRecentGrid(ui, offs) {
  const host = ui.els.recent;
  if (!host) return [];
  const cells = [];
  for (let i = 0; i < RECENT_CELLS; i++) {
    const cell = makeSwatchButton({
      hex: null,
      aria: "최근 색",
      emptyLabel: EMPTY_LABEL,
      onPick: (e) => {
        if (cell.disabled) return;
        pickSwatch(ui, e, cell.dataset.color);
      },
    }, offs);
    host.append(cell);
    cells.push(cell);
  }
  return cells;
}

// <META - ROLE : Repaint the 32 recent cells in place; unused slots stay placeholders | L118-136>
function paintRecent(ui) {
  const list = ui.getRecent().slice(0, RECENT_CELLS);
  for (let i = 0; i < ui.recentCells.length; i++) {
    const cell = ui.recentCells[i];
    const hex = normHex(list[i]) ?? "";
    cell.className = hex ? "dt-swatch" : "dt-swatch is-empty";
    cell.disabled = hex === "";
    if (hex) {
      cell.dataset.color = hex;
      cell.style.background = hex;
    } else {
      delete cell.dataset.color;
      cell.style.removeProperty("background");
    }
    cell.title = hex || EMPTY_LABEL;
    cell.setAttribute("aria-label", hex ? `최근 색 ${hex}` : EMPTY_LABEL);
  }
}

// <META - ROLE : Record a colour at the front of recent, deduped and capped | L138-145>
function pushRecent(ui, hex) {
  const v = normHex(hex);
  if (!v) return;
  const list = [v, ...ui.getRecent().filter((c) => c !== v)].slice(0, RECENT_CELLS);
  ui.saveRecent(list);
  paintRecent(ui);
}

// <META - ROLE : Fill the palette grid once; right-click loads the secondary slot | L147-161>
function mountPalette(ui, offs) {
  const host = ui.els.palette;
  if (!host) return;
  const cells = DEFAULT_PALETTE.map((hex) => makeSwatchButton({
    hex,
    aria: "팔레트",
    onPick: (e) => pickSwatch(ui, e, hex),
    onContextMenu: (e) => {
      e.preventDefault();
      ui.safe(() => applyColor(ui, SECONDARY, hex, { record: true }));
    },
  }, offs));
  host.replaceChildren(...cells);
}

// <META - ROLE : Mark the palette cell holding a colour currently loaded in either slot | L163-175>
// `data-active` is what style.css keys the corner target marker off, so the palette
// shows which entry is already loaded into the primary or secondary slot.
function syncActiveSwatches(ui) {
  const host = ui.els.palette;
  if (!host || !ui.session) return;
  const s = ui.session.settings;
  const active = new Set([normHex(s[PRIMARY]), normHex(s[SECONDARY])]);
  for (const cell of host.querySelectorAll(".dt-swatch[data-color]")) {
    if (active.has(normHex(cell.dataset.color))) cell.setAttribute("data-active", "true");
    else cell.removeAttribute("data-active");
  }
}

// <META - ROLE : Hue ring + SV square; the wheel is a view fed only by paint() | L178-193>
// A drag across the ring fires onChange for EVERY hue it sweeps. Recording from there
// filed every colour the pointer merely passed over — "둘러보는 모든 색이 최근에 등록".
// So onChange commits WITHOUT recording (it is only the live preview) and the recent
// entry is written once, from onCommit, when the gesture actually ends.
function mountWheel(ui) {
  ui.wheel = createColorWheel({
    container: ui.els.wheelWrap,
    onChange: (hex) => ui.safe(() => applyColor(ui, ui.slot, hex, { record: false })),
    onCommit: () => ui.safe(() => pushRecent(ui, canonicalHex(ui))),
  });
  return () => ui.wheel?.dispose();
}

// <META - ROLE : FG/BG swap + reset; returns dispose | L199-228>
// The overlapping FG/BG pair is ONE control: clicking it (or Enter/Space on it) swaps
// foreground and background. Individual swatches no longer select a slot, because only
// the foreground may paint — the background is a buffer, not a drawing colour.
function mountSlotButtons(ui) {
  const offs = [];
  const bind = (node, fn, evt = "click") => {
    if (!node) return;
    const on = (e) => ui.safe(() => fn(e));
    node.addEventListener(evt, on);
    offs.push(() => node.removeEventListener(evt, on));
  };
  const swap = () => {
    if (!ui.session) return;
    const s = ui.session.settings;
    applyColor(ui, PRIMARY, s[SECONDARY]);
    applyColor(ui, SECONDARY, s[PRIMARY]);
  };
  bind(ui.els.fgbg, swap);
  bind(ui.els.fgbg, swap, "keydown");
  bind(ui.els.resetBtn, () => {
    applyColor(ui, PRIMARY, "#000000");
    applyColor(ui, SECONDARY, "#ffffff");
  });
  return () => {
    for (const off of offs) off();
  };
}

// <META - ROLE : Every panel node the colour module drives, by selector | L225-231>
const EL_SELECTORS = {
  primary: "#dt-color-primary", secondary: "#dt-color-secondary",
  fgbg: "#dt-fgbg", resetBtn: "#dt-color-reset",
  palette: "#dt-palette",
  recent: "#dt-recent", wheelWrap: ".dt-wheel-wrap",
};

// <META - ROLE : Subscribe to session colour/history events; returns dispose | L233-252>
function mountSessionEvents(ui) {
  const session = ui.session;
  if (!session) return () => {};
  const onSettings = (e) => {
    const k = e.detail?.key;
    if (k !== PRIMARY && k !== SECONDARY) return;
    paint(ui, false);
    // `ui.lock` is raised only while THIS panel commits through applyColor, and the
    // synchronous SETTINGS_CHANGED dispatch happens inside that window. A lock of 0
    // therefore means the value arrived from OUTSIDE — in practice the eyedropper,
    // which writes session settings directly and so used to bypass the recent grid
    // entirely. Recording here catches those samples without double-recording our
    // own picker picks, and without recording slot swaps / resets.
    if (ui.lock === 0) {
      const v = normHex(e.detail?.value ?? session.settings[k]);
      if (v) pushRecent(ui, v);
    }
  };
  const onHistory = () => {
    let label = "";
    try { label = session.history.undoLabel() ?? ""; } catch { /* ignore */ }
    if (PAINT_LABELS.has(label)) pushRecent(ui, session.settings[PRIMARY]);
  };
  session.addEventListener(EVENTS.SETTINGS_CHANGED, onSettings);
  session.addEventListener(EVENTS.HISTORY_CHANGED, onHistory);
  return () => {
    session.removeEventListener(EVENTS.SETTINGS_CHANGED, onSettings);
    session.removeEventListener(EVENTS.HISTORY_CHANGED, onHistory);
  };
}

// <META - ROLE : Dispose: drop every listener, then remove every node we created | L254-265>
function makeDispose(ui, disposers) {
  return () => {
    for (const d of disposers) {
      try { d(); } catch { /* ignore */ }
    }
    disposers.length = 0;
    for (const cell of ui.recentCells) cell.remove();
    ui.recentCells.length = 0;
    ui.els.palette?.replaceChildren();
  };
}

// <META - ROLE : Mount color panel, return dispose | L267-296>
export function mountColor(root, deps = {}) {
  if (!root || typeof document === "undefined") return () => {};
  const { session = null } = deps;
  const disposers = [];
  const ui = {
    session,
    slot: PRIMARY,
    lock: 0,
    wheel: null,
    fields: resolveColorFields(root),
    recentCells: [],
    getRecent: typeof deps.getRecent === "function" ? deps.getRecent : () => [],
    saveRecent: typeof deps.saveRecent === "function" ? deps.saveRecent : () => {},
    els: Object.fromEntries(
      Object.entries(EL_SELECTORS).map(([k, sel]) => [k, root.querySelector(sel)]),
    ),
    canonical: () => canonicalHex(ui),
    apply: (hex) => applyColor(ui, ui.slot, hex, { record: true }),
  };
  ui.safe = makeSafe(session);
  ui.recentCells = buildRecentGrid(ui, disposers);
  paintRecent(ui);
  mountPalette(ui, disposers);
  disposers.push(mountColorFields(ui, ui.fields));
  disposers.push(mountSlotButtons(ui));
  disposers.push(mountWheel(ui), mountSessionEvents(ui));
  paint(ui, true);
  return makeDispose(ui, disposers);
}