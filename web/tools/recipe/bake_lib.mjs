// Shared helpers for recipe bakers: $token resolution (via the same
// color_tokens.js the transpiler paints with) and geometric offsets.
import { parseColor, resolveColorToken } from "./color_tokens.js";

export const toHex = (rgba) =>
  "#" + rgba.slice(0, 3).map((v) => v.toString(16).padStart(2, "0").toUpperCase()).join("");

export function bakeValue(value, palette) {
  if (Array.isArray(value)) return value.map((v) => bakeValue(v, palette));
  if (value && typeof value === "object") {
    const out = {};
    for (const [k, v] of Object.entries(value)) out[k] = bakeValue(v, palette);
    return out;
  }
  if (typeof value === "string" && value.trim().startsWith("$")) {
    const resolved = resolveColorToken(value, palette);
    const parsed = parseColor(resolved);
    if (!parsed) throw new Error(`cannot resolve token ${value}`);
    if (parsed[3] !== 255) throw new Error(`non-opaque token ${value} -> ${parsed}`);
    return toHex(parsed);
  }
  return value;
}

const isPair = (v) => Array.isArray(v) && v.length >= 2 &&
  typeof v[0] === "number" && typeof v[1] === "number";
const isPairList = (v) => Array.isArray(v) && v.length > 0 && v.every(isPair);

export function offsetCmd(cmd, dx, dy) {
  const out = JSON.parse(JSON.stringify(cmd));
  for (const k of ["box", "bbox"]) {
    if (Array.isArray(out[k]) && out[k].length === 2 && out[k].every(isPair)) {
      out[k] = [[out[k][0][0] + dx, out[k][0][1] + dy], [out[k][1][0] + dx, out[k][1][1] + dy]];
    }
  }
  for (const k of ["points", "pts"]) {
    if (isPairList(out[k])) out[k] = out[k].map(([x, y]) => [x + dx, y + dy]);
  }
  // center/pos/pen are [x, y] paint positions; arc/chord start/end are scalar
  // angles and must NOT be shifted (they fail isPair).
  for (const k of ["center", "pos", "pen", "xy", "start"]) {
    if (isPair(out[k])) out[k] = [out[k][0] + dx, out[k][1] + dy, ...out[k].slice(2)];
  }
  for (const k of ["segments", "lines"]) {
    if (Array.isArray(out[k])) {
      for (const seg of out[k]) {
        if (seg && typeof seg === "object" && !Array.isArray(seg)) {
          for (const pk of ["points", "pts"]) {
            if (isPairList(seg[pk])) seg[pk] = seg[pk].map(([x, y]) => [x + dx, y + dy]);
          }
        } else if (isPairList(seg)) {
          seg.splice(0, seg.length, ...seg.map(([x, y]) => [x + dx, y + dy]));
        }
      }
    }
  }
  return out;
}
