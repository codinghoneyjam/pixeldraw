// <META - FILE SUMMARY - Color panel core: mountColor entry point>
import { EVENTS } from "../../core/events.js";
import { DrawToolError } from "../../core/errors.js";
import { parseHex } from "../../core/pixel.js";
import {
  mountColorFields,
  normHex,
  paintColorFields,
  resolveColorFields,
} from "./panel_color_fields.js";
import { buildRecentGrid, paintRecent, pushRecent } from "./color_recent.js";
import { mountPalette, syncActiveSwatches } from "./color_palette.js";
import { mountWheel } from "./panel_color_wheel.js";
import { mountSlotButtons } from "./color_slots.js";
import { mountSessionEvents } from "./color_events.js";

export { DEFAULT_PALETTE } from "./color_palette.js";
import { DEFAULT_PALETTE } from "./color_palette.js";

const PRIMARY = "primaryColor";
const SECONDARY = "secondaryColor";

// <META - ROLE : Run fn, routing DrawToolError to the session notifier | L1-10>
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

// <META - ROLE : SSOT READ: the canonical hex of the active slot, always #rrggbb | L12-15>
function canonicalHex(ui) {
  const raw = ui.session ? ui.session.settings[ui.slot] : "#000000";
  return normHex(raw) ?? "#000000";
}

// <META - ROLE : SSOT WRITE: the ONLY path that commits a colour, repaints and records | L17-32>
function applyColor(ui, key, hex, opts = {}) {
  const v = normHex(hex);
  if (!v) { paint(ui, true); return false; }
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

// <META - ROLE : Repaint every control from the canonical colour | L34-50>
function paint(ui, force = false) {
  if (ui.lock > 0) return canonicalHex(ui);
  const hex = canonicalHex(ui);
  const { els, wheel, session } = ui;
  for (const [btn, key] of [[els.primary, PRIMARY], [els.secondary, SECONDARY]]) {
    if (!btn) continue;
    btn.style.background = session ? session.settings[key] : hex;
    btn.setAttribute("aria-pressed", key === ui.slot ? "true" : "false");
  }
  paintColorFields(ui.fields, hex, force);
  const c = parseHex(hex);
  if (wheel && c) wheel.setFromRgb(c);
  syncActiveSwatches(ui);
  return hex;
}

// <META - ROLE : Switch the active slot | L52-56>
function setSlot(ui, name) {
  if (!ui.session || ui.slot === name) return;
  ui.slot = name;
  paint(ui, true);
}

// <META - ROLE : Swatch click: writes the active slot, Shift targets the other slot | L58-64>
function pickSwatch(ui, e, hex) {
  const v = normHex(hex);
  if (!v) return;
  const key = e && e.shiftKey ? (ui.slot === PRIMARY ? SECONDARY : PRIMARY) : ui.slot;
  ui.safe(() => applyColor(ui, key, v, { record: true }));
}

const EL_SELECTORS = {
  primary: "#dt-color-primary", secondary: "#dt-color-secondary",
  fgbg: "#dt-fgbg", resetBtn: "#dt-color-reset",
  palette: "#dt-palette",
  recent: "#dt-recent", wheelWrap: ".dt-wheel-wrap",
};

// <META - ROLE : Dispose: drop every listener, then remove every node we created | L66-76>
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

// <META - ROLE : Mount color panel, return dispose | L78-115>
export function mountColor(root, deps = {}) {
  if (!root || typeof document === "undefined") return () => {};
  const { session = null } = deps;
  const disposers = [];
  const PALETTE = deps.palette ?? DEFAULT_PALETTE;
  const RECENT_CELLS = PALETTE.length;
  const ui = {
    session,
    slot: PRIMARY,
    lock: 0,
    wheel: null,
    fields: resolveColorFields(root),
    recentCells: [],
    recentCount: RECENT_CELLS,
    getRecent: typeof deps.getRecent === "function" ? deps.getRecent : () => [],
    saveRecent: typeof deps.saveRecent === "function" ? deps.saveRecent : () => {},
    els: Object.fromEntries(
      Object.entries(EL_SELECTORS).map(([k, sel]) => [k, root.querySelector(sel)]),
    ),
    canonical: () => canonicalHex(ui),
    paint: (force) => paint(ui, force),
    applyColor: (key, hex, opts) => applyColor(ui, key, hex, opts),
    apply: (hex) => applyColor(ui, ui.slot, hex, { record: true }),
    pickSwatch: (e, hex) => pickSwatch(ui, e, hex),
    pushRecent: (hex) => pushRecent(ui, hex),
  };
  ui.safe = makeSafe(session);
  ui.recentCells = buildRecentGrid(ui, disposers);
  paintRecent(ui);
  mountPalette(ui, PALETTE, disposers);
  disposers.push(mountColorFields(ui, ui.fields));
  disposers.push(mountSlotButtons(ui));
  disposers.push(mountWheel(ui), mountSessionEvents(ui));
  paint(ui, true);
  return makeDispose(ui, disposers);
}
