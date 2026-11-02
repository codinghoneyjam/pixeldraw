// Usage: node tools/png_unique_colors.mjs <file.png> [maxColors]
// Anti-aliasing detector for exported PNGs: counts distinct RGBA values and flags partial alpha.
import { readFileSync } from "node:fs";
import { decodePng } from "../reference/png_ref.mjs";
const [file, max = "Infinity"] = process.argv.slice(2);
if (!file) { console.error("usage: node tools/png_unique_colors.mjs <file.png> [maxColors]"); process.exit(2); }
const { width, height, rgba } = await decodePng(new Uint8Array(readFileSync(file)));
const seen = new Map(); let partialAlpha = 0;
for (let i = 0; i < rgba.length; i += 4) {
  const a = rgba[i + 3]; if (a !== 0 && a !== 255) partialAlpha++;
  const key = (rgba[i] << 24 | rgba[i + 1] << 16 | rgba[i + 2] << 8 | a) >>> 0; seen.set(key, (seen.get(key) ?? 0) + 1);
}
const colors = [...seen.entries()].map(([k, n]) => ({ rgba: [k >>> 24, (k >>> 16) & 255, (k >>> 8) & 255, k & 255], n }));
console.log(JSON.stringify({ width, height, uniqueColors: colors.length, partialAlphaPixels: partialAlpha, colors: colors.slice(0, 16) }));
if (partialAlpha > 0 || colors.length > Number(max)) { console.error("FAIL: anti-aliasing suspected"); process.exit(1); }
