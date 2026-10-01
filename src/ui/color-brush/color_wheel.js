// <META - FILE SUMMARY - Hue ring + SV square wheel mount>
import { createColorWheel } from "./panel_color_wheel.js";

// <META - ROLE : Hue ring + SV square; the wheel is a view fed only by paint() | L1-12>
export function mountWheel(ui) {
  ui.wheel = createColorWheel({
    container: ui.els.wheelWrap,
    onChange: (hex) => ui.safe(() => ui.applyColor(ui.slot, hex, { record: false })),
    onCommit: () => ui.safe(() => ui.pushRecent(ui.canonical())),
  });
  return () => ui.wheel?.dispose();
}
