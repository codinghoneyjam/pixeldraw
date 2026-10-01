// <META - FILE SUMMARY - File actions: save, open, exportLayer, importLayer, exportPng, new>
import { DrawToolError } from "../../core/errors.js";
import { EVENTS } from "../../core/events.js";
import { documentToJson, importLayerJson, jsonToDocument, layerToJson } from "../../io/serialize.js";
import { exportPngBytes } from "../../io/export_png.js";
import { pickFile, readJsonFile, saveBinaryFile, saveTextFile } from "../../io/file_io.js";
import { confirmDiscardChanges, showNewDocumentDialog, showProgress } from "../dialogs.js";
import { STRINGS } from "../strings.js";

// <META - ROLE : Save document to JSON file | L1-18>
export async function doSave(session, toast) {
  const doc = session.doc;
  if (!doc) return;
  const prog = showProgress("저장 중");
  try {
    const obj = await documentToJson(doc, { onProgress: (d, t) => prog.update(t ? d / t : 0) });
    const ok = await saveTextFile(`${doc.name || "untitled"}.draw.json`, JSON.stringify(obj));
    if (ok) {
      session.history.markSaved();
      session.dispatchEvent(new CustomEvent(EVENTS.HISTORY_CHANGED, {
        detail: { canUndo: session.history.canUndo(), canRedo: session.history.canRedo(), dirty: session.history.isDirty() },
      }));
      toast(STRINGS.toast.saved);
    }
  } finally {
    prog.close();
  }
}

// <META - ROLE : Open document from JSON file | L20-40>
export async function doOpen(session, toast) {
  if (session.history.isDirty()) {
    const go = await confirmDiscardChanges();
    if (!go) return;
  }
  const file = await pickFile(".json,application/json");
  if (!file) return;
  const prog = showProgress("열기");
  try {
    const obj = await readJsonFile(file);
    if (obj && obj.format === "draw_tool.document") {
      const { document: doc, warnings } = await jsonToDocument(obj, { onProgress: (d, t) => prog.update(t ? d / t : 0) });
      session.loadDocument(doc);
      for (const w of warnings) toast(w, "warn");
      toast(STRINGS.toast.opened);
    } else if (obj && obj.format === "draw_tool.layer") {
      session.notify("info", STRINGS.toast.layerFileHint);
    } else {
      throw new DrawToolError("SCHEMA", "unknown file format");
    }
  } finally {
    prog.close();
  }
}

// <META - ROLE : Export active layer to JSON file | L42-55>
export async function doExportLayer(session, toast) {
  const doc = session.doc;
  if (!doc) return;
  const prog = showProgress("레이어 내보내기");
  try {
    const obj = await layerToJson(doc, doc.activeLayerId);
    const layer = doc.getLayer(doc.activeLayerId);
    const ok = await saveTextFile(`${layer.name || "layer"}.drawlayer.json`, JSON.stringify(obj));
    if (ok) toast(STRINGS.toast.exportedLayer);
  } finally {
    prog.close();
  }
}

// <META - ROLE : Import layer from JSON file | L57-72>
export async function doImportLayer(session, toast) {
  const file = await pickFile(".json,application/json");
  if (!file) return;
  const prog = showProgress("레이어 가져오기");
  try {
    const obj = await readJsonFile(file);
    const { layer, dropped, warnings } = await importLayerJson(session.doc, obj);
    session.insertLayer(layer);
    for (const w of warnings) toast(w, "warn");
    if (dropped > 0) toast(`${STRINGS.toast.droppedChunks}: ${dropped}`, "warn");
    toast(STRINGS.toast.importedLayer);
  } finally {
    prog.close();
  }
}

// <META - ROLE : Export document as PNG | L74-87>
export async function doExportPng(session, toast) {
  const doc = session.doc;
  if (!doc) return;
  const prog = showProgress("PNG 내보내기");
  try {
    const bytes = await exportPngBytes(doc, { includeBackground: true });
    prog.update(0.8);
    const ok = await saveBinaryFile(`${doc.name || "untitled"}.png`, bytes, "image/png");
    if (ok) toast(STRINGS.toast.exportedPng);
  } finally {
    prog.close();
  }
}

// <META - ROLE : Create new document via dialog | L89-104>
export async function doNew(session) {
  if (session.history.isDirty()) {
    const go = await confirmDiscardChanges();
    if (!go) return;
  }
  const res = await showNewDocumentDialog();
  if (!res) return;
  let id = undefined;
  try {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") id = crypto.randomUUID();
  } catch { /* ignore */ }
  session.newDocument({ widthPx: res.widthPx, heightPx: res.heightPx, background: res.background, name: res.name, id });
}
