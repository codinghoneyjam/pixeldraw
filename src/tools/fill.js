import { packRGBA, parseHex } from "../core/pixel.js";
import { Tool } from "./tool_base.js";

const FILL_CURSOR =
  "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24'%3E%3Cpath d='M12 3 C12 3 5.5 11.5 5.5 15.5 A6.5 6.5 0 0 0 18.5 15.5 C18.5 11.5 12 3 12 3 Z' fill='white' stroke='black' stroke-width='1.5'/%3E%3Ccircle cx='12' cy='15.5' r='2' fill='black'/%3E%3C/svg%3E\") 12 18, copy";

export function floodFillScanline(writer, get, w, h, sx, sy, seed, fill) {
  if (seed === fill) return;
  const stack = [[sx, sy]];
  while (stack.length > 0) {
    const [cx, cy] = stack.pop();
    if (cx < 0 || cy < 0 || cx >= w || cy >= h) continue;
    if (get(cx, cy) !== seed) continue;
    let l = cx;
    while (l >= 0 && get(l, cy) === seed) l--;
    l++;
    let r = cx;
    while (r < w && get(r, cy) === seed) r++;
    r--;
    for (let i = l; i <= r; i++) writer(i, cy, fill);
    for (const ny of [cy - 1, cy + 1]) {
      if (ny < 0 || ny >= h) continue;
      let i = l;
      while (i <= r) {
        if (get(i, ny) === seed) {
          const s = i;
          while (i <= r && get(i, ny) === seed) i++;
          stack.push([s, ny]);
        } else {
          i++;
        }
      }
    }
  }
}

export class FillTool extends Tool {
  get id() {
    return "fill";
  }

  get cursor() {
    return FILL_CURSOR;
  }

  pointerDown(ev) {
    if (!ev || ev.button !== 0) return;
    const doc = this.session.doc;
    if (!doc) return;
    const w = doc.canvas.widthPx;
    const h = doc.canvas.heightPx;
    if (!Number.isInteger(ev.x) || !Number.isInteger(ev.y) || ev.x < 0 || ev.y < 0 || ev.x >= w || ev.y >= h) return;
    let edit = null;
    try {
      edit = this.session.beginEdit({ label: "페인트통" });
    } catch {
      return;
    }
    const layer = doc.getLayer(doc.activeLayerId);
    const seed = layer.store.getPixel(ev.x, ev.y);
    const c = parseHex(this.session.settings.primaryColor);
    const fill = c ? packRGBA(c.r, c.g, c.b, c.a) : packRGBA(0, 0, 0, 255);
    if (seed === fill) {
      this.session.notify("info", "이미 같은 색입니다");
      try {
        edit.cancel();
      } catch {
        // ignore
      }
      return;
    }
    const writer = edit.writer;
    const get = (x, y) => writer.get(x, y);
    const set = (x, y, v) => writer.set(x, y, v);
    floodFillScanline(set, get, w, h, ev.x, ev.y, seed, fill);
    try {
      edit.commit("페인트통");
    } catch {
      // ignore
    }
  }
}
