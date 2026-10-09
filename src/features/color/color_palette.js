// <META - FILE SUMMARY - Palette data: default 40 colours, swatch mount, active markers>
import { makeSwatchButton, normHex } from "../../features/color/panel_color_fields.js";

export const DEFAULT_PALETTE = Object.freeze([
  "#ffffff", "#ffcdd2", "#ffe0b2", "#fff9c4", "#c8e6c9", "#b2ebf2", "#bbdefb", "#e1bee7",
  "#dedede", "#ef5350", "#ffb74d", "#fff176", "#81c784", "#4dd0e1", "#64b5f6", "#ba68c8",
  "#9e9e9e", "#e53935", "#fb8c00", "#fdd835", "#43a047", "#00acc1", "#1e88e5", "#8e24aa",
  "#616161", "#b71c1c", "#e65100", "#f9a825", "#1b5e20", "#006064", "#0d47a1", "#4a148c",
  "#000000", "#5f0a15", "#6d2f00", "#6e5b00", "#103516", "#0f3c45", "#08214d", "#230a32",
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
