// <META - FILE SUMMARY - localStorage JSON helpers + settings/recent-colors persistence>
import { EVENTS } from "../../core/events.js";

export const SETTINGS_KEY = "dt.settings.v1";
export const RECENT_KEY = "dt.recentColors";
export const PERSIST_KEYS = ["primaryColor", "secondaryColor", "penSize", "gridMode", "snapUnit", "shapeFill", "shapeRadius", "shapeLockAspect", "activeTool"];
export const RECENT_MAX = 32;

export function loadJson(key) {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return null;
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export function storeJson(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch { /* ignore */ }
}

// <META - ROLE : Apply persisted settings onto a fresh session | L30-42>
export function restoreSettings(session) {
  const saved = loadJson(SETTINGS_KEY);
  if (saved && typeof saved === "object") {
    for (const k of PERSIST_KEYS) {
      if (saved[k] !== undefined) {
        try { session.setSetting(k, saved[k]); } catch { /* ignore corrupt */ }
      }
    }
  }
}

// <META - ROLE : Load persisted recent colors as a string list | L45-51>
export function loadRecentColors() {
  const savedRecent = loadJson(RECENT_KEY);
  if (Array.isArray(savedRecent)) return savedRecent.filter((c) => typeof c === "string").slice(0, RECENT_MAX);
  return [];
}

// <META - ROLE : Debounced settings persist on SETTINGS_CHANGED | L54-69>
export function attachSettingsPersist(session) {
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
}
