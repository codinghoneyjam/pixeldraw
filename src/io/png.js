// <META - FILE SUMMARY - Self-contained PNG codec: 8-bit RGBA, filter 0 encode>
// Encode uses filter 0 rows + single IDAT (deflate). Decode supports filters 0-4.
import { MAX_W_PX, MAX_H_PX } from "../core/constants.js";
import { DrawToolError } from "../core/errors.js";

const SIG = [137, 80, 78, 71, 13, 10, 26, 10];

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

// <META - ROLE : Compute CRC32 of bytes | L22-28>
function crc32(u8) {
  let c = 0xffffffff;
  for (let i = 0; i < u8.length; i++) c = CRC_TABLE[(c ^ u8[i]) & 255] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

// <META - ROLE : Compress/decompress via WebStreams deflate | L30-46>
async function deflateCompress(u8) {
  const cs = new CompressionStream("deflate");
  const src = new Blob([u8]).stream().pipeThrough(cs);
  return new Uint8Array(await new Response(src).arrayBuffer());
}

// <META - ROLE : Decompress via WebStreams deflate | L30-46>
async function deflateDecompress(u8) {
  const ds = new DecompressionStream("deflate");
  const src = new Blob([u8]).stream().pipeThrough(ds);
  return new Uint8Array(await new Response(src).arrayBuffer());
}

// <META - ROLE : Build big-endian uint32 bytes | L48-50>
function be32(v) {
  return [(v >>> 24) & 255, (v >>> 16) & 255, (v >>> 8) & 255, v & 255];
}

// <META - ROLE : Assemble one PNG chunk with CRC | L52-59>
function makeChunk(type, data) {
  const body = new Uint8Array(4 + data.length);
  for (let i = 0; i < 4; i++) body[i] = type.charCodeAt(i);
  body.set(data, 4);
  return [...be32(data.length), ...body, ...be32(crc32(body))];
}

// <META - ROLE : Encode RGBA pixels to PNG bytes | L61-72>
export async function encodePng(rgba, width, height) {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width <= 0 || height <= 0) {
    throw new DrawToolError("PNG_UNSUPPORTED", `bad png size ${String(width)}x${String(height)}`);
  }
  const px = rgba instanceof Uint8ClampedArray ? rgba : new Uint8ClampedArray(rgba);
  const raw = new Uint8Array(height * (1 + width * 4));
  for (let y = 0; y < height; y++) {
    raw.set(px.subarray(y * width * 4, (y + 1) * width * 4), y * (1 + width * 4) + 1);
  }
  const idat = await deflateCompress(raw);
  const ihdr = new Uint8Array([...be32(width), ...be32(height), 8, 6, 0, 0, 0]);
  return Uint8Array.from([...SIG, ...makeChunk("IHDR", ihdr), ...makeChunk("IDAT", idat), ...makeChunk("IEND", new Uint8Array(0))]);
}

// <META - ROLE : Decode PNG bytes to RGBA pixels | L74-130>
export async function decodePng(bytes) {
  const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  for (let i = 0; i < 8; i++) {
    if (u8[i] !== SIG[i]) throw new DrawToolError("PNG_SIGNATURE", "not a png file");
  }
  const dv = new DataView(u8.buffer, u8.byteOffset, u8.byteLength);
  let p = 8;
  let w = 0;
  let h = 0;
  let seenIhdr = false;
  const parts = [];
  while (p < u8.length) {
    if (p + 8 > u8.length) throw new DrawToolError("PNG_CRC", "truncated png chunk header");
    const len = dv.getUint32(p);
    const type = String.fromCharCode(u8[p + 4], u8[p + 5], u8[p + 6], u8[p + 7]);
    if (p + 12 + len > u8.length) throw new DrawToolError("PNG_CRC", "truncated png chunk data");
    const data = u8.subarray(p + 8, p + 8 + len);
    if (crc32(u8.subarray(p + 4, p + 8 + len)) !== dv.getUint32(p + 8 + len)) {
      throw new DrawToolError("PNG_CRC", "png chunk crc mismatch");
    }
    if (type === "IHDR") {
      seenIhdr = true;
      w = new DataView(data.buffer, data.byteOffset, data.byteLength).getUint32(0);
      h = new DataView(data.buffer, data.byteOffset, data.byteLength).getUint32(4);
      if (data[8] !== 8 || data[9] !== 6 || data[12] !== 0) {
        throw new DrawToolError("PNG_UNSUPPORTED", "only 8-bit RGBA non-interlaced png supported");
      }
      if (w <= 0 || h <= 0 || w * h > MAX_W_PX * MAX_H_PX) {
        throw new DrawToolError("PNG_UNSUPPORTED", `unsupported png size ${w}x${h}`);
      }
    } else if (type === "IDAT") {
      parts.push(data);
    } else if (type === "IEND") {
      break;
    }
    p += 12 + len;
  }
  if (!seenIhdr) throw new DrawToolError("PNG_UNSUPPORTED", "png missing IHDR");
  const total = parts.reduce((s, a) => s + a.length, 0);
  const cat = new Uint8Array(total);
  let o = 0;
  for (const a of parts) {
    cat.set(a, o);
    o += a.length;
  }
  let raw;
  try {
    raw = await deflateDecompress(cat);
  } catch {
    throw new DrawToolError("PNG_CRC", "png idat decompress failed");
  }
  const stride = w * 4;
  if (raw.length !== h * (stride + 1)) {
    throw new DrawToolError("PNG_UNSUPPORTED", "png scanline size mismatch");
  }
  const out = new Uint8ClampedArray(h * stride);
  for (let y = 0; y < h; y++) {
    const f = raw[y * (stride + 1)];
    const src = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let i = 0; i < stride; i++) {
      const a = i >= 4 ? out[y * stride + i - 4] : 0;
      const b = y > 0 ? out[(y - 1) * stride + i] : 0;
      const c = i >= 4 && y > 0 ? out[(y - 1) * stride + i - 4] : 0;
      let v = src[i];
      if (f === 1) v += a;
      else if (f === 2) v += b;
      else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) {
        const pa = Math.abs(b - c);
        const pb = Math.abs(a - c);
        const pc = Math.abs(a + b - 2 * c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      } else if (f !== 0) {
        throw new DrawToolError("PNG_UNSUPPORTED", `unsupported png filter ${f}`);
      }
      out[y * stride + i] = v & 255;
    }
  }
  return { width: w, height: h, rgba: out };
}
