// <META - FILE SUMMARY - Palette data: default 32 colours, swatch mount, active markers>
import { makeSwatchButton, normHex } from "./panel_color_fields.js";

export const DEFAULT_PALETTE = Object.freeze([
  "#000000", "#1d2b53", "#7e2553", "#008751", "#ab5236", "#5f574f", "#c2c3c7", "#fff1e8",
  "#ff004d", "#ffa300", "#ffec27", "#00e436", "#29adff", "#83769c", "#ff77a8", "#ffccaa",
  "#ffffff", "#e0e0e0", "#c0c0c0", "#a0a0a0", "#808080", "#606060", "#404040", "#202020",
  "#4cc2ff", "#123a52", "#8affff", "#ff8800", "#a4ff5c", "#b04cff", "#ff4c9a", "#3a2a1a",
]);

const PRIMARY = "primaryColor";
const SECONDARY = "secondaryColor";

// <META - ROLE : Fill the palette grid once; right-click loads the secondary slot | L1-18>
export function mountPalette(ui, palette, offs) {
  const host = ui.els.palette;
  if (!host) return;
  const cells = palette.map((hex) => makeSwatchButton({
    hex,
    aria: "팔레트",
    onPick: (e) => ui.pickSwatch(e, hex),
    onContextMenu: (e) => {
      e.preventDefault();
      ui.safe(() => ui.applyColor(SECONDARY, hex, { record: true }));
    },
  }, offs));
  host.replaceChildren(...cells);
}

// <META - ROLE : Mark the palette cell holding a colour currently loaded in either slot | L20-31>
export function syncActiveSwatches(ui) {
  const host = ui.els.palette;
  if (!host || !ui.session) return;
  const s = ui.session.settings;
  const active = new Set([normHex(s[PRIMARY]), normHex(s[SECONDARY])]);
  for (const cell of host.querySelectorAll(".dt-swatch[data-color]")) {
    if (active.has(normHex(cell.dataset.color))) cell.setAttribute("data-active", "true");
    else cell.removeAttribute("data-active");
  }
}
