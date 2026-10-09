// <META - FILE SUMMARY - App assembly: boot order (settings -> autosave -> tools -> chrome)>
import { DEFAULT_DOC } from "../core/constants.js";
import { DrawToolError } from "../core/errors.js";
import { EVENTS } from "../core/events.js";
import { Session } from "../features/document/session.js";
import { CanvasRenderer } from "../render/renderer.js";
import { ToolManager } from "../tools/tool_manager.js";
import { InputController } from "../tools/input_controller.js";
import { PenTool } from "../features/pen/pen.js";
import { EyedropperTool } from "../features/eyedropper/eyedropper.js";
import { FillTool } from "../features/fill/fill.js";
import { HandTool } from "../features/hand/hand.js";
import { ShapeTool } from "../features/shape/shape.js";
import { AutosaveStore } from "../io/store_idb.js";
import { mountColor } from "../features/color/panel_color.js";
import { mountBrush } from "../features/pen/panel_brush.js";
import { mountLayers } from "../features/layers/panel_layers.js";
import { mountCollapsibleSections, mountOptions } from "./optionbar/panel_options.js";
import { mountStatus } from "./statusbar/statusbar.js";
import { showRestoreDialog } from "./shared/dialogs.js";
import { createShortcuts } from "./toolbar/shortcuts.js";
import { STRINGS } from "./shared/strings.js";
import { createViewStore } from "../features/viewport/view_store.js";
import { createMenubar } from "./menubar/menubar.js";
import { createHistoryButtons } from "./menubar/history_buttons.js";
import { refreshPanelCanvases } from "./shared/panel_refresh.js";
import { requestUndo, requestRedo } from "./actions/edit_actions.js";
import { buildActions } from "./actions/action_map.js";
import { wireMenubar } from "./menubar/menubar_wiring.js";
import { wireToolbar } from "./toolbar/toolbar_wiring.js";
import { RECENT_KEY, RECENT_MAX, restoreSettings, loadRecentColors, storeJson, attachSettingsPersist } from "./shared/storage.js";
import { toast } from "./shared/toast.js";

