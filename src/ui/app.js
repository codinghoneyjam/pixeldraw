// <META - FILE SUMMARY - App assembly: boot order 1-9, actions, view, menubar shell>
import { DEFAULT_DOC } from "../core/constants.js";
import { DrawToolError } from "../core/errors.js";
import { EVENTS } from "../core/events.js";
import { Session } from "../model/session.js";
import { CanvasRenderer } from "../render/renderer.js";
import { actualSizeView, clampView, fitView, stepZoom, zoomAt } from "../render/view.js";
import { ToolManager } from "../tools/tool_manager.js";
import { InputController } from "../tools/input_controller.js";
import { PenTool } from "../tools/pen.js";
import { EyedropperTool } from "../tools/eyedropper.js";
import { FillTool } from "../tools/fill.js";
import { HandTool } from "../tools/hand.js";
import { ShapeTool } from "../tools/shape.js";
import { documentToJson, importLayerJson, jsonToDocument, layerToJson } from "../io/serialize.js";
import { exportPngBytes } from "../io/export_png.js";
import { pickFile, readJsonFile, saveBinaryFile, saveTextFile } from "../io/file_io.js";
import { AutosaveStore } from "../io/store_idb.js";
import { mountColor } from "./panel_color.js";
import { mountBrush, paintPreview } from "./panel_brush.js";
import { mountLayers, paintThumb } from "./panel_layers.js";
import { mountCollapsibleSections, mountOptions } from "./panel_options.js";
import { mountStatus } from "./statusbar.js";
import { confirmDiscardChanges, showNewDocumentDialog, showProgress, showResizeCanvasDialog, showRestoreDialog } from "./dialogs.js";
import { createShortcuts } from "./shortcuts.js";
import { STRINGS } from "./strings.js";
import { iconFor } from "./icons.js";

const SETTINGS_KEY = "dt.settings.v1";
const RECENT_KEY = "dt.recentColors";
const PERSIST_KEYS = ["primaryColor", "secondaryColor", "penSize", "gridMode", "snapUnit", "shapeFill", "shapeRadius", "shapeLockAspect", "activeTool"];
const GRID_CYCLE = ["off", "unit", "tile", "pixel"];
// Recent colour history cap. Must stay in sync with the recent-colour grid width
// (DEFAULT_PALETTE.length in core/constants.js) so no swatch ever renders empty.
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

