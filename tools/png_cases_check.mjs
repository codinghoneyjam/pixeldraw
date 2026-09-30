import { readFileSync } from "node:fs";
import { decodePng, fromBase64 } from "../reference/png_ref.mjs";
const { cases } = JSON.parse(readFileSync(new URL("../tests/fixtures/png_cases.json", import.meta.url)));
let ok = 0;
for (const c of cases) {
  try {
    const img = await decodePng(fromBase64(c.png));
    if (c.expect_error) throw new Error(`${c.name}: expected ${c.expect_error}, got success`);
    const hex = Buffer.from(img.rgba).toString("hex");
    if (hex !== c.rgba_hex || img.width !== c.width || img.height !== c.height) throw new Error(`${c.name}: pixel mismatch`);
    ok++;
  } catch (e) {
    if (!c.expect_error || !String(e.message).startsWith(c.expect_error)) throw new Error(`${c.name}: unexpected error ${e.message}`);
    ok++;
  }
}
console.log(`png cases OK: ${ok}/${cases.length}`);
