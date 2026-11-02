import { loadJson, storeJson } from "./storage.js";

const STORAGE_KEY = "dt.panelWidths.v1";
const MAX_PANEL_WIDTH = 440;
const GUTTER_WIDTH = 16;

export function clampPanelWidth(value, min, max) {
  return Math.round(Math.min(max, Math.max(min, Number(value) || min)));
}

export function panelWidthAtPointer(side, startWidth, startX, currentX) {
  return startWidth + (side === "left" ? currentX - startX : startX - currentX);
}

function minWidthFor(side) {
  const narrow = window.innerWidth <= 720;
  const tiny = window.innerWidth <= 360;
  return tiny ? 96 : narrow ? 130 : side === "left" ? 180 : 200;
}

function canvasMinWidth() {
  return window.innerWidth <= 360 ? 40 : window.innerWidth <= 720 ? 48 : 220;
}

function limitsFor(side, app, panels, toolbar) {
  const otherSide = side === "left" ? "right" : "left";
  const min = minWidthFor(side);
  const available = app.clientWidth - toolbar.getBoundingClientRect().width
    - minWidthFor(otherSide) - canvasMinWidth() - GUTTER_WIDTH;
  return { min, max: Math.max(min, Math.min(MAX_PANEL_WIDTH, available)) };
}

export function mountPanelResize(app) {
  if (!app || typeof window === "undefined" || typeof document === "undefined") return () => {};
  const panels = {
    left: app.querySelector("#dt-left-panels"),
    right: app.querySelector("#dt-right-panels"),
  };
  const handles = {
    left: app.querySelector("#dt-left-resize"),
    right: app.querySelector("#dt-right-resize"),
  };
  const toolbar = app.querySelector("#dt-toolbar");
  const canvas = app.querySelector("#dt-canvas-host");
  if (!panels.left || !panels.right || !handles.left || !handles.right || !toolbar || !canvas) return () => {};

  const explicit = new Set();
  const disposers = [];
  const listen = (target, type, fn) => {
    target.addEventListener(type, fn);
    disposers.push(() => target.removeEventListener(type, fn));
  };
  const propertyFor = (side) => side === "left" ? "--panel-w" : "--right-w";

  function setWidth(side, value, preferredOtherWidth = null) {
    const { min, max } = limitsFor(side, app, panels, toolbar);
    const width = clampPanelWidth(value, min, max);
    app.style.setProperty(propertyFor(side), `${width}px`);
    const otherSide = side === "left" ? "right" : "left";
    const currentOtherWidth = panels[otherSide].getBoundingClientRect().width;
    const otherWidthLimit = preferredOtherWidth ?? currentOtherWidth;
    const totalPanelCapacity = app.clientWidth - toolbar.getBoundingClientRect().width
      - canvasMinWidth() - GUTTER_WIDTH;
    const nextOtherWidth = clampPanelWidth(
      Math.min(otherWidthLimit, totalPanelCapacity - width),
      minWidthFor(otherSide),
      otherWidthLimit,
    );
    if (Math.abs(nextOtherWidth - currentOtherWidth) >= 1) {
      explicit.add(otherSide);
      app.style.setProperty(propertyFor(otherSide), `${nextOtherWidth}px`);
      syncValue(otherSide);
    }
    handles[side].setAttribute("aria-valuemin", String(min));
    handles[side].setAttribute("aria-valuemax", String(max));
    handles[side].setAttribute("aria-valuenow", String(width));
    handles[side].setAttribute("aria-valuetext", `${width}px`);
    syncValue(side);
    return width;
  }

  function syncValue(side) {
    const width = panels[side].getBoundingClientRect().width;
    const { min, max } = limitsFor(side, app, panels, toolbar);
    handles[side].setAttribute("aria-valuemin", String(min));
    handles[side].setAttribute("aria-valuemax", String(max));
    handles[side].setAttribute("aria-valuenow", String(Math.round(width)));
    handles[side].setAttribute("aria-valuetext", `${Math.round(width)}px`);
  }

  function persist() {
    const widths = {};
    for (const side of explicit) widths[side] = Math.round(panels[side].getBoundingClientRect().width);
    storeJson(STORAGE_KEY, widths);
  }

  function finish(handle, state) {
    if (!state.active) return;
    state.active = false;
    handle.classList.remove("is-dragging");
    document.body.classList.remove("dt-panel-resizing");
    persist();
  }

  for (const side of ["left", "right"]) {
    const handle = handles[side];
    const state = { active: false, pointerId: null, startX: 0, startWidth: 0, startOtherWidth: 0 };
    listen(handle, "pointerdown", (event) => {
      if (event.button !== 0) return;
      state.active = true;
      state.pointerId = event.pointerId;
      state.startX = event.clientX;
      state.startWidth = panels[side].getBoundingClientRect().width;
      state.startOtherWidth = panels[side === "left" ? "right" : "left"].getBoundingClientRect().width;
      explicit.add(side);
      handle.classList.add("is-dragging");
      document.body.classList.add("dt-panel-resizing");
      try { handle.setPointerCapture(event.pointerId); } catch { /* ignore */ }
      event.preventDefault();
    });
    listen(handle, "pointermove", (event) => {
      if (!state.active || event.pointerId !== state.pointerId) return;
      setWidth(side, panelWidthAtPointer(side, state.startWidth, state.startX, event.clientX), state.startOtherWidth);
    });
    for (const type of ["pointerup", "pointercancel", "lostpointercapture"]) {
      listen(handle, type, () => finish(handle, state));
    }
    listen(handle, "keydown", (event) => {
      const current = panels[side].getBoundingClientRect().width;
      let next;
      if (event.key === "ArrowLeft") next = current - (side === "left" ? 10 : -10);
      else if (event.key === "ArrowRight") next = current + (side === "left" ? 10 : -10);
      else if (event.key === "Home") next = limitsFor(side, app, panels, toolbar).min;
      else if (event.key === "End") next = limitsFor(side, app, panels, toolbar).max;
      else return;
      event.preventDefault();
      explicit.add(side);
      setWidth(side, next);
      persist();
    });
    syncValue(side);
  }

  const saved = loadJson(STORAGE_KEY);
  if (saved && typeof saved === "object") {
    for (const side of ["left", "right"]) {
      if (!Number.isFinite(saved[side])) continue;
      explicit.add(side);
      setWidth(side, saved[side]);
    }
  }

  const onResize = () => {
    for (const side of ["left", "right"]) {
      if (explicit.has(side)) setWidth(side, panels[side].getBoundingClientRect().width);
      else syncValue(side);
    }
  };
  listen(window, "resize", onResize);
  return () => {
    for (const dispose of disposers) dispose();
    document.body.classList.remove("dt-panel-resizing");
  };
}