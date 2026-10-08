// <META - FILE SUMMARY - Vector recipe converter: Document <-> vector command list>
//
// Converts between Draw Tool v2 Document (raster chunks) and vector command lists.
// Each layer stores a sequence of drawing commands (rect, circle, etc.) instead
// of pixel data.
//
// Usage:
//   node tools/recipe/vector_converter.mjs --to-vector --doc <path> --out <path>
//   node tools/recipe/vector_converter.mjs --to-doc --vector <path> --out <path>

import fs from "node:fs";
import path from "node:path";

import { Session } from "../../src/features/document/session.js";
import { PixelWriter } from "../../src/core/chunkstore.js";
import { exportPngBytes } from "../../src/io/export_png.js";
import { documentToJson, jsonToDocument } from "../../src/io/serialize.js";
import { applyCommand, setGlyphSets } from "./render_tile.js";
import { loadGlyphSet } from "./text_glyphs.mjs";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
setGlyphSets(loadGlyphSet(path.join(__dirname, "text_glyphs.json")));

// <META - ROLE : Extract vector commands from Document | L20-120)>
function documentToVectorCommands(doc) {
  const result = {
    schema_version: "2.0.0",
    format: "draw_tool.vector",
    canvas: {
      width_px: doc.canvas.widthPx,
      height_px: doc.canvas.heightPx,
      background: doc.canvas.background
    },
    layers: []
  };

  if (doc.canvas.viewport) {
    result.canvas.viewport = {
      x: doc.canvas.viewport.x,
      y: doc.canvas.viewport.y,
      w: doc.canvas.viewport.w,
      h: doc.canvas.viewport.h
    };
  }

  for (const layer of doc.layers) {
    const commands = [];

    // Extract commands from chunk data
    layer.store.forEachChunk((cx, cy, data) => {
      // Analyze chunk to extract shapes
      const shapes = extractShapesFromChunk(data, cx * 32, cy * 32);
      commands.push(...shapes);
    });

    result.layers.push({
      layer_id: layer.id,
      name: layer.name,
      visible: layer.visible,
      locked: layer.locked,
      opacity: layer.opacity,
      blend: layer.blend,
      commands: commands
    });
  }

  return result;
}

// <META - ROLE : Extract shapes from 32x32 chunk data | L125-250)>
function extractShapesFromChunk(data, offsetX, offsetY) {
  const commands = [];
  const width = 32;
  const height = 32;

  // Simple shape detection: find rectangular regions
  const visited = new Array(width * height).fill(false);

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const idx = (y * width + x) * 4;
      const r = data[idx];
      const g = data[idx + 1];
      const b = data[idx + 2];
      const a = data[idx + 3];

      if (a === 0 || visited[y * width + x]) continue;

      // Find rectangle bounds
      let maxX = x;
      let maxY = y;

      // Expand right
      while (maxX + 1 < width && !visited[y * width + maxX + 1]) {
        const nextIdx = (y * width + maxX + 1) * 4;
        if (data[nextIdx + 3] === 0) break;
        if (data[nextIdx] !== r || data[nextIdx + 1] !== g || data[nextIdx + 2] !== b) break;
        maxX++;
      }

      // Expand down
      let canExpand = true;
      while (canExpand && maxY + 1 < height) {
        for (let checkX = x; checkX <= maxX; checkX++) {
          const checkIdx = ((maxY + 1) * width + checkX) * 4;
          if (data[checkIdx + 3] === 0 || visited[(maxY + 1) * width + checkX]) {
            canExpand = false;
            break;
          }
          if (data[checkIdx] !== r || data[checkIdx + 1] !== g || data[checkIdx + 2] !== b) {
            canExpand = false;
            break;
          }
        }
        if (canExpand) maxY++;
      }

      // Mark visited
      for (let vy = y; vy <= maxY; vy++) {
        for (let vx = x; vx <= maxX; vx++) {
          visited[vy * width + vx] = true;
        }
      }

      // Create command
      const colorHex = `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`;
      const w = maxX - x + 1;
      const h = maxY - y + 1;

      if (w === 1 && h === 1) {
        commands.push({
          cmd: "pixel",
          x: offsetX + x,
          y: offsetY + y,
          color: colorHex
        });
      } else {
        commands.push({
          cmd: "rect",
          box: [[offsetX + x, offsetY + y], [offsetX + maxX, offsetY + maxY]],
          fill: colorHex
        });
      }
    }
  }

  return commands;
}

