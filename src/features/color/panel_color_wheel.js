// <META - FILE SUMMARY - Hue ring + centered SV square: HSV math, raster cache, pointer pick, mount>
import { parseHex, toHex } from "../../core/pixel.js";

export const WHEEL_PX = 132;
export const RING_OUT = 66;
export const RING_IN = 55.5;
export const RING_MID = 60.75;
/** Minimum px of bare ring between the SV square's corner and RING_IN. */
export const RING_CLEARANCE = 3;
/** Largest square that fits inside the ring hole with RING_CLEARANCE on every side. */
export const SV_PX = Math.floor((2 * (RING_IN - RING_CLEARANCE)) / Math.SQRT2);

// <META - ROLE : sRGB -> HSV (pure) | L13-29>
export function rgbToHsv(r, g, b) {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const mx = Math.max(rn, gn, bn);
  const mn = Math.min(rn, gn, bn);
  const d = mx - mn;
  let h = 0;
  if (d > 0) {
    if (mx === rn) h = 60 * (((gn - bn) / d) % 6);
    else if (mx === gn) h = 60 * ((bn - rn) / d + 2);
    else h = 60 * ((rn - gn) / d + 4);
  }
  if (h < 0) h += 360;
  return { h, s: mx === 0 ? 0 : d / mx, v: mx };
}

// <META - ROLE : HSV -> sRGB triple (pure) | L31-40>
export function hsvToRgb(h, s, v) {
  const hue = ((h % 360) + 360) % 360;
  const c = v * s;
  const x = c * (1 - Math.abs(((hue / 60) % 2) - 1));
  const m = v - c;
  const sectors = [[c, x, 0], [x, c, 0], [0, c, x], [0, x, c], [x, 0, c], [c, 0, x]];
  const sec = sectors[Math.min(5, Math.floor(hue / 60))];
  return [Math.round((sec[0] + m) * 255), Math.round((sec[1] + m) * 255), Math.round((sec[2] + m) * 255)];
}

// <META - ROLE : Pointer offset -> hue degrees (pure) | L42-45>
export function angleToHue(dx, dy) {
  return (Math.atan2(dy, dx) * 180 / Math.PI + 450) % 360;
}

/**
 * @param {number} h hue degrees
 * @param {number} s 0..1
 * @param {number} v 0..1
 * @returns {string} "#rrggbb"
 */
export function hsvToHex(h, s, v) {
  const [r, g, b] = hsvToRgb(h, s, v);
  return toHex(r, g, b);
}

// <META - ROLE : True when an offset lands on the ring band rather than inside the hole | L58-61>
function onRing(dx, dy) {
  return Math.hypot(dx, dy) >= RING_IN - 6;
}

// <META - ROLE : Fill a square canvas via per-pixel callback | L63-78>
function paintPixels(ctx, size, fn) {
  const img = ctx.createImageData(size, size);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const px = fn(x, y);
      if (!px) continue;
      const i = (y * size + x) * 4;
      img.data[i] = px[0];
      img.data[i + 1] = px[1];
      img.data[i + 2] = px[2];
      img.data[i + 3] = 255;
    }
  }
  ctx.putImageData(img, 0, 0);
}

// <META - ROLE : Find or create a canvas inside the wrap, track created nodes | L80-94>
// Sizes are set as canvas ATTRIBUTES only. No inline CSS is written here: the
// color-picker tier (style_picker.css) owns position/size/centring so there is
// a single owner of the picker layout.
function resolveCanvas(container, id, size, label, created) {
  const found = container.querySelector(`#${id}`);
  if (found) return found;
  const cv = document.createElement("canvas");
  cv.id = id;
  cv.width = size;
  cv.height = size;
  cv.setAttribute("aria-label", label);
  created.push(cv);
  container.append(cv);
  return cv;
}

