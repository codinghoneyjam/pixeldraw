// Core constants — single source of truth (docs/contract.md §1).

export const TILE_PX = 64;
export const UNIT_PX = 32;
export const CHUNK_PX = 32;
export const CHUNK_LEN = 32 * 32 * 4;
export const MIN_SIZE_PX = 32;
export const MAX_W_PX = 1920;
export const MAX_H_PX = 1088;
export const MAX_CHUNKS_X = 60;
export const MAX_CHUNKS_Y = 34;
export const PEN_MIN = 1;
export const PEN_MAX = 64;
export const PEN_DEFAULT = 1;
export const MAX_LAYERS = 64;
export const UNDO_MAX_STEPS = 200;
export const UNDO_MAX_BYTES = 96 * 1024 * 1024;
export const ZOOM_LEVELS = Object.freeze([0.25, 0.5, 1, 2, 3, 4, 6, 8, 12, 16, 24, 32]);
export const DEFAULT_DOC = Object.freeze({ widthPx: 512, heightPx: 512, background: "transparent" });
export const SCHEMA_VERSION = "2.0.0";

export function chunkKey(cx, cy) {
  return cy * 64 + cx;
}

export function chunkCoords(key) {
  return { cx: key % 64, cy: Math.floor(key / 64) };
}

export function isValidCanvasSize(w, h) {
  return Number.isInteger(w) && Number.isInteger(h)
    && w >= MIN_SIZE_PX && h >= MIN_SIZE_PX
    && w <= MAX_W_PX && h <= MAX_H_PX
    && w % CHUNK_PX === 0 && h % CHUNK_PX === 0;
}
