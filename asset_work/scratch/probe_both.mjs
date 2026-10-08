import { Session } from "../../src/model/session.js";
import { ShapeTool } from "../../src/tools/shape.js";
import { applyCommand } from "../../tools/recipe/render_tile.js";
import { ChunkStore, PixelWriter } from "../../src/core/chunkstore.js";
import { parseColor, resolveColorToken } from "../../tools/recipe/color_tokens.js";
import { packRGBA, toHex } from "../../src/core/pixel.js";
import { readFileSync } from "node:fs";

const recipe = JSON.parse(readFileSync("asset_work/target/assetdb/entity/weapon/weapon_sword.json", "utf-8"));
const palette = recipe.palette ?? {};
const cmd = recipe.albedo_layers[5]; // guard polygon, both-mode
console.log(JSON.stringify(cmd));

// Path A: recipe applyCommand (both in one call).
const storeA = new ChunkStore(128, 128);
const wA = new PixelWriter(storeA, "a");
applyCommand({ writer: wA, palette, width: 128, height: 128 }, structuredClone(cmd));
wA.finish();

// Path B: UI split (fill commit, then outline commit).
const session = new Session();
session.newDocument({ widthPx: 128, heightPx: 128 });
const t = new ShapeTool({ session, getView: () => ({ zoom: 1, offsetX: 0, offsetY: 0 }), requestRender: () => {} }, "polygon");
const ev = (x, y) => ({ x, y, fx: x, fy: y, button: 0, shift: false, ctrl: false, alt: false });
const key = (k) => ({ key: k, shift: false, ctrl: false, alt: false });
const lit = (v) => { const p = parseColor(resolveColorToken(v, palette)); return toHex(p[0], p[1], p[2]); };
for (const [mode, col] of [["fill", lit(cmd.fill)], ["outline", lit(cmd.outline)]]) {
  session.setSetting("shapeFill", mode);
  session.setSetting("primaryColor", col);
  for (const [x, y] of cmd.pts) t.pointerDown(ev(x, y));
  t.keyDown(key("Enter"));
  t.keyDown(key("Enter"));
}
const layer = session.doc.getLayer(session.doc.activeLayerId);
let diff = 0;
for (let y = 0; y < 128; y++) {
  for (let x = 0; x < 128; x++) {
    if (layer.store.getPixel(x, y) !== storeA.getPixel(x, y)) {
      if (diff < 10) console.log(`(${x},${y}) ui=${layer.store.getPixel(x, y) >>> 0} recipe=${storeA.getPixel(x, y) >>> 0}`);
      diff++;
    }
  }
}
console.log("single-cmd diff:", diff);
console.log("pending spec was:", JSON.stringify(t.getPending()));
