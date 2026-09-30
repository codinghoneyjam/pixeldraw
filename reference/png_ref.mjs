// NON-NORMATIVE reference PNG codec: 8-bit RGBA, non-interlaced only. Uses CompressionStream('deflate') = zlib format.
const SIG = [137, 80, 78, 71, 13, 10, 26, 10];
const CRC = (() => { const t = new Uint32Array(256); for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; } return t; })();
const crc32 = (u8) => { let c = 0xffffffff; for (const b of u8) c = CRC[(c ^ b) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };

async function pipe(u8, stream) {
  const out = new Blob([u8]).stream().pipeThrough(stream);
  return new Uint8Array(await new Response(out).arrayBuffer());
}
const be32 = (v) => [(v >>> 24) & 255, (v >>> 16) & 255, (v >>> 8) & 255, v & 255];
function chunk(type, data) {
  const body = new Uint8Array(4 + data.length);
  for (let i = 0; i < 4; i++) body[i] = type.charCodeAt(i);
  body.set(data, 4);
  return [...be32(data.length), ...body, ...be32(crc32(body))];
}

export async function encodePng(rgba, w, h) {
  const raw = new Uint8Array(h * (1 + w * 4));               // filter type 0 per row
  for (let y = 0; y < h; y++) raw.set(rgba.subarray(y * w * 4, (y + 1) * w * 4), y * (1 + w * 4) + 1);
  const idat = await pipe(raw, new CompressionStream("deflate"));
  const ihdr = new Uint8Array([...be32(w), ...be32(h), 8, 6, 0, 0, 0]);
  return Uint8Array.from([...SIG, ...chunk("IHDR", ihdr), ...chunk("IDAT", idat), ...chunk("IEND", new Uint8Array(0))]);
}

export async function decodePng(u8) {
  for (let i = 0; i < 8; i++) if (u8[i] !== SIG[i]) throw new Error("PNG_SIGNATURE");
  const dv = new DataView(u8.buffer, u8.byteOffset, u8.byteLength);
  let p = 8, w = 0, h = 0; const parts = [];
  while (p < u8.length) {
    const len = dv.getUint32(p), type = String.fromCharCode(...u8.subarray(p + 4, p + 8)), data = u8.subarray(p + 8, p + 8 + len);
    if (crc32(u8.subarray(p + 4, p + 8 + len)) !== dv.getUint32(p + 8 + len)) throw new Error("PNG_CRC");
    if (type === "IHDR") {
      w = new DataView(data.buffer, data.byteOffset).getUint32(0); h = new DataView(data.buffer, data.byteOffset).getUint32(4);
      if (data[8] !== 8 || data[9] !== 6 || data[12] !== 0) throw new Error("PNG_UNSUPPORTED (need 8-bit RGBA, non-interlaced)");
    } else if (type === "IDAT") parts.push(data);
    else if (type === "IEND") break;
    p += 12 + len;
  }
  const cat = new Uint8Array(parts.reduce((s, a) => s + a.length, 0)); let o = 0;
  for (const a of parts) { cat.set(a, o); o += a.length; }
  const raw = await pipe(cat, new DecompressionStream("deflate"));
  const stride = w * 4, out = new Uint8ClampedArray(h * stride);
  for (let y = 0; y < h; y++) {
    const f = raw[y * (stride + 1)], src = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let i = 0; i < stride; i++) {
      const a = i >= 4 ? out[y * stride + i - 4] : 0, b = y ? out[(y - 1) * stride + i] : 0, c = i >= 4 && y ? out[(y - 1) * stride + i - 4] : 0;
      let v = src[i];
      if (f === 1) v += a; else if (f === 2) v += b; else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) { const pa = Math.abs(b - c), pb = Math.abs(a - c), pc = Math.abs(a + b - 2 * c); v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c; }
      else if (f !== 0) throw new Error("PNG_UNSUPPORTED (bad filter)");
      out[y * stride + i] = v & 255;
    }
  }
  return { width: w, height: h, rgba: out };
}
export const toBase64 = (u8) => Buffer.from(u8).toString("base64");       // browser: btoa(String.fromCharCode(...chunks))
export const fromBase64 = (s) => new Uint8Array(Buffer.from(s, "base64"));
