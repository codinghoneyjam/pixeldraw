// <META - FILE SUMMARY - Vector JSON: trace raster chunks to commands + bake commands back>
// Split out of serialize.js: the vector half of import/export. serialize.js keeps
// the raster document/layer JSON half and re-exports these four entry points.
// <META - SUMMARY CONT - documentToVectorJson scans every non-empty chunk into
// <META - SUMMARY CONT - rect/pixel commands; vectorJsonToDocument replays them
// <META - SUMMARY CONT - through applyCommand, resolving "$name" palette tokens.
import { SCHEMA_VERSION, CHUNK_PX } from "../core/constants.js";
import { ChunkStore, PixelWriter } from "../core/chunkstore.js";
import { DrawToolError } from "../core/errors.js";
import { Document, IdGen } from "../features/document/document.js";
import { Layer } from "../features/layers/layer.js";
import { applyCommand } from "../../tools/recipe/render_tile.js";

// <META - ROLE : Extract vector commands from Document layers | L15-53>
export function documentToVectorJson(doc) {
  const result = {
    schema_version: SCHEMA_VERSION,
    format: "draw_tool.vector",
    canvas: {
      width_px: doc.canvas.widthPx,
      height_px: doc.canvas.heightPx,
      background: doc.canvas.background,
    },
    layers: [],
  };

  if (doc.canvas.viewport) {
    result.canvas.viewport = {
      x: doc.canvas.viewport.x,
      y: doc.canvas.viewport.y,
      w: doc.canvas.viewport.w,
      h: doc.canvas.viewport.h,
    };
  }

  for (const layer of doc.layers) {
    const commands = [];
    layer.store.forEachChunk((cx, cy, data) => {
      const shapes = extractShapesFromChunk(data, cx * CHUNK_PX, cy * CHUNK_PX);
      commands.push(...shapes);
    });
    result.layers.push({
      layer_id: layer.id,
      name: layer.name,
      visible: layer.visible,
      locked: layer.locked,
      opacity: layer.opacity,
      blend: layer.blend,
      commands,
    });
  }
  return result;
}

// <META - ROLE : Extract shapes from 32x32 chunk data | L56-102>
function extractShapesFromChunk(data, offsetX, offsetY) {
  const commands = [];
  const width = CHUNK_PX;
  const height = CHUNK_PX;
  const visited = new Array(width * height).fill(false);

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const idx = (y * width + x) * 4;
      const r = data[idx];
      const g = data[idx + 1];
      const b = data[idx + 2];
      const a = data[idx + 3];
      if (a === 0 || visited[y * width + x]) continue;

      let maxX = x;
      let maxY = y;
      while (maxX + 1 < width && !visited[y * width + maxX + 1]) {
        const nextIdx = (y * width + maxX + 1) * 4;
        if (data[nextIdx + 3] === 0) break;
        if (data[nextIdx] !== r || data[nextIdx + 1] !== g || data[nextIdx + 2] !== b) break;
        maxX++;
      }
      let canExpand = true;
      while (canExpand && maxY + 1 < height) {
        for (let checkX = x; checkX <= maxX; checkX++) {
          const checkIdx = ((maxY + 1) * width + checkX) * 4;
          if (data[checkIdx + 3] === 0 || visited[(maxY + 1) * width + checkX]) { canExpand = false; break; }
          if (data[checkIdx] !== r || data[checkIdx + 1] !== g || data[checkIdx + 2] !== b) { canExpand = false; break; }
        }
        if (canExpand) maxY++;
      }
      for (let vy = y; vy <= maxY; vy++) {
        for (let vx = x; vx <= maxX; vx++) visited[vy * width + vx] = true;
      }
      const colorHex = `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
      const w = maxX - x + 1;
      const h = maxY - y + 1;
      if (w === 1 && h === 1) {
        commands.push({ cmd: "pixel", x: offsetX + x, y: offsetY + y, color: colorHex });
      } else {
        commands.push({ cmd: "rect", box: [[offsetX + x, offsetY + y], [offsetX + maxX, offsetY + maxY]], fill: colorHex });
      }
    }
  }
  return commands;
}

// <META - ROLE : Build Document from vector command JSON | L105-163>
export async function vectorJsonToDocument(obj) {
  if (!obj || typeof obj !== "object") {
    throw new DrawToolError("VECTOR_INVALID", "vector JSON must be an object");
  }
  if (!obj.canvas || typeof obj.canvas !== "object") {
    throw new DrawToolError("VECTOR_INVALID", "vector JSON is missing 'canvas'");
  }
  const { width_px: widthPx, height_px: heightPx } = obj.canvas;
  if (!Number.isFinite(widthPx) || !Number.isFinite(heightPx) || widthPx <= 0 || heightPx <= 0) {
    throw new DrawToolError("VECTOR_INVALID", `invalid canvas size ${widthPx}x${heightPx}`);
  }
  if (!Array.isArray(obj.layers) || obj.layers.length === 0) {
    throw new DrawToolError("VECTOR_INVALID", "vector JSON must have a non-empty 'layers' array");
  }
  const background = obj.canvas.background ?? "transparent";
  const ids = new IdGen();
  const layers = [];
  const usedIds = [];
  let activeLayerId = null;
  for (const layerData of obj.layers) {
    const id = typeof layerData.layer_id === "string" && layerData.layer_id ? layerData.layer_id : `layer_${layers.length + 1}`;
    usedIds.push(id);
    const store = new ChunkStore(widthPx, heightPx);
    const layer = new Layer(
      id,
      typeof layerData.name === "string" ? layerData.name : `Layer ${layers.length + 1}`,
      layerData.visible !== false,
      layerData.locked === true,
      Number.isFinite(layerData.opacity) ? layerData.opacity : 1.0,
      typeof layerData.blend === "string" ? layerData.blend : "normal",
      store,
    );
    const writer = new PixelWriter(store, layer.id);
    const ctx = { writer, palette: obj.palette ?? {}, width: widthPx, height: heightPx };
    const commands = Array.isArray(layerData.commands) ? layerData.commands : [];
    for (const cmd of commands) {
      const resolvedCmd = { ...cmd };
      if (typeof resolvedCmd.fill === "string" && resolvedCmd.fill.startsWith("$")) {
        resolvedCmd.fill = obj.palette?.[resolvedCmd.fill.slice(1)] ?? "#000000";
      }
      if (typeof resolvedCmd.color === "string" && resolvedCmd.color.startsWith("$")) {
        resolvedCmd.color = obj.palette?.[resolvedCmd.color.slice(1)] ?? "#000000";
      }
      applyCommand(ctx, resolvedCmd);
    }
    writer.finish();
    layers.push(layer);
    if (activeLayerId === null) activeLayerId = layer.id;
  }
  ids.seed(usedIds);
  return new Document({
    id: obj.document_id,
    name: obj.name ?? "Untitled",
    canvas: { widthPx, heightPx, background, viewport: obj.canvas.viewport ?? null },
    layers,
    activeLayerId,
    ids,
  });
}
