// <META - FILE SUMMARY - Recipe color token resolution: $tokens, body_* derivations, hex/array parse>
//
// Pure module: no DOM, ESM, Node-importable. Reuses packRGBA/parseHex from src/core/pixel.js.
// NOTE: kept as .js (not .mjs) on purpose — this module ships in the browser
// import graph via tools/recipe/render_tile.js, and Windows' mimetypes registry
// maps .mjs to text/plain which strict ESM checking rejects.

import { packRGBA, parseHex } from "../../src/core/pixel.js";

const clamp255 = (v) => {
  const n = Math.trunc(Number(v));
  if (!Number.isFinite(n)) return 0;
  return n < 0 ? 0 : n > 255 ? 255 : n;
};

// <META - ROLE : Parse hex string / [r,g,b(,a)] into [r,g,b,a] or 0 for transparent | L18-38>
export function parseColor(raw) {
  if (raw === null || raw === undefined) return 0;
  if (Array.isArray(raw)) {
    if (raw.length < 3) return 0;
    const a = raw.length > 3 ? raw[3] : 255;
    return [clamp255(raw[0]), clamp255(raw[1]), clamp255(raw[2]), clamp255(a)];
  }
  if (typeof raw === "string") {
    const s = raw.trim();
    if (s === "" || s.toLowerCase() === "none") return 0;
    const parsed = parseHex(s);
    return parsed ? [parsed.r, parsed.g, parsed.b, parsed.a] : 0;
  }
  return 0;
}

// <META - ROLE : Resolve $token/palette name/body_* derivation to raw color value | L41-78>
export function resolveColorToken(token, palette, bodyHex) {
  if (Array.isArray(token)) return token;
  if (typeof token !== "string") return token;
  const s = token.trim();
  if (s === "" || s.toLowerCase() === "none") return s;

  const pal = palette ?? {};
  const body = bodyHex ?? pal["body_color"] ?? pal["body_color_hex"] ?? pal["body"] ?? pal["$body_color"] ?? pal["$body_color_hex"] ?? pal["$body"];
  const name = s.startsWith("$") ? s.slice(1) : s;

  if (name === "body_outline") return [0, 0, 0, 255];
  if (name === "body_dark" || name === "body_deep" || name === "body_light" || name === "body_bright") {
    const factor = name === "body_dark" ? 0.6 : name === "body_deep" ? 0.22 : 1.25;
    const parsed = parseColor(body ?? "#808080");
    const base = parsed || [128, 128, 128, 255];
    return [clamp255(base[0] * factor), clamp255(base[1] * factor), clamp255(base[2] * factor), base[3]];
  }

  if (s.startsWith("$")) {
    const v = pal[`$${name}`] ?? pal[name] ?? pal[`$${name}_hex`] ?? pal[`${name}_hex`];
    // Legacy fallback (geometry.resolve_color): unknown tokens resolve to
    // white, except body_color which falls back to mid gray #808080.
    if (v === undefined) return name === "body_color" ? "#808080" : "#FFFFFF";
    if (typeof v === "string" && v.trim().startsWith("$")) return resolveColorToken(v, pal, body);
    return v;
  }

  // Non-$ strings are palette names or literal colors; literals pass through
  // to parseColor (hex/rgb), unknowns resolve like legacy ($-less lookup).
  const v = pal[s] ?? pal[`$${s}`];
  if (v !== undefined) {
    if (typeof v === "string" && v.trim().startsWith("$")) return resolveColorToken(v, pal, body);
    return v;
  }
  return s;
}

// <META - ROLE : Resolve token/hex/array through palette then pack to u32 | L81-89>
export function resolveColor(raw, palette) {
  if (raw === null || raw === undefined || raw === "") return 0;
  const resolved = resolveColorToken(raw, palette);
  const parsed = parseColor(resolved);
  if (!parsed) return 0;
  return packRGBA(parsed[0], parsed[1], parsed[2], parsed[3]);
}
