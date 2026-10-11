// <META - FILE SUMMARY - File save/open: native paths in the desktop shell, browser APIs under run.py>
//
// One module, two runtimes -- src/ui/actions/file_actions.js does not change:
//
//   desktop app  `window.__TAURI__` exists (app.withGlobalTauri in tauri.conf.json).
//                pick_save_path / pick_open_path show native dialogs, and
//                write_file_bytes / read_file_bytes write to the exact path the
//                user chose instead of going through the browser download folder.
//
//   run.py       no `window.__TAURI__`; the File System Access API is used when
//                present, otherwise an anchor download.
//
// Bytes cross the bridge as base64 so the payload only depends on JSON, never on
// a particular Tauri IPC version.
//
// Node-importable: nothing here touches `document` or `window` at module scope.

import { DrawToolError } from "../core/errors.js";
import { base64ToBytes, bytesToBase64 } from "./base64.js";

const MAX_JSON_BYTES = 64 * 1024 * 1024;

// <META - ROLE : Bound Tauri invoke, or null when running in a plain browser | L27-33>
function tauriInvoke() {
  const tauri = globalThis.window && globalThis.window.__TAURI__;
  if (!tauri || typeof tauri.invoke !== "function") return null;
  return (cmd, args) => tauri.invoke(cmd, args);
}

// <META - ROLE : Derive a native dialog filter from the suggested name or MIME | L35-42>
function filterFor(suggestedName, mime) {
  const match = /\.([a-z0-9]+)$/i.exec(String(suggestedName || ""));
  if (match) return { name: match[1].toUpperCase(), extensions: [match[1].toLowerCase()] };
  if (/png/.test(String(mime || ""))) return { name: "PNG", extensions: ["png"] };
  return { name: "JSON", extensions: ["json"] };
}

// <META - ROLE : Ask for a save path, then write bytes; false when cancelled | L44-54>
async function saveNative(invoke, suggestedName, bytes, mime) {
  const path = await invoke("pick_save_path", {
    suggested: sanitizeFileName(suggestedName),
    filter: filterFor(suggestedName, mime),
  });
  if (!path) return false;
  await invoke("write_file_bytes", { path, data: bytesToBase64(bytes) });
  return true;
}

// <META - ROLE : Pick a file with a native dialog and wrap it as a real File | L56-70>
async function pickFileNative(invoke, accept) {
  const duplicates = String(accept || "")
    .split(",")
    .map((s) => s.trim().replace(/^\./, "").split("/").pop())
    .filter((s) => s && /^[a-z0-9]+$/i.test(s));
  // ".json,application/json" yields ["json", "json"]; a dialog filter wants each
  // extension exactly once.
  const extensions = [...new Set(duplicates)];
  const path = await invoke("pick_open_path", {
    filter: extensions.length ? { name: "JSON", extensions } : null,
  });
  if (!path) return null;
  const data = base64ToBytes(await invoke("read_file_bytes", { path }));
  const name = String(path).split(/[\\/]/).pop() || "file.json";
  // Returning a File keeps readJsonFile's contract (size + text()) identical.
  return new File([data], name);
}

// <META - ROLE : Sanitize a suggested file name | L72-86>
export function sanitizeFileName(name) {
  let s = typeof name === "string" ? name : "";
  s = s.replace(/[\\/:*?"<>|\x00-\x1f]+/g, "_");
  s = s.replace(/^[\s.]+|[\s.]+$/g, "");
  if (s.length > 80) s = s.slice(0, 80);
  s = s.replace(/^[\s.]+|[\s.]+$/g, "");
  if (s.length === 0) return "untitled";
  return s;
}

// <META - ROLE : Download fallback for saving blobs | L88-99>
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

// <META - ROLE : Save bytes natively, via picker, or via download fallback | L101-125>
async function saveBytes(suggestedName, bytes, mime) {
  const name = sanitizeFileName(suggestedName);
  const invoke = tauriInvoke();
  if (invoke) {
    const data = typeof bytes === "string" ? new TextEncoder().encode(bytes) : bytes;
    return saveNative(invoke, suggestedName, data, mime);
  }
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

// <META - ROLE : Save text file | L127-129>
export async function saveTextFile(suggestedName, text, mime = "application/json") {
  return saveBytes(suggestedName, text, mime);
}

// <META - ROLE : Save binary file | L131-133>
export async function saveBinaryFile(suggestedName, bytes, mime = "application/octet-stream") {
  return saveBytes(suggestedName, bytes, mime);
}

// <META - ROLE : Pick a file natively, via picker, or via input fallback | L135-157>
export async function pickFile(accept) {
  const invoke = tauriInvoke();
  if (invoke) return pickFileNative(invoke, accept);
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

// <META - ROLE : Read + parse a JSON file with size guard | L159-173>
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
