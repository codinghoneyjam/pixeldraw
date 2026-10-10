function decodeAlpha(encoded, width, height) {
  const binary = atob(encoded);
  if (binary.length !== width * height) throw new RangeError("invalid glyph alpha size");
  return Uint8Array.from(binary, (char) => char.charCodeAt(0));
}

export function decodeGlyphSets(raw) {
  const fonts = {};
  for (const [name, font] of Object.entries(raw?.fonts ?? {})) {
    const glyphs = {};
    for (const [char, glyph] of Object.entries(font.glyphs ?? {})) {
      const shaped = {};
      for (const [strokeWidth, shape] of Object.entries(glyph.shaped ?? {})) {
        shaped[strokeWidth] = {
          dx: shape.dx,
          dy: shape.dy,
          w: shape.w,
          h: shape.h,
          alpha: decodeAlpha(shape.alpha_b64, shape.w, shape.h),
        };
      }
      glyphs[char] = {
        advance: glyph.advance,
        dx: glyph.dx,
        dy: glyph.dy,
        w: glyph.w,
        h: glyph.h,
        alpha: decodeAlpha(glyph.alpha_b64, glyph.w, glyph.h),
        shaped,
      };
    }
    fonts[name] = { path: font.path, size: font.size, glyphs };
  }
  return fonts;
}