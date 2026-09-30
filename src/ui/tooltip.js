// <META - FILE SUMMARY - Hover/click tooltip layer for .dt-tip-trigger, body-level node>
/**
 * Floating tooltip controller.
 *
 * - One tooltip at a time, ever.
 * - Shown on hover (mouseover) / focus (focusin) and on click (pinned, for touch).
 * - Hidden on mouseout, focusout, Escape, outside click, and an idle auto-hide timer.
 * - The floating node is appended to `document.body` because the option bar is
 *   `overflow-x: auto; overflow-y: hidden` and would clip anything inside it.
 * - The node uses the class contract `.dt-tooltip`; triggers use `.dt-tip-trigger`.
 *   Position is applied inline as `left`/`top` only; all other styling is CSS-owned.
 *
 * No `document` / `window` access happens at import time.
 */

/** Idle milliseconds a shown tooltip stays up before it auto-hides. */
export const TIP_AUTO_HIDE_MS = 2500;

const TRIGGER_SEL = ".dt-tip-trigger";
const GAP = 6;
const MARGIN = 8;
const MAX_TEXT = 400;

// <META - ROLE : Guard for DOM availability, keeps node import safe | L28-30>
function hasDom() {
  return typeof document !== "undefined" && typeof window !== "undefined";
}

// <META - ROLE : Clamp a box inside the viewport | L33-45>
function clampBox(rect, tip) {
  const vw = window.innerWidth || 0;
  const vh = window.innerHeight || 0;
  let left = rect.left;
  let top = rect.bottom + GAP;
  if (left + tip.offsetWidth + MARGIN > vw) left = vw - tip.offsetWidth - MARGIN;
  if (left < MARGIN) left = MARGIN;
  if (top + tip.offsetHeight + MARGIN > vh) top = rect.top - tip.offsetHeight - GAP;
  if (top < MARGIN) top = Math.max(MARGIN, vh - tip.offsetHeight - MARGIN);
  return { left, top };
}

// <META - ROLE : Nearest tooltip trigger for an event target, null if outside root | L48-55>
function triggerFor(node) {
  if (!node || typeof node.closest !== "function") return null;
  const el = node.closest(TRIGGER_SEL);
  return el && this.root && this.root.contains(el) ? el : null;
}

/**
 * Build a tooltip controller scoped to `root`.
 * @param {HTMLElement|null} root container searched for `.dt-tip-trigger` elements
 * @returns {{dispose: () => void, close: () => void, closeWithin: (el: Element|null) => void, isOpen: () => boolean}}
 */
export function createTooltips(root) {
  const state = {
    root: root || null,
    tip: null,
    current: null,
    pinned: false,
    timer: null,
    disposers: [],
  };
  if (!state.root || !hasDom()) {
    return { dispose: () => {}, close: () => {}, closeWithin: () => {}, isOpen: () => false };
  }

  // <META - ROLE : Create/attach the single floating node under document.body | L69-80>
  function ensureNode() {
    if (state.tip) return state.tip;
    const el = document.createElement("div");
    el.className = "dt-tooltip";
    el.setAttribute("role", "tooltip");
    el.hidden = true;
    document.body.append(el);
    state.tip = el;
    state.disposers.push(() => el.remove());
    return el;
  }

  // <META - ROLE : Clear the pending idle auto-hide timer | L83-87>
  function clearTimer() {
    if (state.timer === null) return;
    clearTimeout(state.timer);
    state.timer = null;
  }

  // <META - ROLE : Start (or restart) the idle auto-hide countdown | L90-95>
  function armTimer() {
    clearTimer();
    state.timer = setTimeout(() => {
      state.timer = null;
      hide();
    }, TIP_AUTO_HIDE_MS);
  }

  // <META - ROLE : Hide the tooltip and reset aria-expanded on the trigger | L98-109>
  function hide() {
    clearTimer();
    if (state.tip) state.tip.hidden = true;
    if (state.current) state.current.setAttribute("aria-expanded", "false");
    state.current = null;
    state.pinned = false;
  }

  // <META - ROLE : Show tooltip for trigger; pinned keeps it past mouseleave | L112-127>
  function show(trigger, pinned) {
    const text = String(trigger.getAttribute("data-tip") || "").trim().slice(0, MAX_TEXT);
    if (!text) return;
    if (state.current && state.current !== trigger) hide();
    const el = ensureNode();
    el.textContent = text;
    el.hidden = false;
    state.current = trigger;
    state.pinned = pinned === true;
    trigger.setAttribute("aria-expanded", "true");
    const box = clampBox(trigger.getBoundingClientRect(), el);
    el.style.left = `${Math.round(box.left)}px`;
    el.style.top = `${Math.round(box.top)}px`;
    armTimer();
  }

  const lookup = (e) => triggerFor.call({ root: state.root }, e.target);

  function onOver(e) {
    const el = lookup(e);
    if (el) show(el, false);
  }
  function onFocusIn(e) {
    const el = lookup(e);
    if (el) show(el, false);
  }
  function onOut(e) {
    const el = lookup(e);
    if (!el || el !== state.current || state.pinned) return;
    const to = e.relatedTarget;
    if (to && el.contains(to)) return;
    hide();
  }
  function onFocusOut(e) {
    const el = lookup(e);
    if (!el || el !== state.current || state.pinned) return;
    const to = e.relatedTarget;
    if (to && el.contains(to)) return;
    hide();
  }
  function onClick(e) {
    const el = lookup(e);
    if (!el) return;
    if (state.current === el && state.pinned) hide();
    else show(el, true);
  }
  function onDocClick(e) {
    if (!state.current) return;
    const t = e.target;
    if (state.current.contains(t) || (state.tip && state.tip.contains(t))) return;
    hide();
  }
  function onKeyDown(e) {
    if (e.key === "Escape") hide();
  }

  const listen = (target, type, fn, opts) => {
    target.addEventListener(type, fn, opts);
    state.disposers.push(() => target.removeEventListener(type, fn, opts));
  };
  listen(state.root, "mouseover", onOver);
  listen(state.root, "focusin", onFocusIn);
  listen(state.root, "mouseout", onOut);
  listen(state.root, "focusout", onFocusOut);
  listen(state.root, "click", onClick);
  listen(document, "click", onDocClick);
  listen(document, "keydown", onKeyDown);

  return {
    // <META - ROLE : Close tooltip, teardown node, drop every listener | L163-170>
    dispose() {
      hide();
      for (const d of state.disposers) {
        try { d(); } catch { /* ignore */ }
      }
      state.disposers = [];
      state.tip = null;
    },
    close: hide,
    // <META - ROLE : Close only when the open trigger lives inside el | L172-176>
    closeWithin(el) {
      if (state.current && el && el.contains(state.current)) hide();
    },
    isOpen() {
      return state.current !== null;
    },
  };
}
