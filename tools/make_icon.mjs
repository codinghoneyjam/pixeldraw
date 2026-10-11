// <META - FILE SUMMARY - Draw the Pixeldraw app icon (1024x1024 PNG) with the editor's own codec>
//
// The icon is generated from the editor's default palette so the desktop shell
// and the in-app colour picker stay in sync. Encoding goes through src/io/png.js
// (the same codec that exports PNGs) so no image dependency is added.
//
// Run: node tools/make_icon.mjs [--out FILE]

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { encodePng } from "../src/io/png.js";
import { DEFAULT_PALETTE } from "../src/features/color/color_palette.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const SIZE = 1024;
const MARGIN = 96;   // dark frame around the swatch grid
const GUTTER = 32;   // gap between swatches
const CELLS = 6;
const CELL = (SIZE - 2 * MARGIN - (CELLS - 1) * GUTTER) / CELLS; // 112 px

const FRAME = [0x16, 0x1a, 0x26]; // deep navy, matches --dt-bg-* in style_base.css

function hexToRgb(hex) {
  return [
    parseInt(hex.slice(1, 3), 16),
    parseInt(hex.slice(3, 5), 16),
    parseInt(hex.slice(5, 7), 16),
  ];
}

// <META - ROLE : Fill the 1024x1024 buffer: frame + 6x6 palette grid | L...>
function draw() {
  const rgba = new Uint8ClampedArray(SIZE * SIZE * 4);
  for (let i = 0; i < SIZE * SIZE; i++) {
    rgba[i * 4] = FRAME[0];
    rgba[i * 4 + 1] = FRAME[1];
    rgba[i * 4 + 2] = FRAME[2];
    rgba[i * 4 + 3] = 255;
  }
  for (let row = 0; row < CELLS; row++) {
    for (let col = 0; col < CELLS; col++) {
      const rgb = hexToRgb(DEFAULT_PALETTE[row * CELLS + col] || "#ffffff");
      const x0 = MARGIN + col * (CELL + GUTTER);
      const y0 = MARGIN + row * (CELL + GUTTER);
      for (let y = 0; y < CELL; y++) {
        for (let x = 0; x < CELL; x++) {
          const i = ((y0 + y) * SIZE + (x0 + x)) * 4;
          rgba[i] = rgb[0];
          rgba[i + 1] = rgb[1];
          rgba[i + 2] = rgb[2];
          rgba[i + 3] = 255;
        }
      }
    }
  }
  return rgba;
}

const OUT_FLAG = process.argv.indexOf("--out");
const outFile = OUT_FLAG > -1 && process.argv[OUT_FLAG + 1]
  ? path.resolve(process.cwd(), process.argv[OUT_FLAG + 1])
  : path.join(__dirname, "..", "src-tauri", "icons", "source.png");

const png = await encodePng(draw(), SIZE, SIZE);
fs.mkdirSync(path.dirname(outFile), { recursive: true });
fs.writeFileSync(outFile, png);
console.log(`make_icon: ${SIZE}x${SIZE} (${(png.length / 1024).toFixed(0)} KiB) -> ${outFile}`);
