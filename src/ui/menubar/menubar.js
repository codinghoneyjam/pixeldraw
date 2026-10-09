// <META - FILE SUMMARY - Menubar shell: hover + click open, delayed leave close, dispose>

const MENU_CLOSE_DELAY_MS = 220;

// <META - ROLE : Menubar shell: hover + click open, delayed leave close, dispose | L1-60>
export function createMenubar(root) {
  const noop = () => {};
  if (!root || typeof document === "undefined") return { closeAll: noop, dispose: noop };
  const disposers = [];
  const listen = (t, type, fn) => {
    t.addEventListener(type, fn);
    disposers.push(() => t.removeEventListener(type, fn));
  };
  const wraps = [...root.querySelectorAll(".dt-menu")];
  let openWrap = null;
  let openedByHover = false;
  let closeTimer = null;

  function cancelClose() {
    if (closeTimer === null) return;
    clearTimeout(closeTimer);
    closeTimer = null;
  }
  function setOpen(wrap, open) {
    const panel = wrap ? wrap.querySelector("div[role='menu']") : null;
    const trigger = wrap ? wrap.querySelector("button[data-menu]") : null;
    const hidden = open !== true;
    const expanded = open === true ? "true" : "false";
    if (panel && panel.hidden !== hidden) panel.hidden = hidden;
    if (trigger && trigger.getAttribute("aria-expanded") !== expanded) {
      trigger.setAttribute("aria-expanded", expanded);
    }
  }
  function closeAll() {
    cancelClose();
    for (const w of wraps) setOpen(w, false);
    openWrap = null;
    openedByHover = false;
  }
  function open(wrap, source) {
    cancelClose();
    if (openWrap === wrap) return;
    closeAll();
    openWrap = wrap;
    openedByHover = source === "hover";
    setOpen(wrap, true);
  }
  function scheduleClose() {
    cancelClose();
    closeTimer = setTimeout(() => {
      closeTimer = null;
      closeAll();
    }, MENU_CLOSE_DELAY_MS);
  }
  for (const wrap of wraps) {
    const trigger = wrap.querySelector("button[data-menu]");
    listen(wrap, "pointerenter", () => open(wrap, "hover"));
    listen(wrap, "pointerleave", scheduleClose);
    if (trigger) {
      listen(trigger, "click", () => {
        if (openWrap === wrap && openedByHover) openedByHover = false;
        else if (openWrap === wrap) closeAll();
        else open(wrap, "click");
      });
    }
  }
  listen(document, "pointerdown", (e) => {
    if (!root.contains(e.target)) closeAll();
  });
  listen(document, "keydown", (e) => {
    if (e.key === "Escape") closeAll();
  });
  return {
    closeAll,
    dispose() {
      closeAll();
      for (const d of disposers) {
        try { d(); } catch { /* ignore */ }
      }
      disposers.length = 0;
    },
  };
}
