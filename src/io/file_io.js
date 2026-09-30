// <META - FILE SUMMARY - Browser file save/load with File System Access or fallback>
// Node-importable (no top-level DOM access). readJsonFile enforces 64 MiB + JSON parse.
import { DrawToolError } from "../core/errors.js";

const MAX_JSON_BYTES = 64 * 1024 * 1024;

// <META - ROLE : Sanitize a suggested file name | L8-22>
export function sanitizeFileName(name) {
  let s = typeof name === "string" ? name : "";
  s = s.replace(/[\\/:*?"<>|\x00-\x1f]+/g, "_");
  s = s.replace(/^[\s.]+|[\s.]+$/g, "");
  if (s.length > 80) s = s.slice(0, 80);
  s = s.replace(/^[\s.]+|[\s.]+$/g, "");
  if (s.length === 0) return "untitled";
  return s;
}

// <META - ROLE : Download fallback for saving blobs | L24-36>
function downloadBlob(blob, suggestedName) {
  const doc = globalThis.document;
  const a = doc.createElement("a");
  const url = URL.createObjectURL(blob);
  a.href = url;
  a.download = suggestedName;
  doc.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

// <META - ROLE : Save bytes via picker or download fallback | L38-72>
async function saveBytes(suggestedName, bytes, mime) {
  const name = sanitizeFileName(suggestedName);
  const blob = new Blob([bytes], { type: mime });
  try {
    const picker = globalThis.showSaveFilePicker;
    if (typeof picker === "function") {
      const handle = await picker({ suggestedName: name });
      const writable = await handle.createWritable();
      await writable.write(blob);
      await writable.close();
      return true;
    }
  } catch (e) {
    if (e && e.name === "AbortError") return false;
    throw e;
  }
  if (typeof globalThis.document === "undefined") {
    throw new DrawToolError("INVALID_STATE", "no file save API available");
  }
  downloadBlob(blob, name);
  return true;
}

// <META - ROLE : Save text file | L74-76>
export async function saveTextFile(suggestedName, text, mime = "application/json") {
  return saveBytes(suggestedName, text, mime);
}

// <META - ROLE : Save binary file | L78-80>
export async function saveBinaryFile(suggestedName, bytes, mime = "application/octet-stream") {
  return saveBytes(suggestedName, bytes, mime);
}

// <META - ROLE : Pick a file via picker or input fallback | L82-116>
export async function pickFile(accept) {
  try {
    const picker = globalThis.showOpenFilePicker;
    if (typeof picker === "function") {
      const [handle] = await picker({ multiple: false });
      if (!handle) return null;
      return await handle.getFile();
    }
  } catch (e) {
    if (e && e.name === "AbortError") return null;
    throw e;
  }
  const doc = globalThis.document;
  if (!doc) throw new DrawToolError("INVALID_STATE", "no file pick API available");
  return new Promise((resolve) => {
    const input = doc.createElement("input");
    input.type = "file";
    if (accept) input.accept = accept;
    input.onchange = () => resolve(input.files && input.files[0] ? input.files[0] : null);
    input.oncancel = () => resolve(null);
    input.click();
  });
}

// <META - ROLE : Read + parse a JSON file with size guard | L118-134>
export async function readJsonFile(file) {
  if (!file || typeof file.size !== "number" || typeof file.text !== "function") {
    throw new DrawToolError("SCHEMA", "unreadable file");
  }
  if (file.size > MAX_JSON_BYTES) {
    throw new DrawToolError("FILE_TOO_LARGE", `file too large (${file.size} bytes)`);
  }
  const text = await file.text();
  try {
    return JSON.parse(text);
  } catch {
    throw new DrawToolError("SCHEMA", "file is not valid JSON");
  }
}
