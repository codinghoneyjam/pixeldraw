// <META - FILE SUMMARY - Chunk-level IndexedDB autosave with debounce + force interval>
// Node-importable: all browser APIs touched lazily. Pure record builders exported for tests.
import { EVENTS } from "../core/events.js";
import { openDatabase, promisify, _INTERNALS as _INTERNALS_SCHEMA } from "./idb_schema.js";
import { buildMetaRecord, buildChunkRecords, chunkRecordKey } from "./idb_record_builder.js";
import { FlushScheduler, _INTERNALS as _INTERNALS_SCHEDULER } from "./idb_scheduler.js";
import { load, isMetaValid, clear, _INTERNALS as _INTERNALS_LOADER } from "./idb_loader.js";

// <META - ROLE : Autosave store bound to a session | L13-163>
export class AutosaveStore {
  constructor(db) {
    this._db = db;
    this._session = null;
    this._dirty = new Map();
    this._metaDirty = false;
    this._fullRewrite = false;
    this._knownLayers = new Set();
    this._scheduler = new FlushScheduler();
    this._flushPromise = null;
    this._onPixels = (e) => this._markPixels(e.detail);
    this._onLayers = () => this._markMeta();
    this._onReplaced = () => this._markReplaced();
    this._onHide = () => {
      this.flushNow();
    };
  }

  // <META - ROLE : Open or return null when unavailable | L35-49>
  static async open() {
    const idb = globalThis.indexedDB;
    if (!idb) return null;
    try {
      const db = await openDatabase(idb);
      return new AutosaveStore(db);
    } catch {
      return null;
    }
  }

  // <META - ROLE : Subscribe to session events | L51-73>
  attach(session) {
    this.detach();
    this._session = session;
    const doc = session.doc;
    if (doc) {
      for (const l of doc.layers) this._knownLayers.add(l.id);
    }
    session.addEventListener(EVENTS.PIXELS_CHANGED, this._onPixels);
    session.addEventListener(EVENTS.LAYERS_CHANGED, this._onLayers);
    session.addEventListener(EVENTS.DOCUMENT_REPLACED, this._onReplaced);
    const docRef = globalThis.document;
    if (docRef && typeof docRef.addEventListener === "function") {
      docRef.addEventListener("visibilitychange", this._onHide);
    }
    const win = globalThis.window ?? globalThis;
    if (win && typeof win.addEventListener === "function" && win !== globalThis) {
      win.addEventListener("pagehide", this._onHide);
    } else if (typeof globalThis.addEventListener === "function") {
      try {
        globalThis.addEventListener("pagehide", this._onHide);
      } catch {
        /* ignore */
      }
    }
  }

  // <META - ROLE : Unsubscribe from session events | L75-91>
  detach() {
    if (this._session) {
      this._session.removeEventListener(EVENTS.PIXELS_CHANGED, this._onPixels);
      this._session.removeEventListener(EVENTS.LAYERS_CHANGED, this._onLayers);
      this._session.removeEventListener(EVENTS.DOCUMENT_REPLACED, this._onReplaced);
      this._session = null;
    }
    const docRef = globalThis.document;
    if (docRef && typeof docRef.removeEventListener === "function") {
      docRef.removeEventListener("visibilitychange", this._onHide);
    }
    if (typeof globalThis.removeEventListener === "function") {
      try {
        globalThis.removeEventListener("pagehide", this._onHide);
      } catch {
        /* ignore */
      }
    }
    this._scheduler.cancel();
  }

  // <META - ROLE : Record pixel dirty set | L93-111>
  _markPixels(detail) {
    if (!detail || typeof detail.layerId !== "string") return;
    if (detail.all === true) {
      this._dirty.set(detail.layerId, "all");
    } else if (Array.isArray(detail.chunks)) {
      let entry = this._dirty.get(detail.layerId);
      if (entry === "all") {
        /* keep */
      } else {
        if (!entry) {
          entry = new Map();
          this._dirty.set(detail.layerId, entry);
        }
        for (const c of detail.chunks) {
          if (c && Number.isInteger(c.cx) && Number.isInteger(c.cy)) entry.set(`${c.cx},${c.cy}`, { cx: c.cx, cy: c.cy });
        }
      }
    }
    this._schedule();
  }

  // <META - ROLE : Record meta dirty | L113-118>
  _markMeta() {
    this._metaDirty = true;
    this._schedule();
  }

  // <META - ROLE : Record full rewrite on replace | L120-127>
  _markReplaced() {
    this._fullRewrite = true;
    this._dirty.clear();
    this._metaDirty = true;
    this._schedule();
  }

