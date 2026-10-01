// <META - FILE SUMMARY - Recent color grid: build, paint, push>
import { makeSwatchButton } from "./color-brush/panel_color_fields.js";
import { normHex } from "./color-brush/panel_color_fields.js";

const EMPTY_LABEL = "빈 칸";

// <META - ROLE : Build the fixed N-cell recent grid once; it is never rebuilt | L1-22>
export function buildRecentGrid(ui, offs) {
  const host = ui.els.recent;
  if (!host) return [];
  const cells = [];
  for (let i = 0; i < ui.recentCount; i++) {
    const cell = makeSwatchButton({
      hex: null,
      aria: "최근 색",
      emptyLabel: EMPTY_LABEL,
      onPick: (e) => {
        if (cell.disabled) return;
        ui.pickSwatch(e, cell.dataset.color);
      },
    }, offs);
    host.append(cell);
    cells.push(cell);
  }
  return cells;
}

// <META - ROLE : Repaint N recent cells in place; unused slots stay placeholders | L24-42>
export function paintRecent(ui) {
  const list = ui.getRecent().slice(0, ui.recentCount);
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

// <META - ROLE : Record a colour at the front of recent, deduped and capped | L44-51>
export function pushRecent(ui, hex) {
  const v = normHex(hex);
  if (!v) return;
  const list = [v, ...ui.getRecent().filter((c) => c !== v)].slice(0, ui.recentCount);
  ui.saveRecent(list);
  paintRecent(ui);
}