// <META - ROLE : View store owned by app with clamp + subscribe | L52-110>
function createViewStore(host, session) {
  let view = { zoom: 1, offsetX: 0, offsetY: 0 };
  const subs = new Set();
  function viewport() {
    let w = 800;
    let h = 600;
    try {
      if (host && host.clientWidth > 0) w = host.clientWidth;
      if (host && host.clientHeight > 0) h = host.clientHeight;
    } catch { /* ignore */ }
    return { w, h };
  }
  function clamp(v) {
    const doc = session.doc;
    if (!doc) return { ...v };
    const vp = viewport();
    try {
      return clampView(v, doc.canvas.widthPx, doc.canvas.heightPx, vp.w, vp.h);
    } catch {
      return { ...v };
    }
  }
  function emit() {
    for (const fn of subs) {
      try { fn(view); } catch { /* ignore */ }
    }
  }
  return {
    get() {
      return { ...view };
    },
    set(v) {
      view = clamp({ ...v });
      emit();
    },
    subscribe(fn) {
      subs.add(fn);
      return () => subs.delete(fn);
    },
    center() {
      const vp = viewport();
      return { x: vp.w / 2, y: vp.h / 2 };
    },
    zoomTo(z) {
      if (!Number.isFinite(z) || z <= 0) return;
      const c = this.center();
      this.set(zoomAt(view, c.x, c.y, z));
    },
    zoomIn() {
      this.zoomTo(stepZoom(view.zoom, 1));
    },
    zoomOut() {
      this.zoomTo(stepZoom(view.zoom, -1));
    },
    fit() {
      const doc = session.doc;
      if (!doc) return;
      const vp = viewport();
      this.set(fitView(doc.canvas.widthPx, doc.canvas.heightPx, vp.w, vp.h));
    },
    actual() {
      const doc = session.doc;
      if (!doc) return;
      const vp = viewport();
      this.set(actualSizeView(doc.canvas.widthPx, doc.canvas.heightPx, vp.w, vp.h));
    },
  };
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

const MENU_CLOSE_DELAY_MS = 220;

// <META - ROLE : Menubar shell: hover + click open, delayed leave close, dispose | L136-186>
function createMenubar(root) {
  const noop = () => {};
  if (!root || typeof document === "undefined") return { closeAll: noop, dispose: noop };
  const disposers = [];
  const listen = (t, type, fn) => {
    t.addEventListener(type, fn);
    disposers.push(() => t.removeEventListener(type, fn));
  };
  const wraps = [...root.querySelectorAll(".dt-menu")];
  let openWrap = null;
  let closeTimer = null;

  function cancelClose() {
    if (closeTimer === null) return;
    clearTimeout(closeTimer);
    closeTimer = null;
  }
  function setOpen(wrap, open) {
    const panel = wrap ? wrap.querySelector("div[role='menu']") : null;
    const trigger = wrap ? wrap.querySelector("button[data-menu]") : null;
    if (panel) panel.hidden = open !== true;
    if (trigger) trigger.setAttribute("aria-expanded", open === true ? "true" : "false");
  }
  function closeAll() {
    cancelClose();
    for (const w of wraps) setOpen(w, false);
    openWrap = null;
  }
  function open(wrap) {
    cancelClose();
    if (openWrap === wrap) return;
    closeAll();
    openWrap = wrap;
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
    listen(wrap, "pointerenter", () => open(wrap));
    listen(wrap, "pointerleave", scheduleClose);
    if (trigger) {
      listen(trigger, "click", () => {
        if (openWrap === wrap) closeAll();
        else open(wrap);
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
      // closeAll() cancels the pending close timer and hides every dropdown, so a
      // teardown mid-hover cannot leave a stranded open menu or a live timer.
      closeAll();
      for (const d of disposers) {
        try { d(); } catch { /* ignore */ }
      }
      disposers.length = 0;
    },
  };
}

// <META - ROLE : Top-bar undo/redo enable state from history caps, inert under a dialog | L188-232>
function createHistoryButtons(root, session) {
  const noop = () => {};
  if (!root || !session || typeof document === "undefined") return noop;
  const undoBtn = root.querySelector('.dt-menubar-actions button[data-action="edit.undo"]');
  const redoBtn = root.querySelector('.dt-menubar-actions button[data-action="edit.redo"]');
  if (!undoBtn && !redoBtn) return noop;
  const disposers = [];
  const on = (t, type, fn) => {
    t.addEventListener(type, fn);
    disposers.push(() => t.removeEventListener(type, fn));
  };
  const app = document.getElementById("dt-app");
  const dialogRoot = document.getElementById("dt-dialog-root");
  let observer = null;

  // Mirrors shortcuts.js isBusyUi(): a dialog owns the keyboard, so its shortcuts do too.
  function isBusy() {
    if (dialogRoot && dialogRoot.querySelector("[role='dialog']")) return true;
    return app !== null && app.getAttribute("aria-busy") === "true";
  }
  function sync() {
    const busy = isBusy();
    let u = false;
    let r = false;
    try {
      u = session.history.canUndo();
      r = session.history.canRedo();
    } catch {
      u = false;
      r = false;
    }
    if (undoBtn) undoBtn.disabled = busy || !u;
    if (redoBtn) redoBtn.disabled = busy || !r;
  }
  on(session, EVENTS.HISTORY_CHANGED, sync);
  on(session, EVENTS.DOCUMENT_REPLACED, sync);
  if (typeof MutationObserver === "function") {
    observer = new MutationObserver(sync);
    if (dialogRoot) observer.observe(dialogRoot, { childList: true });
    if (app) observer.observe(app, { attributes: true, attributeFilter: ["aria-busy"] });
  }
  sync();
  return () => {
    if (observer) observer.disconnect();
    for (const d of disposers) {
      try { d(); } catch { /* ignore */ }
    }
    disposers.length = 0;
  };
}

// <META - ROLE : Repaint canvas-backed panel content after a collapsed section reopens | L234-250>
function refreshPanelCanvases(session) {
  const s = session.settings;
  const preview = document.getElementById("dt-brush-preview");
  if (preview) paintPreview(preview, s.penSize, s.primaryColor);
  const list = document.getElementById("dt-layer-list");
  const doc = session.doc;
  if (!list || !doc) return;
  for (const li of list.querySelectorAll("li[data-layer-id]")) {
    const canvas = li.querySelector("canvas.dt-layer-thumb");
    if (canvas) paintThumb(canvas, doc.findLayer(li.dataset.layerId));
  }
}

// <META - ROLE : Boot sequence per G3 order 1-9 | L112-400>
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
          else {
            console.error(e);
            session.notify("error", STRINGS.toast.unexpected);
          }
        });
      }
      return out;
    } catch (e) {
      if (e instanceof DrawToolError) session.notify("error", e.message, e.code);
      else {
        console.error(e);
        session.notify("error", STRINGS.toast.unexpected);
      }
      return undefined;
    }
  }

  function requestUndo() {
    try {
      const t = toolManager ? toolManager.active : null;
      if (t && typeof t.hasPending === "function" && t.hasPending()) {
        t.discardPending();
        return;
      }
    } catch { /* fall through to session.undo */ }
    session.undo();
  }
  function requestRedo() {
    try {
      const t = toolManager ? toolManager.active : null;
      if (t && typeof t.hasPending === "function" && t.hasPending()) return;
    } catch { /* fall through */ }
    session.redo();
  }

  // 1. settings + recent
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

  // 2. autosave peek/restore
  let store = null;
  try {
    store = await AutosaveStore.open();
  } catch {
    store = null;
  }
  let restored = false;
  if (store) {
    let peek = null;
    try { peek = await store.peek(); } catch { peek = null; }
    if (peek) {
      const choice = await showRestoreDialog(peek);
      if (choice === "restore") {
        let doc = null;
        try { doc = await store.load(); } catch { doc = null; }
        if (doc) {
          session.loadDocument(doc, { markSaved: false });
          restored = true;
        }
      } else {
        try { await store.clear(); } catch { /* ignore */ }
      }
    }
  }
  if (!restored) {
    session.newDocument({ widthPx: DEFAULT_DOC.widthPx, heightPx: DEFAULT_DOC.heightPx, background: DEFAULT_DOC.background });
  }

  // 3. tools + renderer + input + view
  const env = { session, getView: () => viewStore.get(), requestRender: () => renderer && renderer.requestRender() };
  toolManager = new ToolManager({ session, getView: env.getView, requestRender: env.requestRender });
  toolManager.register(new PenTool(env, { mode: "draw" }));
  toolManager.register(new PenTool(env, { mode: "erase" }));
  toolManager.register(new FillTool(env));
  toolManager.register(new EyedropperTool(env));
  for (const kind of ["line", "rect", "rrect", "ellipse"]) toolManager.register(new ShapeTool(env, kind));
  toolManager.register(new HandTool(env));
  renderer = new CanvasRenderer({ host, session, getView: () => viewStore.get(), getOverlay: () => toolManager.overlay });
  renderer.attach();
  input = new InputController({
    host,
    session,
    toolManager,
    getView: () => viewStore.get(),
    setView: (v) => viewStore.set(v),
    getViewportSize: () => {
      const w = host.clientWidth || 800;
      const h = host.clientHeight || 600;
      return { w, h };
    },
    onHover: (pos) => { if (statusApi) statusApi.setCursor(pos); },
    requestRender: () => renderer.requestRender(),
  });
  input.attach();
  viewStore.fit();
  viewStore.subscribe(() => renderer.requestRender());

  // file flows
  async function doSave() {
    const doc = session.doc;
    if (!doc) return;
    const prog = showProgress("저장 중");
    try {
      const obj = await documentToJson(doc, { onProgress: (d, t) => prog.update(t ? d / t : 0) });
      const ok = await saveTextFile(`${doc.name || "untitled"}.draw.json`, JSON.stringify(obj));
      if (ok) {
        session.history.markSaved();
        session.dispatchEvent(new CustomEvent(EVENTS.HISTORY_CHANGED, {
          detail: { canUndo: session.history.canUndo(), canRedo: session.history.canRedo(), dirty: session.history.isDirty() },
        }));
        toast(STRINGS.toast.saved);
      }
    } finally {
      prog.close();
    }
  }
  async function doOpen() {
    if (session.history.isDirty()) {
      const go = await confirmDiscardChanges();
      if (!go) return;
    }
    const file = await pickFile(".json,application/json");
    if (!file) return;
    const prog = showProgress("열기");
    try {
      const obj = await readJsonFile(file);
      if (obj && obj.format === "draw_tool.document") {
        const { document: doc, warnings } = await jsonToDocument(obj, { onProgress: (d, t) => prog.update(t ? d / t : 0) });
        session.loadDocument(doc);
        for (const w of warnings) toast(w, "warn");
        toast(STRINGS.toast.opened);
      } else if (obj && obj.format === "draw_tool.layer") {
        session.notify("info", STRINGS.toast.layerFileHint);
      } else {
        throw new DrawToolError("SCHEMA", "unknown file format");
      }
    } finally {
      prog.close();
    }
  }
  async function doExportLayer() {
    const doc = session.doc;
    if (!doc) return;
    const prog = showProgress("레이어 내보내기");
    try {
      const obj = await layerToJson(doc, doc.activeLayerId);
      const layer = doc.getLayer(doc.activeLayerId);
      const ok = await saveTextFile(`${layer.name || "layer"}.drawlayer.json`, JSON.stringify(obj));
      if (ok) toast(STRINGS.toast.exportedLayer);
    } finally {
      prog.close();
    }
  }
  async function doImportLayer() {
    const file = await pickFile(".json,application/json");
    if (!file) return;
    const prog = showProgress("레이어 가져오기");
    try {
      const obj = await readJsonFile(file);
      const { layer, dropped, warnings } = await importLayerJson(session.doc, obj);
      session.insertLayer(layer);
      for (const w of warnings) toast(w, "warn");
      if (dropped > 0) toast(`${STRINGS.toast.droppedChunks}: ${dropped}`, "warn");
      toast(STRINGS.toast.importedLayer);
    } finally {
      prog.close();
    }
  }
  async function doExportPng() {
    const doc = session.doc;
    if (!doc) return;
    const prog = showProgress("PNG 내보내기");
    try {
      const bytes = await exportPngBytes(doc, { includeBackground: true });
      prog.update(0.8);
      const ok = await saveBinaryFile(`${doc.name || "untitled"}.png`, bytes, "image/png");
      if (ok) toast(STRINGS.toast.exportedPng);
    } finally {
      prog.close();
    }
  }
  async function doNew() {
    if (session.history.isDirty()) {
      const go = await confirmDiscardChanges();
      if (!go) return;
    }
    const res = await showNewDocumentDialog();
    if (!res) return;
    let id = undefined;
    try {
      if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") id = crypto.randomUUID();
    } catch { /* ignore */ }
    session.newDocument({ widthPx: res.widthPx, heightPx: res.heightPx, background: res.background, name: res.name, id });
  }

  const actions = {
    "file.new": () => runAction(doNew),
    "file.open": () => runAction(doOpen),
    "file.save": () => runAction(doSave),
    "file.exportLayer": () => runAction(doExportLayer),
    "file.importLayer": () => runAction(doImportLayer),
    "file.exportPng": () => runAction(doExportPng),
    "edit.undo": () => runAction(requestUndo),
    "edit.redo": () => runAction(requestRedo),
    "canvas.resize": () => runAction(async () => {
      const doc = session.doc;
      if (!doc) return;
      const res = await showResizeCanvasDialog({ widthPx: doc.canvas.widthPx, heightPx: doc.canvas.heightPx });
      if (res) session.resizeCanvas(res.widthPx, res.heightPx);
    }),
    "view.zoomIn": () => viewStore.zoomIn(),
    "view.zoomOut": () => viewStore.zoomOut(),
    "view.fit": () => viewStore.fit(),
    "view.actual": () => viewStore.actual(),
    "view.gridCycle": () => runAction(() => {
      const cur = session.settings.gridMode;
      const next = GRID_CYCLE[(GRID_CYCLE.indexOf(cur) + 1) % GRID_CYCLE.length];
      session.setSetting("gridMode", next);
    }),
    "layer.add": () => runAction(() => session.addLayer()),
    "layer.duplicate": () => runAction(() => session.duplicateLayer(session.doc.activeLayerId)),
    "layer.remove": () => runAction(() => session.removeLayer(session.doc.activeLayerId)),
    "layer.mergeDown": () => runAction(() => session.mergeDown(session.doc.activeLayerId)),
    "layer.up": () => runAction(() => {
      const d = session.doc;
      session.moveLayer(d.activeLayerId, d.indexOf(d.activeLayerId) + 1);
    }),
    "layer.down": () => runAction(() => {
      const d = session.doc;
      session.moveLayer(d.activeLayerId, d.indexOf(d.activeLayerId) - 1);
    }),
    "brush.step": (delta) => runAction(() => {
      const cur = session.settings.penSize;
      session.setSetting("penSize", Math.max(1, Math.min(64, cur + delta)));
    }),
    "color.swap": () => runAction(() => {
      const s = session.settings;
      session.setSetting("primaryColor", s.secondaryColor);
      session.setSetting("secondaryColor", s.primaryColor);
    }),
    "color.reset": () => runAction(() => {
      session.setSetting("primaryColor", "#000000");
      session.setSetting("secondaryColor", "#ffffff");
    }),
  };

  // menubar wiring
  const menubar = document.getElementById("dt-menubar");
  const menuShell = createMenubar(menubar);
  const disposeHistoryButtons = createHistoryButtons(menubar, session);
  panelDisposers.push(() => {
    menuShell.dispose();
    disposeHistoryButtons();
  });
  // One delegation covers dropdown items AND the top-bar undo/redo group.
  for (const item of menubar.querySelectorAll("button[data-action]")) {
    item.addEventListener("click", () => {
      if (item.disabled) return;
      menuShell.closeAll();
      const fn = actions[item.dataset.action];
      if (typeof fn === "function") fn();
    });
  }

  // toolbar wiring
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

  // 4. panels + status
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

  // 5. autosave attach
  if (store) store.attach(session);

  // 6. window events
  window.addEventListener("resize", () => {
    renderer.resize();
    viewStore.set(viewStore.get());
  });
  window.addEventListener("beforeunload", (e) => {
    if (session.history.isDirty()) e.preventDefault();
  });

  // 7-8. replaced => fit; status notify => toast for warn/error
  session.addEventListener(EVENTS.DOCUMENT_REPLACED, () => viewStore.fit());
  session.addEventListener(EVENTS.STATUS_MESSAGE, (e) => {
    const { level = "info", text = "" } = e.detail ?? {};
    if (level === "warn" || level === "error") toast(String(text), level);
  });

  const shortcuts = createShortcuts({ toolManager, session, actions, requestUndo, requestRedo });

  // 9. debug
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

// Shell helpers are exported so the UI contract (menu state machine, undo/redo
// enable state, canvas refresh on re-expand) is testable without a browser.
export { boot, createHistoryButtons, createMenubar, refreshPanelCanvases };
