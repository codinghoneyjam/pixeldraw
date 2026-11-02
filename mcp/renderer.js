/**
 * PixelDraw Headless Renderer & PNG Exporter
 */

import path from "node:path";
import fs from "node:fs";
import { ChunkStore, PixelWriter } from "../web/src/core/chunkstore.js";
import { applyShape } from "../web/src/core/raster/shape_raster.js";
import { parseHex, packRGBA, TRANSPARENT } from "../web/src/core/pixel.js";
import { encodePng } from "../web/src/io/png.js";
import { parseQuery } from "./parser.js";

function colorToPacked(c) {
  if (!c || c === "none" || c === "transparent") return TRANSPARENT;
  if (typeof c === "string") {
    const parsed = parseHex(c);
    if (!parsed) return TRANSPARENT;
    return packRGBA(parsed.r, parsed.g, parsed.b, parsed.a);
  }
  if (Array.isArray(c)) {
    const a = c.length > 3 ? c[3] : 255;
    return packRGBA(c[0], c[1], c[2], a);
  }
  return TRANSPARENT;
}

export async function renderQuery({ query, outputPath, width, height, background }) {
  const { canvas, commands } = parseQuery(query, { width, height, background });
  const w = canvas.w;
  const h = canvas.h;

  const store = new ChunkStore(w, h);
  const writer = new PixelWriter(store, "layer_0");

  // Paint background if needed
  const bgPacked = colorToPacked(canvas.bg);
  if (bgPacked !== TRANSPARENT) {
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        writer.set(x, y, bgPacked);
      }
    }
  }

  // Execute commands in order
  for (const cmd of commands) {
    const fillCol = colorToPacked(cmd.fill);
    const outlineCol = colorToPacked(cmd.outline);
    const sw = cmd.strokeWidth ?? 1;

    let fillMode = "fill";
    if (fillCol !== TRANSPARENT && outlineCol !== TRANSPARENT) {
      fillMode = "both";
    } else if (outlineCol !== TRANSPARENT && fillCol === TRANSPARENT) {
      fillMode = "outline";
    }

    if (cmd.type === "ellipse" || cmd.type === "circle") {
      const primary = fillMode === "outline" ? outlineCol : fillCol;
      const secondary = fillCol;
      applyShape(writer, {
        kind: "ellipse",
        bbox: cmd.bbox,
        strokeWidth: sw,
        fillMode,
      }, primary, secondary);
    } else if (cmd.type === "rect" || cmd.type === "rrect") {
      const primary = fillMode === "outline" ? outlineCol : fillCol;
      const secondary = fillCol;
      applyShape(writer, {
        kind: "rrect",
        bbox: cmd.bbox,
        radius: cmd.radius ?? 0,
        strokeWidth: sw,
        fillMode,
      }, primary, secondary);
    } else if (cmd.type === "line") {
      const col = colorToPacked(cmd.color);
      applyShape(writer, {
        kind: "line",
        p0: cmd.p0,
        p1: cmd.p1,
        strokeWidth: sw,
      }, col, 0);
    } else if (cmd.type === "polygon") {
      const primary = fillMode === "outline" ? outlineCol : fillCol;
      const secondary = fillCol;
      applyShape(writer, {
        kind: "polygon",
        points: cmd.points,
        strokeWidth: sw,
        fillMode,
      }, primary, secondary);
    } else if (cmd.type === "pixel") {
      const col = colorToPacked(cmd.color);
      writer.set(cmd.x, cmd.y, col);
    }
  }

  writer.finish();

  // Extract pixel buffer
  const pixelBuffer = new Uint32Array(w * h);
  let nonZeroCount = 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const p = store.getPixel(x, y);
      pixelBuffer[y * w + x] = p;
      if (p !== 0) nonZeroCount++;
    }
  }

  const clampedBytes = new Uint8ClampedArray(pixelBuffer.buffer);
  const pngBytes = await encodePng(clampedBytes, w, h);

  if (outputPath) {
    const resolvedPath = path.resolve(process.cwd(), outputPath);
    const targetDir = path.dirname(resolvedPath);
    if (!fs.existsSync(targetDir)) {
      fs.mkdirSync(targetDir, { recursive: true });
    }
    fs.writeFileSync(resolvedPath, pngBytes);
    return {
      success: true,
      outputPath: resolvedPath,
      width: w,
      height: h,
      fileSize: pngBytes.byteLength,
      drawnPixels: nonZeroCount,
    };
  }

  return {
    success: true,
    width: w,
    height: h,
    pngBytes,
    fileSize: pngBytes.byteLength,
    drawnPixels: nonZeroCount,
  };
}