// <META - ROLE : Boot sequence per G3 order 1-9 | L35-170>
async function boot() {
  const app = document.getElementById("dt-app");
  const host = document.getElementById("dt-canvas-host");
  if (!app || !host) throw new Error("missing #dt-app or #dt-canvas-host");

  const session = new Session();
  const viewStore = createViewStore(host, session);
  let renderer = null;
  let toolManager = null;
  let input = null;
  let statusApi = null;
  const panelDisposers = [];

  function runAction(fn) {
    try {
      const out = fn();
      if (out && typeof out.catch === "function") {
        out.catch((e) => {
          if (e instanceof DrawToolError) session.notify("error", e.message, e.code);
          else { console.error(e); session.notify("error", STRINGS.toast.unexpected); }
        });
      }
      return out;
    } catch (e) {
      if (e instanceof DrawToolError) session.notify("error", e.message, e.code);
      else { console.error(e); session.notify("error", STRINGS.toast.unexpected); }
      return undefined;
    }
  }

  // Boot order matters: settings + recent colours first, then autosave restore,
  // then tools/renderer/input, then panels.
  restoreSettings(session);
  let recentColors = loadRecentColors();

  // Autosave peek/restore runs BEFORE any document exists: if the user declines,
  // newDocument() below supplies the default document instead.
  let store = null;
  try { store = await AutosaveStore.open(); } catch { store = null; }
  let restored = false;
  if (store) {
    let peek = null;
    try { peek = await store.peek(); } catch { peek = null; }
    if (peek) {
      const choice = await showRestoreDialog(peek);
      if (choice === "restore") {
        let doc = null;
        try { doc = await store.load(); } catch { doc = null; }
        if (doc) { session.loadDocument(doc, { markSaved: false }); restored = true; }
      } else {
        try { await store.clear(); } catch { /* ignore */ }
      }
    }
  }
  if (!restored) {
    session.newDocument({ widthPx: DEFAULT_DOC.widthPx, heightPx: DEFAULT_DOC.heightPx, background: DEFAULT_DOC.background });
  }

  // Tools, renderer, input, and view need the document from the step above.
  const env = { session, getView: () => viewStore.get(), requestRender: () => renderer && renderer.requestRender() };
  toolManager = new ToolManager({ session, getView: env.getView, requestRender: env.requestRender });
  toolManager.register(new PenTool(env, { mode: "draw" }));
  toolManager.register(new PenTool(env, { mode: "erase" }));
  toolManager.register(new FillTool(env));
  toolManager.register(new EyedropperTool(env));
  for (const kind of ["line", "rect", "rrect", "ellipse", "polygon"]) toolManager.register(new ShapeTool(env, kind));
  toolManager.register(new HandTool(env));
  renderer = new CanvasRenderer({ host, session, getView: () => viewStore.get(), getOverlay: () => toolManager.overlay });
  renderer.attach();
  input = new InputController({
    host, session, toolManager,
    getView: () => viewStore.get(),
    setView: (v) => viewStore.set(v),
    getViewportSize: () => ({ w: host.clientWidth || 800, h: host.clientHeight || 600 }),
    onHover: (pos) => { if (statusApi) statusApi.setCursor(pos); },
    requestRender: () => renderer.requestRender(),
  });
  input.attach();
  viewStore.fit();
  viewStore.subscribe(() => renderer.requestRender());

  const actions = buildActions({ session, toolManager, viewStore, runAction, toast });

  panelDisposers.push(wireMenubar(document.getElementById("dt-menubar"), { session, actions }));
  wireToolbar(document.getElementById("dt-toolbar"), { session, runAction });

  // Panels mount last: they read settings and the view store established above.
  statusApi = mountStatus(document.getElementById("dt-statusbar"), { session, view: viewStore });
  panelDisposers.push(mountColor(document.getElementById("dt-left-panels"), {
    session,
    getRecent: () => recentColors.slice(),
    saveRecent: (list) => { recentColors = list.slice(0, RECENT_MAX); storeJson(RECENT_KEY, recentColors); },
  }));
  panelDisposers.push(mountBrush(document.getElementById("dt-brush-panel"), { session }));
  panelDisposers.push(mountLayers(document.getElementById("dt-layer-panel"), { session }));
  panelDisposers.push(mountOptions(document.getElementById("dt-optionbar"), { session, toolManager, view: viewStore }));
  panelDisposers.push(mountCollapsibleSections(
    [document.getElementById("dt-left-panels"), document.getElementById("dt-right-panels")],
    { onExpand: () => refreshPanelCanvases(session) },
  ));

  attachSettingsPersist(session);

  if (store) store.attach(session);

  window.addEventListener("resize", () => { renderer.resize(); viewStore.set(viewStore.get()); });
  window.addEventListener("beforeunload", (e) => { if (session.history.isDirty()) e.preventDefault(); });

  session.addEventListener(EVENTS.DOCUMENT_REPLACED, () => viewStore.fit());
  session.addEventListener(EVENTS.STATUS_MESSAGE, (e) => {
    const { level = "info", text = "" } = e.detail ?? {};
    if (level === "warn" || level === "error") toast(String(text), level);
  });

  const shortcuts = createShortcuts({ toolManager, session, actions,
    requestUndo: () => requestUndo(session, toolManager),
    requestRedo: () => requestRedo(session, toolManager),
  });

  // `?debug` exposes the live session/tool handles on window for manual probing.
  try {
    if (new URLSearchParams(location.search).has("debug")) {
      window.__drawTool = { session, getView: () => viewStore.get(), toolManager };
    }
  } catch { /* ignore */ }

  window.addEventListener("unload", () => {
    shortcuts.dispose();
    input.dispose();
    renderer.dispose();
    toolManager.dispose();
    for (const d of panelDisposers) {
      try { d(); } catch { /* ignore */ }
    }
    if (statusApi) statusApi();
  });
}

if (typeof document !== "undefined") {
  boot().catch((e) => console.error(e));
}

export { boot, createHistoryButtons, createMenubar, refreshPanelCanvases };