// <META - ROLE : Pointer drag binding: capture first, whole gesture tracked by the flag | L96-121>
// `pick` fires continuously while dragging (that is the live preview); `commit` fires
// EXACTLY ONCE when the gesture ends. Callers must record a recent-colour entry only
// from `commit` — recording from `pick` files every colour the pointer merely swept
// across, which is not a deliberate choice.
function bindDrag(canvas, pick, commit, disposers) {
  const flag = { on: false };
  const listen = (type, fn) => {
    canvas.addEventListener(type, fn);
    disposers.push(() => canvas.removeEventListener(type, fn));
  };
  listen("pointerdown", (e) => {
    if (e.button !== undefined && e.button !== 0) return;
    flag.on = true;
    try { canvas.setPointerCapture(e.pointerId); } catch { /* ignore */ }
    pick(e);
  });
  listen("pointermove", (e) => { if (flag.on) pick(e); });
  for (const type of ["pointerup", "pointercancel", "lostpointercapture"]) {
    listen(type, () => {
      if (!flag.on) return;
      flag.on = false;
      try { commit(); } catch { /* ignore */ }
    });
  }
  listen("contextmenu", (e) => e.preventDefault());
}

// <META - ROLE : Cache the hue ring raster once, then blit it each repaint | L116-142>
function createRingRenderer(canvas) {
  const ctx = canvas.getContext("2d");
  const base = document.createElement("canvas");
  base.width = WHEEL_PX;
  base.height = WHEEL_PX;
  paintPixels(base.getContext("2d"), WHEEL_PX, (x, y) => {
    const dx = x - WHEEL_PX / 2 + 0.5;
    const dy = y - WHEEL_PX / 2 + 0.5;
    const r = Math.hypot(dx, dy);
    if (r > RING_OUT || r < RING_IN) return null;
    return hsvToRgb(angleToHue(dx, dy), 1, 1);
  });
  return function paintRing(state) {
    if (!ctx) return;
    ctx.clearRect(0, 0, WHEEL_PX, WHEEL_PX);
    ctx.drawImage(base, 0, 0);
    const a = ((state.h - 90) * Math.PI) / 180;
    ctx.beginPath();
    ctx.arc(WHEEL_PX / 2 + Math.cos(a) * RING_MID, WHEEL_PX / 2 + Math.sin(a) * RING_MID, 7, 0, Math.PI * 2);
    ctx.fillStyle = `hsl(${state.h},100%,50%)`;
    ctx.fill();
    ctx.lineWidth = 2;
    ctx.strokeStyle = "#ffffff";
    ctx.stroke();
  };
}

// <META - ROLE : Recache the SV gradient only when the hue changes | L144-168>
function createSvRenderer(canvas) {
  const ctx = canvas.getContext("2d");
  const base = document.createElement("canvas");
  base.width = SV_PX;
  base.height = SV_PX;
  const span = Math.max(1, SV_PX - 1);
  let baseHue = -1;
  function ensureBase(hue) {
    if (baseHue === hue) return;
    paintPixels(base.getContext("2d"), SV_PX, (x, y) => hsvToRgb(hue, x / span, 1 - y / span));
    baseHue = hue;
  }
  return function paintSv(state) {
    if (!ctx) return;
    ensureBase(state.h);
    ctx.clearRect(0, 0, SV_PX, SV_PX);
    ctx.drawImage(base, 0, 0);
    ctx.beginPath();
    ctx.arc(state.s * span, (1 - state.v) * span, 5, 0, Math.PI * 2);
    ctx.lineWidth = 2;
    ctx.strokeStyle = "#ffffff";
    ctx.stroke();
  };
}

// <META - ROLE : Pointer offset -> SV in 0..1, corners reachable and clamped | L170-175>
function pointerToSv(rect, e) {
  const s = rect.width > 0 ? Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width)) : 0;
  const v = rect.height > 0 ? 1 - Math.min(1, Math.max(0, (e.clientY - rect.top) / rect.height)) : 0;
  return { s, v };
}

// <META - ROLE : Pointer offset from the ring centre in canvas pixels (pure) | L177-184>
function offsetInRing(rect, e) {
  const scale = rect.width > 0 ? WHEEL_PX / rect.width : 1;
  return {
    dx: (e.clientX - (rect.left + rect.width / 2)) * scale,
    dy: (e.clientY - (rect.top + rect.height / 2)) * scale,
  };
}

/**
 * @param {{wheelCanvas: HTMLCanvasElement, svCanvas: HTMLCanvasElement}} c
 */