  // <META - ROLE : Debounce with 5s force cap | L129-133>
  _schedule() {
    this._scheduler.schedule(() => this.flushNow());
  }

  // <META - ROLE : Peek meta summary | L135-145>
  async peek() {
    const tx = this._db.transaction("meta", "readonly");
    const meta = await promisify(tx.objectStore("meta").get("current"));
    if (!meta) return null;
    return { documentId: meta.documentId, name: meta.name, updatedAt: meta.updatedAt };
  }

  // <META - ROLE : Flush dirty state in one txn | L147-163>
  async flushNow() {
    if (this._flushPromise) return this._flushPromise;
    this._flushPromise = this._flushInner().finally(() => {
      this._flushPromise = null;
    });
    return this._flushPromise;
  }

  // <META - ROLE : Flush body with failure notify | L165-243>
  async _flushInner() {
    const session = this._session;
    const doc = session ? session.doc : null;
    if (!doc) return;
    if (!this._metaDirty && !this._fullRewrite && this._dirty.size === 0) return;
    const meta = buildMetaRecord(doc);
    try {
      await this._writeTxn(doc, meta);
    } catch (e) {
      if (session && typeof session.notify === "function") {
        if (e && e.name === "QuotaExceededError") {
          session.notify("warn", "자동저장 공간 부족", "STORAGE_QUOTA");
        } else {
          session.notify("warn", "자동저장 실패", "INVALID_STATE");
        }
      }
      return;
    }
    this._dirty.clear();
    this._metaDirty = false;
    this._fullRewrite = false;
    this._knownLayers = new Set(doc.layers.map((l) => l.id));
  }

  // <META - ROLE : Single readwrite txn over meta+chunks | L245-305>
  _writeTxn(doc, meta) {
    return new Promise((resolve, reject) => {
      const tx = this._db.transaction(["meta", "chunks"], "readwrite");
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error);
      const metaStore = tx.objectStore("meta");
      const chunkStore = tx.objectStore("chunks");
      const run = async () => {
        if (this._fullRewrite) {
          await promisify(metaStore.clear());
          await promisify(chunkStore.clear());
          await promisify(metaStore.put(meta));
          for (const layer of doc.layers) {
            const keys = [];
            layer.store.forEachChunk((cx, cy) => keys.push({ cx, cy }));
            for (const rec of buildChunkRecords(layer, keys)) {
              await promisify(chunkStore.put(rec));
            }
          }
          return;
        }
        await promisify(metaStore.put(meta));
        const currentIds = new Set(doc.layers.map((l) => l.id));
        for (const known of this._knownLayers) {
          if (!currentIds.has(known)) {
            const idx = chunkStore.index("byLayer");
            const keys = await promisify(idx.getAllKeys(known));
            for (const k of keys) await promisify(chunkStore.delete(k));
          }
        }
        for (const [layerId, entry] of this._dirty) {
          const layer = doc.findLayer(layerId);
          if (!layer) {
            const idx = chunkStore.index("byLayer");
            const keys = await promisify(idx.getAllKeys(layerId));
            for (const k of keys) await promisify(chunkStore.delete(k));
            continue;
          }
          if (entry === "all") {
            const idx = chunkStore.index("byLayer");
            const stored = new Set(await promisify(idx.getAllKeys(layerId)));
            const live = new Set();
            const keys = [];
            layer.store.forEachChunk((cx, cy) => keys.push({ cx, cy }));
            for (const rec of buildChunkRecords(layer, keys)) {
              live.add(rec.key);
              await promisify(chunkStore.put(rec));
            }
            for (const k of stored) {
              if (!live.has(k)) await promisify(chunkStore.delete(k));
            }
          } else {
            for (const { cx, cy } of entry.values()) {
              const data = layer.store.getChunk(cx, cy);
              if (!data) {
                await promisify(chunkStore.delete(chunkRecordKey(layerId, cx, cy)));
              } else {
                const copy = data.slice();
                await promisify(chunkStore.put({ key: chunkRecordKey(layerId, cx, cy), layerId, cx, cy, data: copy.buffer }));
              }
            }
          }
        }
      };
      run().catch((e) => {
        try {
          tx.abort();
        } catch {
          /* ignore */
        }
        reject(e);
      });
    });
  }

  // <META - ROLE : Rebuild document from storage (delegated) | L249-252>
  async load() {
    return load(this._db);
  }

  // <META - ROLE : Clear both stores (delegated) | L254-257>
  async clear() {
    return clear(this._db);
  }
}

export const _INTERNALS = { ..._INTERNALS_SCHEMA, ..._INTERNALS_LOADER, ..._INTERNALS_SCHEDULER };
