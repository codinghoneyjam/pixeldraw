// <META - FILE SUMMARY - View option section: grid mode + zoom select/fit/actual>
// Owns the controls that read or write the view. The zoom select mirrors the
// view through its subscription; the grid select writes a setting.
import { ZOOM_LEVELS } from "../../core/constants.js";
import { EVENTS } from "../../core/events.js";

// <META - ROLE : Mount the view controls; returns {dispose} or null | L9-..>
export function mountViewOptions(root, deps = {}) {
  const { session, view = null, safe = (fn) => fn() } = deps;
  const q = (sel) => root?.querySelector(sel) ?? null;
  const gridSel = q("#dt-grid-mode");
  const zoomSel = q("#dt-zoom-select");
  const zoomFit = q("#dt-zoom-fit");
  const zoomActual = q("#dt-zoom-actual");
  if (!gridSel && !zoomSel && !zoomFit && !zoomActual) return null;

  const disposers = [];
  const listen = (t, type, fn) => {
    t.addEventListener(type, fn);
    disposers.push(() => t.removeEventListener(type, fn));
  };

  // <META - ROLE : Nearest level to the current zoom | L26-29>
  function syncZoom() {
    if (!zoomSel || !view) return;
    const z = view.get().zoom;
    const best = ZOOM_LEVELS.reduce((a, b) => (Math.abs(b - z) < Math.abs(a - z) ? b : a), ZOOM_LEVELS[0]);
    zoomSel.value = String(best);
  }

  // <META - ROLE : Reflect gridMode, unless the user is picking in the select | L31-34>
  function syncGrid() {
    if (!gridSel || document.activeElement === gridSel) return;
    gridSel.value = session.settings.gridMode;
  }

  if (zoomSel) {
    zoomSel.replaceChildren();
    for (const z of ZOOM_LEVELS) {
      const o = document.createElement("option");
      o.value = String(z);
      o.textContent = `${Math.round(z * 100)}%`;
      zoomSel.append(o);
    }
    zoomSel.addEventListener("change", () => safe(() => {
      if (view) view.zoomTo(Number(zoomSel.value));
    }));
  }
  if (gridSel) gridSel.addEventListener("change", () => safe(() => session.setSetting("gridMode", gridSel.value)));
  if (zoomFit) zoomFit.addEventListener("click", () => safe(() => view && view.fit()));
  if (zoomActual) zoomActual.addEventListener("click", () => safe(() => view && view.actual()));

  if (session) {
    listen(session, EVENTS.SETTINGS_CHANGED, (e) => {
      if (e.detail?.key === "gridMode") syncGrid();
    });
  }
  if (view && typeof view.subscribe === "function") {
    disposers.push(view.subscribe(syncZoom));
  }

  syncGrid();
  syncZoom();
  return {
    dispose: () => {
      for (const d of disposers) {
        try { d(); } catch { /* ignore */ }
      }
    },
  };
}
