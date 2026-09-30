// <META - FILE SUMMARY - EyedropperTool: colour pick on left or right mouse button>
// <META - BUTTON MAP - L/R pick, Shift -> secondary, Alt -> active-layer raw pixel>

/*
 * Eyedropper button -> colour mapping
 *   left  (button 0)               -> primaryColor   (sampled from the display composite)
 *   left  (button 0) + Shift       -> secondaryColor
 *   right (button 2)              -> secondaryColor
 *   right (button 2) + Shift      -> secondaryColor (the Shift rule wins, unchanged legacy behaviour)
 *   any accepted button + Alt     -> read the ACTIVE LAYER's raw pixel instead of the composite
 *   middle (button 1) / other      -> ignored entirely
 * Holding a button and dragging keeps sampling until pointerUp; the button chosen
 * at pointerdown is latched, because pointermove reports button -1 once the
 * button is no longer changing and would otherwise retarget a right-drag.
 */

import { toHex, unpackRGBA } from "../core/pixel.js";
import { samplePixel } from "../render/composite.js";
import { Tool } from "./tool_base.js";

const EYEDROPPER_CURSOR =
  "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24'%3E%3Crect x='10.5' y='2' width='3' height='9' fill='white' stroke='black' stroke-width='1'/%3E%3Ccircle cx='12' cy='14' r='3.5' fill='white' stroke='black' stroke-width='1.2'/%3E%3Cpath d='M12 17.5 L12 21' stroke='black' stroke-width='1.5'/%3E%3Ccircle cx='12' cy='21' r='1' fill='black'/%3E%3C/svg%3E\") 12 21, crosshair";

export class EyedropperTool extends Tool {
  // <META - ROLE : construct with drag sampling state | L26-31>
  constructor(env = {}) {
    super(env);
    this._down = false;
    this._btn = 0;
    this._pos = null;
  }

  get id() {
    return "eyedropper";
  }

  get cursor() {
    return EYEDROPPER_CURSOR;
  }

  // <META - ROLE : release drag sampling when the tool is swapped out | L42-44>
  deactivate() {
    this._release();
  }

  cancel() {
    this._release();
  }

  // <META - ROLE : clear the drag latch so a released button stops sampling | L51-54>
  _release() {
    this._down = false;
    this._btn = 0;
  }

  // <META - ROLE : button acceptance override: left AND right pick, middle ignored | L57-61>
  acceptsButton(button) {
    const b = Number(button);
    if (!Number.isFinite(b)) return false;
    return b === 0 || b === 2;
  }

  // <META - ROLE : resolve rgba from active layer (alt) or composite, then write colour | L64-87>
  _pick(ev) {
    if (!this.session || !this.session.doc) return;
    const doc = this.session.doc;
    let rgba;
    if (ev.alt === true) {
      let layer = null;
      try {
        layer = doc.getLayer(doc.activeLayerId);
      } catch {
        return;
      }
      rgba = unpackRGBA(layer.store.getPixel(ev.x, ev.y));
    } else {
      rgba = samplePixel(doc, ev.x, ev.y);
    }
    if (!rgba || rgba[3] === 0) {
      this.session.notify("info", "투명 픽셀");
      return;
    }
    const hex = toHex(rgba[0], rgba[1], rgba[2]);
    const secondary = ev.shift === true || this._btn === 2;
    const key = secondary ? "secondaryColor" : "primaryColor";
    this.session.setSetting(key, hex);
  }

  // <META - ROLE : begin sampling on any accepted button, latching it for the drag | L90-97>
  pointerDown(ev) {
    if (!ev) return;
    if (!this.acceptsButton(ev.button ?? 0)) return;
    this._down = true;
    this._btn = Number(ev.button);
    this._pos = { x: ev.x, y: ev.y };
    this._pick(ev);
  }

  pointerMove(ev) {
    if (!this._down) return;
    this._pos = { x: ev.x, y: ev.y };
    this._pick(ev);
  }

  pointerUp(_ev) {
    this._release();
  }

  hover(ev) {
    this._pos = ev ? { x: ev.x, y: ev.y } : null;
    try {
      this.requestRender();
    } catch {
      // ignore
    }
  }

  overlay(ctx, info) {
    if (!ctx || !info || !this._pos) return;
    const view = info.view;
    if (!view) return;
    const dpr = info.dpr ?? 1;
    const toDevice = info.pixelToDevice;
    const p = typeof toDevice === "function" ? toDevice(this._pos.x, this._pos.y) : null;
    if (!p) return;
    const w = Math.max(1, view.zoom * dpr);
    try {
      ctx.save?.();
      ctx.globalCompositeOperation = "difference";
      ctx.strokeStyle = "rgba(255,255,255,1)";
      ctx.lineWidth = 1;
      if (typeof ctx.strokeRect === "function") ctx.strokeRect(p.x + 0.5, p.y + 0.5, w, w);
      ctx.globalCompositeOperation = "source-over";
    } finally {
      try {
        ctx.restore?.();
      } catch {
        // ignore
      }
    }
  }
}
