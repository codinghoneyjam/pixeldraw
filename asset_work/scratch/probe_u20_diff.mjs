import { Session } from "../../src/model/session.js";
import { ShapeTool } from "../../src/tools/shape.js";
import { renderTile } from "../../tools/recipe/render_tile.js";
import { parseColor, resolveColorToken } from "../../tools/recipe/color_tokens.js";
import { toHex, unpackRGBA } from "../../src/core/pixel.js";
import { readFileSync } from "node:fs";

const recipe = JSON.parse(readFileSync("asset_work/target/assetdb/entity/weapon/weapon_sword.json", "utf-8"));
const palette = recipe.palette ?? {};
const layers = recipe.albedo_layers;
const [W, H] = recipe.canvas;
const lit = (v) => {
  const p = parseColor(resolveColorToken(v, palette));
  if (!p) return "#000000";
  return toHex(p[0], p[1], p[2]);
};
const ev = (x, y) => ({ x, y, fx: x, fy: y, button: 0, shift: false, ctrl: false, alt: false });
const key = (k) => ({ key: k, shift: false, ctrl: false, alt: false });
const view = () => ({ zoom: 1, offsetX: 0, offsetY: 0 });
const session = new Session();
session.newDocument({ widthPx: W, heightPx: H });
const tools = {};
for (const k of ["polygon", "line", "rect"]) {
  tools[k] = new ShapeTool({ session, getView: view, requestRender: () => {} }, k);
}
for (const cmd of layers) {
  const type = cmd.cmd ?? cmd.type;
  const fill = cmd.fill ?? cmd.color ? lit(cmd.fill ?? cmd.color) : null;
  const outline = cmd.outline ? lit(cmd.outline) : null;
  session.setSetting("penSize", cmd.width ?? 1);
  const passes = [];
  if (fill && outline) passes.push(["fill", fill], ["outline", outline]);
  else if (fill) passes.push(["fill", fill]);
  else if (outline) passes.push(["outline", outline]);
  else continue;
  for (const [mode, color] of passes) {
    session.setSetting("shapeFill", mode);
    session.setSetting("primaryColor", color);
    if (type === "polygon") {
      const t = tools.polygon;
      for (const [x, y] of cmd.pts ?? cmd.points) t.pointerDown(ev(x, y));
      t.keyDown(key("Enter"));
      t.keyDown(key("Enter"));
    } else if (type === "line") {
      const t = tools.line;
      const pts = cmd.points ?? cmd.pts;
      t.pointerDown(ev(pts[0][0], pts[0][1]));
      t.pointerMove(ev(pts[pts.length - 1][0], pts[pts.length - 1][1]));
      t.pointerUp(ev(pts[pts.length - 1][0], pts[pts.length - 1][1]));
      t.commit();
    }
  }
}
const ref = renderTile(structuredClone(layers), palette, W, H);
const layer = session.doc.getLayer(session.doc.activeLayerId);
const diffs = [];
for (let y = 0; y < H; y++) {
  for (let x = 0; x < W; x++) {
    const a = layer.store.getPixel(x, y);
    const b = ref.data[y * W + x];
    if (a !== b) diffs.push([x, y, unpackRGBA(a), unpackRGBA(b)]);
  }
}
console.log("diff count:", diffs.length);
console.log(diffs.slice(0, 25).map(([x, y, a, b]) => `(${x},${y}) ui=${a} recipe=${b}`).join("\n"));
