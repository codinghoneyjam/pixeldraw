// <META - FILE SUMMARY - Base64 encode/decode for chunk PNG payloads>
// Works in browsers (btoa/atob) and Node (Buffer). Chunked to avoid call-stack overflow.
import { DrawToolError } from "../core/errors.js";

// <META - ROLE : Encode bytes to base64 | L5-29>
export function bytesToBase64(u8) {
  const bytes = u8 instanceof Uint8Array ? u8 : new Uint8Array(u8);
  if (typeof Buffer !== "undefined" && typeof Buffer.from === "function") {
    return Buffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength).toString("base64");
  }
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
  }
  return btoa(binary);
}

// <META - ROLE : Decode base64 to bytes, bad chars throw SCHEMA | L31-55>
export function base64ToBytes(s) {
  if (typeof s !== "string" || !/^[A-Za-z0-9+/]*={0,2}$/.test(s)) {
    throw new DrawToolError("SCHEMA", "invalid base64 string");
  }
  if (typeof Buffer !== "undefined" && typeof Buffer.from === "function") {
    try {
      const buf = Buffer.from(s, "base64");
      return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
    } catch {
      throw new DrawToolError("SCHEMA", "invalid base64 string");
    }
  }
  let binary;
  try {
    binary = atob(s);
  } catch {
    throw new DrawToolError("SCHEMA", "invalid base64 string");
  }
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
  return out;
}
