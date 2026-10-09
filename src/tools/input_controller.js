// <META - FILE SUMMARY - InputController: pointer state machine (pan/zoom) + tool dispatch>
// <META - SUMMARY CONT - Event registration lives in pointer_bindings.js / keyboard_bindings.js
// <META - SUMMARY CONT - The cursor priority ladder lives in cursor_state.js (DOM-free)>

import { bindKeyboardInput } from "./keyboard_bindings.js";
import { bindPointerInput } from "./pointer_bindings.js";
import { WheelAccumulator } from "./wheel_accumulator.js";
import { applyCursor, markInside } from "./cursor_state.js";

// Re-exported so the wheel-accumulator and cursor-state contracts keep one
// public import site: callers import from input_controller.js, not the internals.
export { WheelAccumulator };
export { DEFAULT_CURSOR, PAN_CURSOR, SPACE_CURSOR, resolveCursor, readCursorState } from "./cursor_state.js";

export class InputController {
  constructor({ host, session, toolManager, getView, setView, getViewportSize, onHover = () => {}, requestRender = () => {} } = {}) {
    this.host = host;
    this.session = session;
    this.toolManager = toolManager;
    this.getView = getView;
    this.setView = setView;
    this.getViewportSize = getViewportSize;
    this.onHover = onHover;
    this.requestRender = requestRender;
    this._attached = false;
    this._panning = false;
    this._panPointerId = null;
    this._panLast = null;
    this._drawPointerId = null;
    this._spaceDown = false;
    this._inside = true;
    this._wheel = new WheelAccumulator();
    this._bindings = null;
  }

  // <META - ROLE : pan gating: middle button, space-held, or the hand tool | L35-45>
  _shouldPan(domEv) {
    if (domEv.button === 1) return true;
    if (this._spaceDown) return true;
    try {
      if (this.toolManager.active.id === "hand") return true;
    } catch {
      // ignore
    }
    return false;
  }

  // <META - ROLE : ask the active tool which buttons it wants; left-only fallback | L47-61>
  _toolAcceptsButton(button) {
    let tool = null;
    try {
      tool = this.toolManager?.active ?? null;
    } catch {
      return button === 0;
    }
    if (!tool || typeof tool.acceptsButton !== "function") return button === 0;
    try {
      return tool.acceptsButton(button) === true;
    } catch {
      return false;
    }
  }

  _hostRect() {
    try {
      return this.host.getBoundingClientRect();
    } catch {
      return { left: 0, top: 0 };
    }
  }

  // <META - ROLE : cursor write-through; the ladder itself lives in cursor_state.js | L71-74>
  _applyCursor() {
    return applyCursor(this);
  }

  // <META - ROLE : pointer presence flag; delegates to cursor_state.js | L77-80>
  _markInside(inside = true) {
    return markInside(this, inside);
  }

  // <META - ROLE : shared "no preview" path so leave/cancel never leave stale hover | L81-89>
  _clearHover() {
    this.toolManager.hover(null);
    try {
      this.onHover(null);
    } catch {
      // ignore
    }
  }

  // <META - ROLE : while outside, decide from the rect if a captured move came back | L91-101>
  _syncInside(domEv, rect) {
    if (!this._panning && this._drawPointerId === null) {
      this._markInside();
      return;
    }
    if (typeof rect.right !== "number" || typeof rect.bottom !== "number") return;
    const { clientX: x, clientY: y } = domEv;
    if (x < rect.left || x >= rect.right || y < rect.top || y >= rect.bottom) this._markInside(false);
    else this._markInside();
  }

  // <META - ROLE : drop all transient pointer state (blur / tab hidden) | L103-110>
  _resetTransientState() {
    this._spaceDown = false;
    this._panning = false;
    this._panPointerId = null;
    this._panLast = null;
    this._drawPointerId = null;
  }

  attach() {
    if (this._attached || !this.host) return;
    this._attached = true;
    const pointer = bindPointerInput(this);
    const keyboard = bindKeyboardInput(this);
    this._bindings = { pointer, keyboard };
    this._applyCursor();
  }

  dispose() {
    if (!this._attached) return;
    this._attached = false;
    if (this._bindings) {
      this._bindings.pointer.dispose();
      this._bindings.keyboard.dispose();
      this._bindings = null;
    }
  }
}