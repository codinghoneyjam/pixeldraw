// <META - FILE SUMMARY - Baked glyph set loader + string layout (deterministic text)>
//
// Node-side companion to text_mask.js: reads the committed glyph bitmap JSON
// (baked once from real fonts) and lays out strings into per-char placements.
// Rasterization itself never touches a font file (contract invariant 8).

import fs from "node:fs";

function decodeAlpha(b64, w, h) {
  const bytes = Buffer.from(b64, "base64");
  if (bytes.length !== w * h) {
    throw new RangeError(`glyph alpha size ${bytes.length} != ${w}x${h}`);
  }
  return bytes;
}

// <META - ROLE : Load committed glyph JSON into runtime glyph sets | L14-30>
export function loadGlyphSet(jsonPath) {
  const raw = JSON.parse(fs.readFileSync(jsonPath, "utf-8"));
  const fonts = {};
  for (const [name, font] of Object.entries(raw.fonts)) {
    const glyphs = {};
    for (const [ch, g] of Object.entries(font.glyphs)) {
      const shaped = {};
      for (const [sw, s] of Object.entries(g.shaped ?? {})) {
        shaped[sw] = {
          dx: s.dx, dy: s.dy, w: s.w, h: s.h,
          alpha: decodeAlpha(s.alpha_b64, s.w, s.h),
        };
      }
      glyphs[ch] = {
        advance: g.advance,
        dx: g.dx,
        dy: g.dy,
        w: g.w,
        h: g.h,
        alpha: decodeAlpha(g.alpha_b64, g.w, g.h),
        shaped,
      };
    }
    fonts[name] = { path: font.path, size: font.size, glyphs };
  }
  return fonts;
}
