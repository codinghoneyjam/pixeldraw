// <META - FILE SUMMARY - Menubar shell + history buttons + action click delegation>
import { createMenubar } from "./menubar.js";
import { createHistoryButtons } from "./history_buttons.js";

// <META - ROLE : Mount menubar chrome and delegate button clicks to actions | L5-20>
export function wireMenubar(menubar, { session, actions }) {
  const menuShell = createMenubar(menubar);
  const disposeHistoryButtons = createHistoryButtons(menubar, session);
  for (const item of menubar.querySelectorAll("button[data-action]")) {
    item.addEventListener("click", () => {
      if (item.disabled) return;
      menuShell.closeAll();
      const fn = actions[item.dataset.action];
      if (typeof fn === "function") fn();
    });
  }
  return () => { menuShell.dispose(); disposeHistoryButtons(); };
}
