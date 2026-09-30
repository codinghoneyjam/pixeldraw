// <META - FILE SUMMARY - IndexedDB schema initialization and promisify utility>

const DB_NAME = "draw_tool_v2";
const DB_VERSION = 1;

// <META - ROLE : Promisify an IDB request | L8-20>
export function promisify(request) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

// <META - ROLE : Open database with stores | L22-44>
export function openDatabase(idb) {
  return new Promise((resolve, reject) => {
    let request;
    try {
      request = idb.open(DB_NAME, DB_VERSION);
    } catch (e) {
      reject(e);
      return;
    }
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains("meta")) db.createObjectStore("meta", { keyPath: "key" });
      if (!db.objectStoreNames.contains("chunks")) {
        const cs = db.createObjectStore("chunks", { keyPath: "key" });
        cs.createIndex("byLayer", "layerId", { unique: false });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

export const _INTERNALS = { DB_NAME, DB_VERSION };
