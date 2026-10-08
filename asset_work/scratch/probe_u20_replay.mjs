// Phase-1 proof: replay U20 sword albedo_layers through the UI ShapeTools
// (polygon placing, line/rect drags) and compare against the recipe renderer.
import { Session } from "../../src/features/document/session.js";
import { ShapeTool } from "../../src/features/shape/shape.js";
import { renderTile } from "../../tools/recipe/render_tile.js";
import { parseColor, resolveColorToken } from "../../tools/recipe/color_tokens.js";
import { toHex } from "../../src/core/pixel.js";
import { readFileSync } from "node:fs";

const recipe = JSON.parse(readFileSync("asset_work/target/assetdb/entity/weapon/weapon_sword.json", "utf-8"));
const palette = recipe.palette ?? {};
const layers = recipe.albedo_layers;
const [W, H] = recipe.canvas;

const lit = (v) => {
  const r = resolveColorToken(v, palette);
  const p = parseColor(r);
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

let nPoly = 0, nLine = 0, nRect = 0;
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
      nPoly++;
      const t = tools.polygon;
      for (const [x, y] of cmd.pts ?? cmd.points) t.pointerDown(ev(x, y));
      t.keyDown(key("Enter"));
      t.keyDown(key("Enter"));
    } else if (type === "line") {
      nLine++;
      const t = tools.line;
      const pts = cmd.points ?? cmd.pts;
      t.pointerDown(ev(pts[0][0], pts[0][1]));
      t.pointerMove(ev(pts[pts.length - 1][0], pts[pts.length - 1][1]));
      t.pointerUp(ev(pts[pts.length - 1][0], pts[pts.length - 1][1]));
      t.commit();
    } else if (type === "rect" || type === "rectangle") {
      nRect++;
      const t = tools.rect;
      const [[x0, y0], [x1, y1]] = cmd.box ?? cmd.bbox;
      t.pointerDown(ev(x0, y0));
      t.pointerMove(ev(x1, y1));
      t.pointerUp(ev(x1, y1));
      t.commit();
    } else {
      console.log("UNHANDLED BY UI:", type);
    }
  }
}

// Reference: same commands through the recipe renderer with the same palette.
const ref = renderTile(structuredClone(layers), palette, W, H);
const layer = session.doc.getLayer(session.doc.activeLayerId);
let diff = 0, total = 0;
for (let y = 0; y < H; y++) {
  for (let x = 0; x < W; x++) {
    total++;
    if (layer.store.getPixel(x, y) !== ref.data[y * W + x]) diff++;
  }
}
console.log(`cmds polygon=${nPoly} line=${nLine} rect=${nRect}`);
console.log(`UI replay vs recipe render: ${total - diff}/${total} match, diff=${diff}`);
if (diff !== 0) process.exitCode = 1;
