// <META - FILE SUMMARY - Toolbar tool buttons: icons, activeTool sync, click dispatch>
import { EVENTS } from "../../core/events.js";
import { iconFor } from "./icons.js";

// <META - ROLE : Mount toolbar buttons and keep aria-pressed in sync | L5-22>
export function wireToolbar(toolbar, { session, runAction }) {
  function syncToolbar() {
    let active = "pen";
    try { active = session.settings.activeTool; } catch { /* ignore */ }
    for (const b of toolbar.querySelectorAll("button[data-tool]")) {
      b.setAttribute("aria-pressed", b.dataset.tool === active ? "true" : "false");
    }
  }
  for (const b of toolbar.querySelectorAll("button[data-tool]")) {
    const id = b.dataset.tool;
    if (!b.innerHTML.trim()) b.innerHTML = iconFor(id);
    b.addEventListener("click", () => runAction(() => session.setSetting("activeTool", id)));
  }
  session.addEventListener(EVENTS.SETTINGS_CHANGED, (e) => {
    if (e.detail?.key === "activeTool") syncToolbar();
  });
  syncToolbar();
}
