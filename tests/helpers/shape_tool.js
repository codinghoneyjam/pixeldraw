// <META - FILE SUMMARY - Shared fixtures for the ShapeTool test files (events, setup, fake writer)>

import { EVENTS } from "../../src/core/events.js";
import { packRGBA } from "../../src/core/pixel.js";
import { Session } from "../../src/features/document/session.js";
import { ShapeTool } from "../../src/features/shape/shape.js";

export const P = "#ff0000";
export const S = "#0000ff";
export const PP = packRGBA(255, 0, 0, 255);
export const SS = packRGBA(0, 0, 255, 255);

// <META - ROLE : synthetic ToolEvent with every field the tool reads | L16-24>
export function ev(x, y, extra = {}) {
  return {
    x, y, fx: x, fy: y, sx: x, sy: y, button: 0,
    shift: false, ctrl: false, alt: false,
    pointerId: 1, pointerType: "mouse",
    coalesced: [{ x, y }], timeStamp: 0, ...extra,
  };
}

// <META - ROLE : synthetic key event | L26-28>
export function key(k, extra = {}) {
  return { key: k, shift: false, ctrl: false, alt: false, ...extra };
}

// <META - ROLE : session + ShapeTool pair with the render counter exposed | L30-38>
export function setup(kind = "rect", w = 64, h = 64) {
  const session = new Session();
  session.newDocument({ widthPx: w, heightPx: h });
  session.setSetting("primaryColor", P);
  session.setSetting("secondaryColor", S);
  let renders = 0;
  const tool = new ShapeTool({ session, getView: () => ({ zoom: 1, offsetX: 0, offsetY: 0 }), requestRender: () => renders++ }, kind);
  return { session, tool, renders: () => renders };
}

// <META - ROLE : down/move/up triple over one shape | L40-44>
export function drag(tool, x0, y0, x1, y1, extra = {}) {
  tool.pointerDown(ev(x0, y0, extra));
  tool.pointerMove(ev(x1, y1, extra));
  tool.pointerUp(ev(x1, y1, extra));
}

// <META - ROLE : read one stored pixel | L46-48>
export function pixel(session, layerId, x, y) {
  return session.doc.getLayer(layerId).store.getPixel(x, y);
}

// <META - ROLE : collect STATUS_MESSAGE details | L50-53>
export function statuses(session) {
  const out = [];
  session.addEventListener(EVENTS.STATUS_MESSAGE, (e) => out.push(e.detail));
  return out;
}

// <META - ROLE : collect TOOL_STATE details | L55-58>
export function toolStates(session) {
  const out = [];
  session.addEventListener(EVENTS.TOOL_STATE, (e) => out.push(e.detail));
  return out;
}

// <META - ROLE : in-memory PixelWriter stand-in for preview comparisons | L60-63>
export function fakeWriter(W, H) {
  const grid = Array.from({ length: H }, () => new Array(W).fill(0));
  return { grid, set(x, y, v) { if (x >= 0 && y >= 0 && x < W && y < H) grid[y][x] = v; } };
}