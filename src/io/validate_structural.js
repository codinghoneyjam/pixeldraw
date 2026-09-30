// <META - FILE SUMMARY - Structural validation for canvas, chunk, raster, layer>

const CANVAS_KEYS = ["tile_px", "unit_px", "width_px", "height_px", "background"];
const LAYER_KEYS = ["layer_id", "name", "type", "visible", "locked", "opacity", "blend", "raster", "shapes"];
const RASTER_KEYS = ["chunk_px", "encoding", "chunks"];
const CHUNK_KEYS = ["cx", "cy", "png"];
const BLENDS = ["normal", "multiply", "screen", "overlay", "darken", "lighten"];
const PNG_RE = /^iVBORw0KGgo[A-Za-z0-9+/]*={0,2}$/;
const BG_RE = /^(transparent|#([0-9a-fA-F]{6}|[0-9a-fA-F]{8}))$/;

// <META - ROLE : Plain object check | L16-18>
function isObj(v) {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

// <META - ROLE : Flag unexpected keys as SCHEMA | L20-26>
function checkNoExtra(obj, allowed, base, push) {
  for (const k of Object.keys(obj)) {
    if (!allowed.includes(k)) push("SCHEMA", `${base}/${k}`, `unexpected property ${k}`);
  }
}

// <META - ROLE : Validate canvas object | L28-71>
export function checkCanvas(c, base, push) {
  if (!isObj(c)) {
    push("SCHEMA", base, "canvas must be an object");
    return null;
  }
  checkNoExtra(c, CANVAS_KEYS, base, push);
  if (c.tile_px !== 64) push("SCHEMA", `${base}/tile_px`, "tile_px must be 64");
  if (c.unit_px !== 32) push("SCHEMA", `${base}/unit_px`, "unit_px must be 32");
  let w = null;
  let h = null;
  if (!Number.isInteger(c.width_px) || c.width_px < 32 || c.width_px > 1920 || c.width_px % 32 !== 0) {
    push("SCHEMA", `${base}/width_px`, "width_px must be a multiple of 32 in [32,1920]");
  } else w = c.width_px;
  if (!Number.isInteger(c.height_px) || c.height_px < 32 || c.height_px > 1088 || c.height_px % 32 !== 0) {
    push("SCHEMA", `${base}/height_px`, "height_px must be a multiple of 32 in [32,1088]");
  } else h = c.height_px;
  if (c.background !== undefined && (typeof c.background !== "string" || !BG_RE.test(c.background))) {
    push("SCHEMA", `${base}/background`, "background must be transparent or #rrggbb[#aa]");
  }
  return w !== null && h !== null ? { widthPx: w, heightPx: h } : null;
}

// <META - ROLE : Validate one chunk entry | L73-97>
export function checkChunk(ch, base, push) {
  if (!isObj(ch)) {
    push("SCHEMA", base, "chunk must be an object");
    return false;
  }
  checkNoExtra(ch, CHUNK_KEYS, base, push);
  let ok = true;
  if (!Number.isInteger(ch.cx) || ch.cx < 0 || ch.cx > 59) {
    push("SCHEMA", `${base}/cx`, "cx must be an integer in [0,59]");
    ok = false;
  }
  if (!Number.isInteger(ch.cy) || ch.cy < 0 || ch.cy > 33) {
    push("SCHEMA", `${base}/cy`, "cy must be an integer in [0,33]");
    ok = false;
  }
  if (typeof ch.png !== "string" || !PNG_RE.test(ch.png)) {
    push("SCHEMA", `${base}/png`, "png must be base64 starting with iVBORw0KGgo");
    ok = false;
  }
  return ok;
}

// <META - ROLE : Validate raster payload | L99-123>
export function checkRaster(r, base, push) {
  if (!isObj(r)) {
    push("SCHEMA", base, "raster must be an object");
    return [];
  }
  checkNoExtra(r, RASTER_KEYS, base, push);
  if (r.chunk_px !== 32) push("SCHEMA", `${base}/chunk_px`, "chunk_px must be 32");
  if (r.encoding !== "png_base64") push("SCHEMA", `${base}/encoding`, "encoding must be png_base64");
  if (!Array.isArray(r.chunks)) {
    push("SCHEMA", `${base}/chunks`, "chunks must be an array");
    return [];
  }
  if (r.chunks.length > 2040) push("SCHEMA", `${base}/chunks`, "chunks must hold at most 2040 entries");
  const structuralOk = [];
  for (let j = 0; j < r.chunks.length; j++) {
    structuralOk.push(checkChunk(r.chunks[j], `${base}/chunks/${j}`, push));
  }
  return structuralOk;
}

// <META - ROLE : Validate one layer | L125-171>
export function checkLayer(ly, base, push) {
  if (!isObj(ly)) {
    push("SCHEMA", base, "layer must be an object");
    return { structuralChunks: [], type: null };
  }
  checkNoExtra(ly, LAYER_KEYS, base, push);
  if (typeof ly.layer_id !== "string" || ly.layer_id.length < 1 || ly.layer_id.length > 64) {
    push("SCHEMA", `${base}/layer_id`, "layer_id must be a string of 1-64 chars");
  }
  if (typeof ly.name !== "string" || ly.name.length > 128) {
    push("SCHEMA", `${base}/name`, "name must be a string of at most 128 chars");
  }
  if (ly.type !== "raster" && ly.type !== "vector") {
    push("SCHEMA", `${base}/type`, "type must be raster or vector");
  }
  if (typeof ly.visible !== "boolean") push("SCHEMA", `${base}/visible`, "visible must be boolean");
  if (typeof ly.locked !== "boolean") push("SCHEMA", `${base}/locked`, "locked must be boolean");
  if (typeof ly.opacity !== "number" || !Number.isFinite(ly.opacity) || ly.opacity < 0 || ly.opacity > 1) {
    push("SCHEMA", `${base}/opacity`, "opacity must be a number in [0,1]");
  }
  if (!BLENDS.includes(ly.blend)) push("SCHEMA", `${base}/blend`, "blend must be a known blend mode");
  let structuralChunks = [];
  if (ly.type === "raster") {
    if (ly.shapes !== undefined) push("SCHEMA", `${base}/shapes`, "raster layer must not have shapes");
    if (ly.raster === undefined) push("SCHEMA", `${base}/raster`, "raster layer requires raster");
    else structuralChunks = checkRaster(ly.raster, `${base}/raster`, push);
  } else if (ly.type === "vector") {
    if (ly.raster !== undefined) push("SCHEMA", `${base}/raster`, "vector layer must not have raster");
    if (!Array.isArray(ly.shapes)) push("SCHEMA", `${base}/shapes`, "vector layer requires shapes array");
  }
  return { structuralChunks, type: ly.type };
}

export const _INTERNALS = { CANVAS_KEYS, LAYER_KEYS, RASTER_KEYS, CHUNK_KEYS, BLENDS, PNG_RE, BG_RE };
