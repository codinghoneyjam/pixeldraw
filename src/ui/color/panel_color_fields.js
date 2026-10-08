// <META - FILE SUMMARY - Picker cells + R/G/B/A + HEX fields: build, read, commit, repaint>
import { parseHex, toHex, toHex8 } from "../../core/pixel.js";

export const CHANNELS = ["r", "g", "b"];

// <META - ROLE : Normalize any hex input to #rrggbb[aa], null when invalid | L6-12>
export function normHex(v) {
  const c = parseHex(v);
  if (!c || c.a === 0) return null;
  return c.a === 255 ? toHex(c.r, c.g, c.b) : toHex8(c.r, c.g, c.b, c.a);
}

// <META - ROLE : True when the element owns the keyboard caret | L13-16>
export function isFocused(el) {
  return typeof document !== "undefined" && document.activeElement === el;
}

// <META - ROLE : True when an input may be overwritten without fighting the user | L18-21>
function writable(input, force) {
  return Boolean(input) && (force || !isFocused(input));
}

// <META - ROLE : Write one input value, honouring the focus guard | L23-26>
function setValue(input, value, force) {
  if (writable(input, force)) input.value = value;
}

// <META - ROLE : Locate the 2x4 grid and its five inputs inside the panel root | L28-37>
export function resolveColorFields(root) {
  if (!root || typeof root.querySelector !== "function") return null;
  const rgb = {};
  for (const ch of CHANNELS) rgb[ch] = root.querySelector(`#dt-color-${ch}`);
  const hex = root.querySelector("#dt-color-hex");
  const alpha = root.querySelector("#dt-color-a");
  if (!hex && !rgb.r) return null;
  return { grid: root.querySelector(".dt-color-fields"), hex, rgb, alpha };
}

// <META - ROLE : Mirror the canonical hex into HEX and the RGBA inputs | L39-49>
export function paintColorFields(fields, hex, force = false) {
  if (!fields) return;
  setValue(fields.hex, hex, force);
  const c = parseHex(hex);
  if (!c) return;
  const vals = { r: c.r, g: c.g, b: c.b };
  for (const ch of CHANNELS) setValue(fields.rgb[ch], String(vals[ch]), force);
  setValue(fields.alpha, String(c.a ?? 255), force);
}

// <META - ROLE : New hex from an RGB cell edit; null when the edit is out of range | L51-64>
// Alpha is preserved: editing R/G/B never resets translucency.
function readRgbEdit(fields, ch, canonical) {
  const input = fields.rgb[ch];
  if (!input) return null;
  const raw = String(input.value ?? "");
  const n = Number(raw);
  if (raw.trim() === "" || !Number.isInteger(n) || n < 0 || n > 255) return null;
  const base = parseHex(canonical) ?? { r: 0, g: 0, b: 0, a: 255 };
  const parts = { r: base.r, g: base.g, b: base.b, a: base.a ?? 255 };
  parts[ch] = n;
  return parts.a === 255 ? toHex(parts.r, parts.g, parts.b) : toHex8(parts.r, parts.g, parts.b, parts.a);
}

// <META - ROLE : New hex from an alpha cell edit; null when out of range | L66-77>
function readAlphaEdit(fields, canonical) {
  const input = fields.alpha;
  if (!input) return null;
  const raw = String(input.value ?? "");
  const n = Number(raw);
  if (raw.trim() === "" || !Number.isInteger(n) || n < 1 || n > 255) return null;
  const base = parseHex(canonical) ?? { r: 0, g: 0, b: 0 };
  return n === 255 ? toHex(base.r, base.g, base.b) : toHex8(base.r, base.g, base.b, n);
}

// <META - ROLE : New hex from a HEX edit; null when the text is not #rgb/#rrggbb/#rrggbbaa | L61-65>
function readHexEdit(fields) {
  if (!fields.hex) return null;
  return normHex(String(fields.hex.value ?? ""));
}

/**
 * Build a `.dt-swatch` button. `hex === null` yields the empty checkerboard cell.
 * `onPick` receives the DOM click event, exactly like a plain `addEventListener`.
 * Every listener is pushed onto `offs` so dispose can remove it.
 * @param {{hex?: string|null, aria: string, emptyLabel?: string,
 *   onPick: (e: any) => void, onContextMenu?: (e: any) => void}} opts
 * @param {Array<() => void>} offs
 */
// <META - ROLE : Create a swatch button and register its listener unsubscribes | L75-98>
export function makeSwatchButton(opts, offs) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "dt-swatch";
  if (opts.hex) {
    b.dataset.color = opts.hex;
    b.style.background = opts.hex;
    b.title = opts.hex;
    b.setAttribute("aria-label", `${opts.aria} ${opts.hex}`);
  } else {
    b.disabled = true;
    b.className = "dt-swatch is-empty";
    b.title = opts.emptyLabel ?? "";
    b.setAttribute("aria-label", opts.emptyLabel ?? "");
  }
  b.addEventListener("click", opts.onPick);
  offs.push(() => b.removeEventListener("click", opts.onPick));
  if (opts.onContextMenu) {
    b.addEventListener("contextmenu", opts.onContextMenu);
    offs.push(() => b.removeEventListener("contextmenu", opts.onContextMenu));
  }
  return b;
}

// <META - ROLE : Single commit path: read -> apply; an invalid read reverts to canonical | L100-106>
function commitField(ui, read) {
  ui.safe(() => {
    const hex = read() ?? ui.canonical();
    ui.apply(hex);
  });
}

// <META - ROLE : Bind commit/revert listeners for HEX and each RGB cell; returns dispose | L108-133>
export function mountColorFields(ui, fields) {
  if (!fields) return () => {};
  const offs = [];
  const bind = (input, read) => {
    if (!input) return;
    const run = () => commitField(ui, () => read(fields, ui.canonical()));
    const onKey = (e) => {
      if (e.key !== "Enter") return;
      e.preventDefault();
      run();
      if (typeof input.blur === "function") input.blur();
    };
    input.addEventListener("change", run);
    input.addEventListener("keydown", onKey);
    offs.push(() => input.removeEventListener("change", run));
    offs.push(() => input.removeEventListener("keydown", onKey));
  };
  bind(fields.hex, (f) => readHexEdit(f));
  for (const ch of CHANNELS) bind(fields.rgb[ch], (f, c) => readRgbEdit(f, ch, c));
  bind(fields.alpha, (f, c) => readAlphaEdit(f, c));
  return () => {
    for (const off of offs) {
      try { off(); } catch { /* ignore */ }
    }
  };
}