// <META - FILE SUMMARY - Window/document keyboard bindings for InputController (space-pan, blur)>
//
// Extracted from input_controller.js: registration only, no decision logic.

import { EVENTS } from "../core/events.js";

/**
 * Register the window/document-scoped listeners on `ctrl`.
 * @param {object} ctrl InputController instance
 * @returns {{dispose:function}} handle that removes what was registered
 */
export function bindKeyboardInput(ctrl) {
  const winBound = {};
  const won = (type, fn) => {
    window.addEventListener(type, fn);
    winBound[type] = fn;
  };

  won("keydown", (e) => {
    const t = e.target;
    const tag = t && t.tagName ? String(t.tagName).toLowerCase() : "";
    if (tag === "input" || tag === "textarea" || t?.isContentEditable === true) return;
    if (e.code === "Space" || e.key === " ") {
      if (!ctrl._spaceDown) {
        ctrl._spaceDown = true;
        if (!ctrl._panning && ctrl._drawPointerId === null) {
          ctrl._applyCursor();
        }
      }
      e.preventDefault?.();
    }
  });
  won("keyup", (e) => {
    if (e.code === "Space" || e.key === " ") {
      ctrl._spaceDown = false;
      if (!ctrl._panning) {
        ctrl._applyCursor();
      }
    }
  });
  won("blur", () => {
    ctrl._resetTransientState();
    ctrl.toolManager.cancel();
    ctrl._applyCursor();
  });
  const onVis = () => {
    if (document.visibilityState !== "hidden") return;
    ctrl._resetTransientState();
    ctrl.toolManager.cancel();
    if (!ctrl._inside) ctrl._clearHover();
    ctrl._applyCursor();
  };
  try {
    document.addEventListener("visibilitychange", onVis);
  } catch {
    // ignore (node)
  }

  const onSettings = (e) => {
    const { key } = e.detail ?? {};
    if (key !== "activeTool") return;
    ctrl._applyCursor();
  };
  try {
    ctrl.session?.addEventListener(EVENTS.SETTINGS_CHANGED, onSettings);
  } catch {
    // ignore (no session bus)
  }

  return {
    dispose() {
      for (const [type, fn] of Object.entries(winBound)) {
        try {
          window.removeEventListener(type, fn);
        } catch {
          // ignore
        }
      }
      try {
        document.removeEventListener("visibilitychange", onVis);
      } catch {
        // ignore
      }
      try {
        ctrl.session?.removeEventListener(EVENTS.SETTINGS_CHANGED, onSettings);
      } catch {
        // ignore
      }
    },
  };
}