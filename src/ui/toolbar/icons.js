// <META - FILE SUMMARY - Inline SVG icons with Korean-char fallback (G1)>
const svg = (body) => `<svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`;

export const ICONS = Object.freeze({
  pen: svg('<path d="M4 20l1-4L16 5l3 3L8 19l-4 1z"/><path d="M14 7l3 3"/>'),
  eraser: svg('<path d="M7 21l-4-4L13 7l4 4-7 7H7z"/><path d="M11 21h10"/>'),
  fill: svg('<path d="M5 11l8-8 6 6-8 8H7l-2-2z"/><path d="M5 15l-2 2 4 4 2-2"/><path d="M19 15s2 2.5 2 4.5a2 2 0 0 1-4 0c0-2 2-4.5 2-4.5z"/>'),
  eyedropper: svg('<path d="M14 4l6 6-9 9H5v-6l9-9z"/><path d="M14 4l2-2 6 6-2 2"/><path d="M4 20h7"/>'),
  line: svg('<path d="M5 19L19 5"/>'),
  rect: svg('<rect x="4" y="6" width="16" height="12"/>'),
  rrect: svg('<rect x="4" y="6" width="16" height="12" rx="4"/>'),
  ellipse: svg('<ellipse cx="12" cy="12" rx="8" ry="6"/>'),
  hand: svg('<path d="M8 12V5a1.5 1.5 0 0 1 3 0v6V4a1.5 1.5 0 0 1 3 0v7V6a1.5 1.5 0 0 1 3 0v8a7 7 0 0 1-7 7h-1a7 7 0 0 1-6-3l-2-4a1.5 1.5 0 0 1 2.5-1.5L8 14z"/>'),
  eye: svg('<path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6-10-6-10-6z"/><circle cx="12" cy="12" r="2.5"/>'),
  eyeOff: svg('<path d="M3 3l18 18"/><path d="M10 6c.7-.1 1.3-.1 2-.1 6.5 0 10 6 10 6a17 17 0 0 1-3 3.5M6 7A16 16 0 0 0 2 12s3.5 6 10 6c1.5 0 2.8-.3 4-.8"/>'),
  lock: svg('<rect x="5" y="11" width="14" height="9"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/>'),
  unlock: svg('<rect x="5" y="11" width="14" height="9"/><path d="M8 11V8a4 4 0 0 1 7.5-2"/>'),
  swap: svg('<path d="M7 4L3 8l4 4"/><path d="M3 8h14"/><path d="M17 12l4 4-4 4"/><path d="M21 16H7"/>'),
  plus: svg('<path d="M12 5v14M5 12h14"/>'),
});

const FALLBACK_CHAR = Object.freeze({
  pen: "펜",
  eraser: "지",
  fill: "통",
  eyedropper: "스",
  line: "선",
  rect: "사",
  rrect: "둥",
  ellipse: "타",
  hand: "손",
});

// <META - ROLE : Return inline SVG or Korean-char fallback | L52-58>
export function iconFor(id) {
  const hit = ICONS[id];
  if (typeof hit === "string") return hit;
  return FALLBACK_CHAR[id] ?? "?";
}

export function hasIcon(id) {
  return typeof ICONS[id] === "string";
}
