// <META - FILE SUMMARY - App assembly: boot order (settings -> autosave -> tools -> chrome)>
import { DEFAULT_DOC } from "../core/constants.js";
import { DrawToolError } from "../core/errors.js";
import { EVENTS } from "../core/events.js";
import { Session } from "../model/session.js";
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
import { iconFor } from "./toolbar/icons.js";
import { createViewStore } from "../features/viewport/view_store.js";
import { createMenubar } from "./menubar/menubar.js";
import { createHistoryButtons } from "./menubar/history_buttons.js";
import { refreshPanelCanvases } from "./shared/panel_refresh.js";
import { doSave, doOpen, doExportLayer, doImportLayer, doExportPng, doNew, doImportRecipe, doExportVector, doImportVector } from "./actions/file_actions.js";
import { requestUndo, requestRedo, brushStep, colorSwap, colorReset } from "./actions/edit_actions.js";
import { zoomIn, zoomOut, fit, actual, gridCycle, canvasResize } from "./actions/edit_actions.js";
import { layerAdd, layerDuplicate, layerRemove, layerMergeDown, layerUp, layerDown } from "../features/layers/layer_actions.js";

const SETTINGS_KEY = "dt.settings.v1";
const RECENT_KEY = "dt.recentColors";
const PERSIST_KEYS = ["primaryColor", "secondaryColor", "penSize", "gridMode", "snapUnit", "shapeFill", "shapeRadius", "shapeLockAspect", "activeTool"];
const RECENT_MAX = 32;

function loadJson(key) {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return null;
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function storeJson(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch { /* ignore */ }
}

function toast(text, level = "info") {
  if (typeof document === "undefined") return;
  const box = document.getElementById("dt-toast");
  if (!box) return;
  const item = document.createElement("div");
  item.className = `dt-toast-item dt-toast-${level}`;
  item.textContent = text;
  box.append(item);
  setTimeout(() => item.remove(), level === "error" ? 8000 : 4000);
}

// <META - ROLE : Boot sequence per G3 order 1-9 | L60-210>
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
  const saved = loadJson(SETTINGS_KEY);
  if (saved && typeof saved === "object") {
    for (const k of PERSIST_KEYS) {
      if (saved[k] !== undefined) {
        try { session.setSetting(k, saved[k]); } catch { /* ignore corrupt */ }
      }
    }
  }
  let recentColors = [];
  const savedRecent = loadJson(RECENT_KEY);
  if (Array.isArray(savedRecent)) recentColors = savedRecent.filter((c) => typeof c === "string").slice(0, RECENT_MAX);

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

  const actions = {
    "file.new": () => runAction(() => doNew(session)),
    "file.open": () => runAction(() => doOpen(session, toast)),
    "file.save": () => runAction(() => doSave(session, toast)),
    "file.exportLayer": () => runAction(() => doExportLayer(session, toast)),
    "file.importLayer": () => runAction(() => doImportLayer(session, toast)),
    "file.exportPng": () => runAction(() => doExportPng(session, toast)),
    "file.importRecipe": () => runAction(() => doImportRecipe(session, toast)),
    "file.exportVector": () => runAction(() => doExportVector(session, toast)),
    "file.importVector": () => runAction(() => doImportVector(session, toast)),
    "edit.undo": () => runAction(() => requestUndo(session, toolManager)),
    "edit.redo": () => runAction(() => requestRedo(session, toolManager)),
    "canvas.resize": () => runAction(() => canvasResize(session)),
    "view.zoomIn": () => zoomIn(viewStore),
    "view.zoomOut": () => zoomOut(viewStore),
    "view.fit": () => fit(viewStore),
    "view.actual": () => actual(viewStore),
    "view.gridCycle": () => runAction(() => gridCycle(session)),
    "layer.add": () => runAction(() => layerAdd(session)),
    "layer.duplicate": () => runAction(() => layerDuplicate(session)),
    "layer.remove": () => runAction(() => layerRemove(session)),
    "layer.mergeDown": () => runAction(() => layerMergeDown(session)),
    "layer.up": () => runAction(() => layerUp(session)),
    "layer.down": () => runAction(() => layerDown(session)),
    "brush.step": (delta) => runAction(() => brushStep(session, delta)),
    "color.swap": () => runAction(() => colorSwap(session)),
    "color.reset": () => runAction(() => colorReset(session)),
  };

  const menubar = document.getElementById("dt-menubar");
  const menuShell = createMenubar(menubar);
  const disposeHistoryButtons = createHistoryButtons(menubar, session);
  panelDisposers.push(() => { menuShell.dispose(); disposeHistoryButtons(); });
  for (const item of menubar.querySelectorAll("button[data-action]")) {
    item.addEventListener("click", () => {
      if (item.disabled) return;
      menuShell.closeAll();
      const fn = actions[item.dataset.action];
      if (typeof fn === "function") fn();
    });
  }

  const toolbar = document.getElementById("dt-toolbar");
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

  // Panels mount last: they read settings and the view store established above.
  statusApi = mountStatus(document.getElementById("dt-statusbar"), { session, view: viewStore });
  panelDisposers.push(mountColor(document.getElementById("dt-color-panel"), {
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

  // persist settings debounce 250ms
  let persistTimer = null;
  session.addEventListener(EVENTS.SETTINGS_CHANGED, () => {
    if (persistTimer !== null) clearTimeout(persistTimer);
    persistTimer = setTimeout(() => {
      persistTimer = null;
      const s = session.settings;
      const out = {};
      for (const k of PERSIST_KEYS) out[k] = s[k];
      storeJson(SETTINGS_KEY, out);
    }, 250);
  });

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
