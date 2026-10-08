// <META - FILE SUMMARY - Keyboard shortcuts: pure resolve + window handler (G2 rules 1-6)>
function normKey(key) {
  if (typeof key !== "string") return "";
  if (key === " ") return "Space";
  return key.length === 1 ? key.toLowerCase() : key;
}

// <META - ROLE : Pure key-descriptor to intent mapping | L10-90>
export function resolveShortcut(desc = {}) {
  const key = normKey(desc.key);
  const ctrl = desc.ctrl === true || desc.meta === true;
  const { shift = false, alt = false } = desc;
  if (alt) return null;
  if (ctrl) {
    const k = key;
    if (k === "n" && shift) return { kind: "action", action: "layer.add" };
    if (k === "n") return { kind: "action", action: "file.new" };
    if (k === "o") return { kind: "action", action: "file.open" };
    if (k === "s") return { kind: "action", action: "file.save" };
    if (k === "e") return { kind: "action", action: "file.exportPng" };
    if (k === "z" && shift) return { kind: "redo" };
    if (k === "z") return { kind: "undo" };
    if (k === "y") return { kind: "redo" };
    if (k === "j") return { kind: "action", action: "layer.duplicate" };
    if (k === "m" && shift) return { kind: "action", action: "layer.mergeDown" };
    if (k === "]") return { kind: "action", action: "layer.up" };
    if (k === "[") return { kind: "action", action: "layer.down" };
    if (k === "'") return { kind: "action", action: "view.gridCycle" };
    return null;
  }
  if (key === "+" || key === "=") return { kind: "action", action: "view.zoomIn" };
  if (key === "-") return { kind: "action", action: "view.zoomOut" };
  if (key === "0") return { kind: "action", action: "view.fit" };
  if (key === "1") return { kind: "action", action: "view.actual" };
  if (key === "b") return { kind: "tool", tool: "pen" };
  if (key === "e") return { kind: "tool", tool: "eraser" };
  if (key === "g") return { kind: "tool", tool: "fill" };
  if (key === "i") return { kind: "tool", tool: "eyedropper" };
  if (key === "l") return { kind: "tool", tool: "line" };
  if (key === "r") return { kind: "tool", tool: "rect" };
  if (key === "u") return { kind: "tool", tool: "rrect" };
  if (key === "o") return { kind: "tool", tool: "ellipse" };
  if (key === "p") return { kind: "tool", tool: "polygon" };
  if (key === "h") return { kind: "tool", tool: "hand" };
  if (key === "x") return { kind: "swapColors" };
  if (key === "d") return { kind: "resetColors" };
  if (key === "[" || key === "{") return { kind: "penSize", delta: shift ? -5 : -1 };
  if (key === "]" || key === "}") return { kind: "penSize", delta: shift ? 5 : 1 };
  return null;
}

// <META - ROLE : Editable-target guard (rule 1) | L92-104>
export function isEditableTarget(target) {
  if (!target || typeof target.closest !== "function") return false;
  try {
    return target.closest("input,textarea,select,[contenteditable='true'],[contenteditable='']") !== null;
  } catch {
    return false;
  }
}

function isBusyUi() {
  if (typeof document === "undefined") return false;
  const root = document.getElementById("dt-dialog-root");
  if (root && root.querySelector("[role='dialog']")) return true;
  const app = document.getElementById("dt-app");
  return app !== null && app.getAttribute("aria-busy") === "true";
}

// <META - ROLE : Window keydown handler implementing rules 1-6 | L106-190>
export function createShortcuts(deps = {}) {
  const { toolManager = null, session = null, actions = {}, requestUndo = null, requestRedo = null } = deps;
  function handleKeyDown(e) {
    if (!e || typeof e.key !== "string") return false;
    if (e.key === " ") return false;
    const desc = {
      key: e.key,
      ctrl: e.ctrlKey === true,
      meta: e.metaKey === true,
      shift: e.shiftKey === true,
      alt: e.altKey === true,
    };
    if (isEditableTarget(e.target)) {
      if (e.key === "Escape" && e.target && typeof e.target.blur === "function") {
        e.target.blur();
        e.preventDefault();
        return true;
      }
      return false;
    }
    if (isBusyUi() || (typeof deps.isBusy === "function" && deps.isBusy())) return false;
    if (toolManager && typeof toolManager.keyDown === "function") {
      let handled = false;
      try {
        handled = toolManager.keyDown({ key: e.key, shift: desc.shift, ctrl: desc.ctrl || desc.meta, alt: desc.alt });
      } catch {
        handled = false;
      }
      if (handled === true) {
        e.preventDefault();
        return true;
      }
    }
    const hit = resolveShortcut(desc);
    if (!hit) return false;
    e.preventDefault();
    try {
      if (hit.kind === "undo") {
        if (typeof requestUndo === "function") requestUndo();
        else if (typeof actions["edit.undo"] === "function") actions["edit.undo"]();
      } else if (hit.kind === "redo") {
        if (typeof requestRedo === "function") requestRedo();
        else if (typeof actions["edit.redo"] === "function") actions["edit.redo"]();
      } else if (hit.kind === "tool") {
        if (session && typeof session.setSetting === "function") session.setSetting("activeTool", hit.tool);
      } else if (hit.kind === "penSize") {
        if (typeof actions["brush.step"] === "function") actions["brush.step"](hit.delta);
        else if (session && typeof session.setSetting === "function") {
          const cur = session.settings.penSize;
          session.setSetting("penSize", Math.max(1, Math.min(64, cur + hit.delta)));
        }
      } else if (hit.kind === "swapColors") {
        if (typeof actions["color.swap"] === "function") actions["color.swap"]();
      } else if (hit.kind === "resetColors") {
        if (typeof actions["color.reset"] === "function") actions["color.reset"]();
      } else if (hit.kind === "action") {
        const fn = actions[hit.action];
        if (typeof fn === "function") fn();
      }
    } catch { /* action-level errors surface via runAction in app */ }
    return true;
  }
  let attached = false;
  if (typeof window !== "undefined" && typeof window.addEventListener === "function") {
    window.addEventListener("keydown", handleKeyDown);
    attached = true;
  }
  return {
    handleKeyDown,
    dispose() {
      if (attached) {
        try { window.removeEventListener("keydown", handleKeyDown); } catch { /* ignore */ }
        attached = false;
      }
    },
  };
}
