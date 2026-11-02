import { DrawToolError } from "../core/errors.js";
import { EVENTS } from "../core/events.js";

export class ToolManager {
  constructor({ session, getView = null, requestRender = () => {} } = {}) {
    if (!session) throw new DrawToolError("INVALID_STATE", "ToolManager needs a session");
    this.session = session;
    this.getView = getView;
    this.requestRender = requestRender;
    this._tools = new Map();
    this._activeId = session.settings.activeTool;
    this._onSettings = (e) => {
      const { key, value } = e.detail ?? {};
      if (key !== "activeTool") return;
      this._switchTo(value);
    };
    session.addEventListener(EVENTS.SETTINGS_CHANGED, this._onSettings);
    const initial = this._tools.get(this._activeId);
    if (initial) {
      try {
        initial.activate();
      } catch {
        // ignore activation failure of initial tool
      }
    }
  }

  register(tool) {
    if (!tool || typeof tool.id !== "string") {
      throw new DrawToolError("INVALID_STATE", "register needs a tool with an id");
    }
    this._tools.set(tool.id, tool);
    if (tool.id === this._activeId) {
      try {
        tool.activate();
      } catch {
        // ignore
      }
    }
    return tool;
  }

  get(id) {
    return this._tools.get(id) ?? null;
  }

  get active() {
    const tool = this._tools.get(this._activeId);
    if (!tool) throw new DrawToolError("INVALID_STATE", `unregistered tool ${String(this._activeId)}`);
    return tool;
  }

  get activeId() {
    return this._activeId;
  }

  _switchTo(nextId) {
    const prevId = this._activeId;
    if (prevId === nextId) return;
    const prev = this._tools.get(prevId) ?? null;
    if (prev && this.session.isEditing) {
      try {
        prev.cancel();
      } catch {
        // cancel must not block the switch
      }
    }
    if (prev) {
      try {
        prev.deactivate();
      } catch {
        // ignore
      }
    }
    this._activeId = nextId;
    const next = this._tools.get(nextId);
    if (!next) return;
    try {
      next.activate();
    } catch {
      // ignore
    }
  }

  pointerDown(ev) {
    this.active.pointerDown(ev);
  }

  pointerMove(ev) {
    this.active.pointerMove(ev);
  }

  pointerUp(ev) {
    this.active.pointerUp(ev);
  }

  cancel() {
    try {
      this.active.cancel();
    } catch {
      // already-clean tools may throw; swallow for input paths
    }
  }

  hover(ev) {
    this.active.hover(ev);
  }

  keyDown(ev) {
    return this.active.keyDown(ev);
  }

  get overlay() {
    let tool = null;
    try {
      tool = this.active;
    } catch {
      return null;
    }
    if (typeof tool.overlay !== "function") return null;
    return tool.overlay.bind(tool);
  }

  get cursor() {
    try {
      return this.active.cursor;
    } catch {
      return "default";
    }
  }

  dispose() {
    this.session.removeEventListener(EVENTS.SETTINGS_CHANGED, this._onSettings);
  }
}