// <META - ROLE : Convert vector commands to Document | L255-350)>
async function vectorCommandsToDocument(vectorJson) {
  const session = new Session();
  const docW = vectorJson.canvas.width_px;
  const docH = vectorJson.canvas.height_px;

  session.newDocument({
    widthPx: docW,
    heightPx: docH,
    background: vectorJson.canvas.background || "transparent"
  });

  const doc = session.doc;

  // Clear default layer and add our layers
  for (const layerData of vectorJson.layers) {
    const layer = doc.addLayer(layerData.name);
    layer.visible = layerData.visible !== false;
    layer.locked = layerData.locked === true;
    layer.opacity = layerData.opacity ?? 1.0;

    const writer = new PixelWriter(layer.store, layer.id);
    const ctx = {
      writer,
      palette: {},
      width: docW,
      height: docH
    };

    for (const cmd of layerData.commands) {
      // Resolve color tokens
      const resolvedCmd = { ...cmd };
      if (resolvedCmd.fill && resolvedCmd.fill.startsWith('$')) {
        resolvedCmd.fill = vectorJson.palette?.[resolvedCmd.fill.slice(1)] || '#000000';
      }
      if (resolvedCmd.color && resolvedCmd.color.startsWith('$')) {
        resolvedCmd.color = vectorJson.palette?.[resolvedCmd.color.slice(1)] || '#000000';
      }
      applyCommand(ctx, resolvedCmd);
    }
    writer.finish();
  }

  return doc;
}

// <META - ROLE : Main CLI entry point | L355-420)>
async function main() {
  const args = process.argv.slice(2);
  let mode = null;
  let inputPath = null;
  let outputPath = null;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--to-vector") mode = "to-vector";
    if (args[i] === "--to-doc") mode = "to-doc";
    if (args[i] === "--doc") inputPath = args[++i];
    if (args[i] === "--vector") inputPath = args[++i];
    if (args[i] === "--out") outputPath = args[++i];
  }

  if (!mode || !inputPath || !outputPath) {
    console.error("Usage:");
    console.error("  node vector_converter.mjs --to-vector --doc <path> --out <path>");
    console.error("  node vector_converter.mjs --to-doc --vector <path> --out <path>");
    process.exit(1);
  }

  const input = fs.readFileSync(inputPath, "utf-8");
  const inputJson = JSON.parse(input);

  if (mode === "to-vector") {
    // Document JSON -> Vector commands
    const { document } = await jsonToDocument(inputJson);
    const vectorJson = documentToVectorCommands(document);
    fs.writeFileSync(outputPath, JSON.stringify(vectorJson, null, 2), "utf-8");
    console.log(`[VectorConverter] Document JSON -> Vector commands: ${outputPath}`);
    console.log(`[VectorConverter] Layers: ${vectorJson.layers.length}`);
    const totalCmds = vectorJson.layers.reduce((sum, l) => sum + l.commands.length, 0);
    console.log(`[VectorConverter] Total commands: ${totalCmds}`);

  } else if (mode === "to-doc") {
    // Vector commands -> Document JSON
    const doc = await vectorCommandsToDocument(inputJson);
    const docJson = await documentToJson(doc);
    fs.writeFileSync(outputPath, JSON.stringify(docJson, null, 2), "utf-8");
    console.log(`[VectorConverter] Vector commands -> Document JSON: ${outputPath}`);
    console.log(`[VectorConverter] Layers: ${docJson.layers.length}`);
  }
}

main().catch(err => {
  console.error(`[VectorConverter] Error: ${err.message}`);
  process.exit(1);
});
