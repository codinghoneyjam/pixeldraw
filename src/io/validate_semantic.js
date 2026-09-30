// <META - FILE SUMMARY - Semantic validation with PNG decoding>
import { DrawToolError } from "../core/errors.js";
import { base64ToBytes } from "./base64.js";
import { decodePng } from "./png.js";

// <META - ROLE : Plain object check | L8-10>
function isObj(v) {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

// <META - ROLE : Semantic chunk checks via real PNG decode | L12-56>
export async function checkChunksSemantic(chunks, structuralOk, dims, base, push) {
  const seen = new Set();
  for (let j = 0; j < chunks.length; j++) {
    const ch = chunks[j];
    if (!isObj(ch)) continue;
    const key = `${ch.cx},${ch.cy}`;
    if (Number.isInteger(ch.cx) && Number.isInteger(ch.cy)) {
      if (seen.has(key)) {
        push("CHUNK_DUPLICATE", `${base}/${j}`, `duplicate chunk (${ch.cx},${ch.cy})`);
      } else seen.add(key);
      if (dims !== null && (ch.cx * 32 >= dims.widthPx || ch.cy * 32 >= dims.heightPx)) {
        push("CHUNK_OUT_OF_CANVAS", `${base}/${j}`, `chunk (${ch.cx},${ch.cy}) outside canvas`);
      }
    }
    if (structuralOk[j] === true) {
      let bad = false;
      try {
        const bytes = base64ToBytes(ch.png);
        const dec = await decodePng(bytes);
        if (dec.width !== 32 || dec.height !== 32) bad = true;
      } catch (e) {
        if (e instanceof DrawToolError) bad = true;
        else bad = true;
      }
      if (bad) push("CHUNK_BAD_PNG", `${base}/${j}/png`, "chunk png must decode to 32x32 RGBA");
    }
  }
}

export const _INTERNALS = {};
