// <META - FILE SUMMARY - FG/BG swap + reset slot buttons>

const PRIMARY = "primaryColor";
const SECONDARY = "secondaryColor";

// <META - ROLE : FG/BG swap + reset; returns dispose | L1-26>
export function mountSlotButtons(ui) {
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
    ui.applyColor(PRIMARY, s[SECONDARY]);
    ui.applyColor(SECONDARY, s[PRIMARY]);
  };
  bind(ui.els.fgbg, swap);
  bind(ui.els.fgbg, swap, "keydown");
  bind(ui.els.resetBtn, () => {
    ui.applyColor(PRIMARY, "#000000");
    ui.applyColor(SECONDARY, "#ffffff");
  });
  return () => {
    for (const off of offs) off();
  };
}
