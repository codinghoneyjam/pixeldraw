// <META - FILE SUMMARY - Option bar shell: per-tool section visibility + shared helpers>
// The shell owns only the cross-tool machinery: which `[data-for-tools]`
// section is visible for the active tool, the tooltip instance, and the
// DrawToolError -> notifier policy. Everything inside a block lives in the
// feature module registered for that block's `data-section`.
import { DrawToolError } from "../../core/errors.js";
import { EVENTS } from "../../core/events.js";
import { createTooltips } from "../shared/tooltip.js";
import { mountViewOptions } from "../../features/viewport/panel_view_options.js";
import { mountShapeOptions } from "../../features/shape/panel_shape_options.js";
export { mountCollapsibleSections } from "../shared/collapsible_sections.js";

// `data-section` name -> the feature module that owns that block. A section
// without a `data-section` holds no controls of its own (help tips only).
const SECTIONS = Object.freeze({
  view: mountViewOptions,
  shape: mountShapeOptions,
});

// <META - ROLE : Mount option bar, return dispose | L18-..>
export function mountOptions(root, deps = {}) {
  if (!root || typeof document === "undefined") return () => {};
  const { session, toolManager = null, view = null } = deps;
  const disposers = [];
  const listen = (t, type, fn) => {
    t.addEventListener(type, fn);
    disposers.push(() => t.removeEventListener(type, fn));
  };
  const q = (sel) => root.querySelector(sel);
  const penSizeLabel = q("#dt-option-pen-size");
  const tips = createTooltips(root);
  disposers.push(() => tips.dispose());

  // Shared so every feature block routes errors the same way without repeating
  // the try/catch. Passed in, not imported, so features never depend on ui.
  function safe(fn) {
    try {
      fn();
    } catch (e) {
      if (e instanceof DrawToolError) session.notify("error", e.message, e.code);
      else console.error(e);
    }
  }
  function activeToolId() {
    try { return session.settings.activeTool; } catch { return "pen"; }
  }

  const parts = new Map();
  for (const [name, mount] of Object.entries(SECTIONS)) {
    const part = mount(root, { root, session, toolManager, view, safe });
    if (part) {
      parts.set(name, part);
      disposers.push(part.dispose);
    }
  }

  const sections = [...root.querySelectorAll("[data-for-tools]")].map((el) => ({
    element: el,
    name: el.dataset.section ?? null,
    tools: String(el.dataset.forTools || "").split(/\s+/).filter(Boolean),
  }));

  // Visibility is generic; the enable/disable rules inside a block belong to
  // the feature module. `sync()` runs for every section whether shown or not,
  // so the disabled state stays a pure function of session state (the original
  // inline loop did the same) instead of depending on what was last visible.
  function syncSections() {
    const id = activeToolId();
    for (const sec of sections) {
      const show = sec.tools.includes(id);
      sec.element.toggleAttribute("hidden", !show);
      if (!show) tips.closeWithin(sec.element);
      const part = parts.get(sec.name);
      if (part && typeof part.sync === "function") safe(() => part.sync());
    }
  }

  if (session) {
    listen(session, EVENTS.SETTINGS_CHANGED, (e) => {
      const k = e.detail?.key;
      if (k === "activeTool") syncSections();
      else if (k === "penSize" && penSizeLabel) penSizeLabel.textContent = `${session.settings.penSize}px`;
    });
    listen(session, EVENTS.TOOL_STATE, (e) => {
      const tool = e.detail?.tool;
      if (!tool || tool !== activeToolId()) return;
      // The section that both serves this tool and edits a live geometry box.
      // Only the shape block has a `fill` hook, which is what keeps the view
      // section (it serves every tool) from swallowing the event.
      const sec = sections.find(
        (s) => s.name && parts.has(s.name)
          && typeof parts.get(s.name).fill === "function"
          && s.tools.includes(tool),
      );
      const part = sec && parts.get(sec.name);
      if (!part) return;
      safe(() => part.fill(e.detail.pending));
      syncSections();
    });
  }

  syncSections();
  if (penSizeLabel) penSizeLabel.textContent = `${session?.settings?.penSize ?? 1}px`;
  return () => {
    for (const d of disposers) {
      try { d(); } catch { /* ignore */ }
    }
  };
}