// <META - ROLE : HSV state owner + the two pointer pick helpers | L189-227>
function createPainter(c) {
  const state = { h: 0, s: 1, v: 1 };
  const paintRing = createRingRenderer(c.wheelCanvas);
  const paintSv = createSvRenderer(c.svCanvas);

  function setHueSV(h, s, v) {
    if (s > 0.02 && v > 0.02 && Number.isFinite(h)) state.h = ((h % 360) + 360) % 360;
    state.s = Math.min(1, Math.max(0, s));
    state.v = Math.min(1, Math.max(0, v));
    paintRing(state);
    paintSv(state);
  }

  function pickHue(e, canvas) {
    const { dx, dy } = offsetInRing(canvas.getBoundingClientRect(), e);
    if (!onRing(dx, dy)) return null;
    setHueSV(angleToHue(dx, dy), state.s, state.v);
    return hsvToHex(state.h, state.s, state.v);
  }

  function pickSatVal(e, canvas) {
    const { s, v } = pointerToSv(canvas.getBoundingClientRect(), e);
    setHueSV(state.h, s, v);
    return hsvToHex(state.h, state.s, state.v);
  }

  setHueSV(state.h, state.s, state.v);
  return {
    setHueSV,
    pickHue,
    pickSatVal,
    /** @param {{r:number,g:number,b:number}} rgb */
    setFromRgb(rgb) {
      const hsv = rgbToHsv(rgb.r, rgb.g, rgb.b);
      setHueSV(hsv.h, hsv.s, hsv.v);
    },
  };
}

// <META - ROLE : Factory: canvases, drags, dispose. Layout belongs to style_picker.css | L229-260>
export function createColorWheel(options = {}) {
  const container = options.container ?? null;
  if (!container || typeof document === "undefined") return null;
  const onChange = typeof options.onChange === "function" ? options.onChange : () => {};
  const onCommit = typeof options.onCommit === "function" ? options.onCommit : () => {};
  const disposers = [];
  const created = [];
  const wheelCanvas = resolveCanvas(container, "dt-hue-wheel", WHEEL_PX, "색상환", created);
  const svCanvas = resolveCanvas(container, "dt-sv-square", SV_PX, "채도 명도", created);
  const painter = createPainter({ wheelCanvas, svCanvas });
  bindDrag(wheelCanvas, (e) => {
    const hex = painter.pickHue(e, wheelCanvas);
    if (hex) onChange(hex);
  }, onCommit, disposers);
  bindDrag(svCanvas, (e) => onChange(painter.pickSatVal(e, svCanvas)), onCommit, disposers);
  return {
    wheelCanvas,
    svCanvas,
    setHueSV: painter.setHueSV,
    setFromRgb: painter.setFromRgb,
    dispose() {
      for (const d of disposers) {
        try { d(); } catch { /* ignore */ }
      }
      disposers.length = 0;
      for (const node of created) {
        try { node.remove(); } catch { /* ignore */ }
      }
      created.length = 0;
    },
  };
}

// <META - ROLE : Re-attach the active slot alpha to an opaque wheel pick | L271-279>
function withSlotAlpha(ui, hex) {
  if (typeof hex !== "string" || !hex.startsWith("#")) return hex;
  let a = 255;
  try {
    const cur = ui.session ? ui.session.settings[ui.slot] : null;
    a = parseHex(cur)?.a ?? 255;
  } catch {
    a = 255;
  }
  if (a === 255) return hex;
  return `${hex}${a.toString(16).padStart(2, "0")}`;
}

// <META - ROLE : Hue ring + SV square; the wheel is a view fed only by paint() | L229-272>
// Wheel picks are opaque by construction, so the current slot alpha is
// re-attached here instead of resetting translucency on every drag.
export function mountWheel(ui) {
  ui.wheel = createColorWheel({
    container: ui.els.wheelWrap,
    onChange: (hex) => ui.safe(() => ui.applyColor(ui.slot, withSlotAlpha(ui, hex), { record: false })),
    onCommit: () => ui.safe(() => ui.pushRecent(ui.canonical())),
  });
  return () => ui.wheel?.dispose();
}