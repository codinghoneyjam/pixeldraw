// <META - FILE SUMMARY - Palette grid: mount swatches and sync active markers>
import { makeSwatchButton, normHex } from "./panel_color_fields.js";
export { DEFAULT_PALETTE } from "./color_palette_data.js";

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
