// <META - FILE SUMMARY - Session event subscriptions for the color panel>
import { EVENTS } from "../core/events.js";
import { normHex } from "./panel_color_fields.js";

const PRIMARY = "primaryColor";
const SECONDARY = "secondaryColor";
const PAINT_LABELS = new Set(["연필", "지우개", "페인트통", "직선", "사각형", "둥근 사각형", "타원"]);

// <META - ROLE : Subscribe to session colour/history events; returns dispose | L1-28>
export function mountSessionEvents(ui) {
  const session = ui.session;
  if (!session) return () => {};
  const onSettings = (e) => {
    const k = e.detail?.key;
    if (k !== PRIMARY && k !== SECONDARY) return;
    ui.paint(false);
    if (ui.lock === 0) {
      const v = normHex(e.detail?.value ?? session.settings[k]);
      if (v) ui.pushRecent(v);
    }
  };
  const onHistory = () => {
    let label = "";
    try { label = session.history.undoLabel() ?? ""; } catch { /* ignore */ }
    if (PAINT_LABELS.has(label)) ui.pushRecent(session.settings[PRIMARY]);
  };
  session.addEventListener(EVENTS.SETTINGS_CHANGED, onSettings);
  session.addEventListener(EVENTS.HISTORY_CHANGED, onHistory);
  return () => {
    session.removeEventListener(EVENTS.SETTINGS_CHANGED, onSettings);
    session.removeEventListener(EVENTS.HISTORY_CHANGED, onHistory);
  };
}
