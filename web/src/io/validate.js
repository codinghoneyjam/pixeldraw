// <META - FILE SUMMARY - Schema + semantic validation for document/layer JSON>
// Never throws: collects up to 50 violations, structural (SCHEMA) first, then semantic.
import { checkCanvas, checkLayer } from "./validate_structural.js";
import { checkChunksSemantic } from "./validate_semantic.js";

const MAX_ERRORS = 50;
const DOC_KEYS = ["schema_version", "format", "document_id", "name", "canvas", "active_layer_id", "layers"];
const LAYER_FILE_KEYS = ["schema_version", "format", "source_canvas", "layer"];

// <META - ROLE : Error collector with 50 cap | L10-20>
function collector() {
  const errors = [];
  const push = (code, path, message) => {
    if (errors.length < MAX_ERRORS) errors.push({ code, path, message });
  };
  return { errors, push };
}

// <META - ROLE : Plain object check | L22-24>
function isObj(v) {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

// <META - ROLE : Flag unexpected keys as SCHEMA | L26-32>
function checkNoExtra(obj, allowed, base, push) {
  for (const k of Object.keys(obj)) {
    if (!allowed.includes(k)) push("SCHEMA", `${base}/${k}`, `unexpected property ${k}`);
  }
}

// <META - ROLE : Validate whole document, structural then semantic | L34-96>
export async function validateDocument(obj) {
  const { errors, push } = collector();
  if (!isObj(obj)) return { ok: false, errors: [{ code: "SCHEMA", path: "", message: "document must be an object" }] };
  checkNoExtra(obj, DOC_KEYS, "", push);
  if (obj.schema_version !== "2.0.0") push("SCHEMA", "/schema_version", "schema_version must be 2.0.0");
  if (obj.format !== "draw_tool.document") push("SCHEMA", "/format", "format must be draw_tool.document");
  if (typeof obj.document_id !== "string" || obj.document_id.length < 1) {
    push("SCHEMA", "/document_id", "document_id must be a non-empty string");
  }
  if (obj.name !== undefined && (typeof obj.name !== "string" || obj.name.length > 128)) {
    push("SCHEMA", "/name", "name must be a string of at most 128 chars");
  }
  if (obj.active_layer_id !== undefined && (typeof obj.active_layer_id !== "string" || obj.active_layer_id.length < 1)) {
    push("SCHEMA", "/active_layer_id", "active_layer_id must be a non-empty string");
  }
  const dims = obj.canvas === undefined ? (push("SCHEMA", "/canvas", "canvas is required"), null) : checkCanvas(obj.canvas, "/canvas", push);
  let layersOk = false;
  let layerInfos = [];
  if (!Array.isArray(obj.layers)) {
    push("SCHEMA", "/layers", "layers must be an array");
  } else {
    if (obj.layers.length < 1 || obj.layers.length > 64) {
      push("SCHEMA", "/layers", "layers must hold 1-64 entries");
    } else layersOk = true;
    for (let i = 0; i < obj.layers.length; i++) {
      layerInfos.push(checkLayer(obj.layers[i], `/layers/${i}`, push));
    }
  }
  if (errors.length > 0 && errors[0].code === "SCHEMA" && errors.length >= MAX_ERRORS) {
    return { ok: errors.length === 0, errors };
  }
  if (layersOk) {
    const seenIds = new Set();
    for (let i = 0; i < obj.layers.length; i++) {
      const ly = obj.layers[i];
      if (!isObj(ly) || typeof ly.layer_id !== "string") continue;
      if (seenIds.has(ly.layer_id)) {
        push("DUP_LAYER_ID", `/layers/${i}/layer_id`, `duplicate layer id ${ly.layer_id}`);
      } else seenIds.add(ly.layer_id);
    }
    if (typeof obj.active_layer_id === "string" && !seenIds.has(obj.active_layer_id)) {
      push("ACTIVE_LAYER_MISSING", "/active_layer_id", `active layer ${obj.active_layer_id} missing`);
    }
    for (let i = 0; i < obj.layers.length; i++) {
      const ly = obj.layers[i];
      if (!isObj(ly) || ly.type !== "raster" || !isObj(ly.raster) || !Array.isArray(ly.raster.chunks)) continue;
      await checkChunksSemantic(ly.raster.chunks, layerInfos[i].structuralChunks, dims, `/layers/${i}/raster/chunks`, push);
      if (errors.length >= MAX_ERRORS) break;
    }
  }
  return { ok: errors.length === 0, errors };
}

// <META - ROLE : Validate single layer file | L98-136>
export async function validateLayerFile(obj) {
  const { errors, push } = collector();
  if (!isObj(obj)) return { ok: false, errors: [{ code: "SCHEMA", path: "", message: "layer file must be an object" }] };
  checkNoExtra(obj, LAYER_FILE_KEYS, "", push);
  if (obj.schema_version !== "2.0.0") push("SCHEMA", "/schema_version", "schema_version must be 2.0.0");
  if (obj.format !== "draw_tool.layer") push("SCHEMA", "/format", "format must be draw_tool.layer");
  const dims = obj.source_canvas === undefined
    ? (push("SCHEMA", "/source_canvas", "source_canvas is required"), null)
    : checkCanvas(obj.source_canvas, "/source_canvas", push);
  let info = { structuralChunks: [] };
  if (obj.layer === undefined) push("SCHEMA", "/layer", "layer is required");
  else info = checkLayer(obj.layer, "/layer", push);
  if (isObj(obj.layer) && obj.layer.type === "raster" && isObj(obj.layer.raster) && Array.isArray(obj.layer.raster.chunks)) {
    await checkChunksSemantic(obj.layer.raster.chunks, info.structuralChunks, dims, "/layer/raster/chunks", push);
  }
  return { ok: errors.length === 0, errors };
}
