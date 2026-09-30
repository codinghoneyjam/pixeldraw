// (1) JS decodes PIL-made PNG chunks from the fixtures; (2) JS-encoded PNG is written for Python/PIL to verify.
import { readFileSync, writeFileSync } from "node:fs";
import { decodePng, encodePng, fromBase64 } from "../reference/png_ref.mjs";
const doc = JSON.parse(readFileSync(new URL("../tests/fixtures/sample_raster_only.json", import.meta.url)));
const out = { decoded: [] };
for (const layer of doc.layers) for (const ch of layer.raster.chunks) {
  const img = await decodePng(fromBase64(ch.png));
  out.decoded.push({ layer: layer.layer_id, cx: ch.cx, cy: ch.cy, w: img.width, h: img.height, px00: [...img.rgba.slice(0, 4)], sum: img.rgba.reduce((a, b) => a + b, 0) });
}
const rgba = new Uint8ClampedArray(32 * 32 * 4);
for (let i = 0; i < 1024; i++) rgba.set([i % 251, (i * 7) % 256, (i * 13) % 256, i % 3 === 0 ? 0 : 255], i * 4);
const png = await encodePng(rgba, 32, 32);
writeFileSync("/tmp/js_encoded.png", png);
out.jsEncodedSum = rgba.reduce((a, b) => a + b, 0);
writeFileSync("/tmp/js_decoded.json", JSON.stringify(out));
console.log("JS decoded", out.decoded.length, "PIL-made chunks; wrote /tmp/js_encoded.png");
