import { DrawToolError } from "../../core/errors.js";
import { SETTING_KEYS } from "../../core/events.js";
import { GRID_MODES } from "../../core/constants.js";
import { parseHex, toHex, toHex8 } from "../../core/pixel.js";

export const TOOL_IDS = Object.freeze([
  "pen",
  "eraser",
  "eyedropper",
  "fill",
  "line",
  "rect",
  "rrect",
  "ellipse",
  "polygon",
  "hand",
]);

const SHAPE_FILLS = Object.freeze(["outline", "fill"]);

function validateColor(value) {
  const c = parseHex(value);
  // Fully transparent paint would silently erase, so alpha 0 is rejected.
  if (c === null || c.a === 0) {
    throw new DrawToolError("OUT_OF_RANGE", `invalid color ${String(value)}`);
  }
  // Canonical form keeps alpha only when translucent: opaque stays #rrggbb.
  return c.a === 255 ? toHex(c.r, c.g, c.b) : toHex8(c.r, c.g, c.b, c.a);
}

function validateInt(value, min, max, what) {
  if (!Number.isInteger(value) || value < min || value > max) {
    throw new DrawToolError("OUT_OF_RANGE", `invalid ${what} ${String(value)}`);
  }
  return value;
}

export function validateSetting(key, value) {
  if (!SETTING_KEYS.includes(key)) {
    throw new DrawToolError("INVALID_STATE", `unknown setting ${String(key)}`);
  }
  if (key === "primaryColor" || key === "secondaryColor") return validateColor(value);
  if (key === "penSize") return validateInt(value, 1, 64, "penSize");
  if (key === "activeTool") {
    if (!TOOL_IDS.includes(value)) throw new DrawToolError("OUT_OF_RANGE", `unknown tool ${String(value)}`);
    return value;
  }
  if (key === "gridMode") {
    if (!GRID_MODES.includes(value)) throw new DrawToolError("OUT_OF_RANGE", `unknown gridMode ${String(value)}`);
    return value;
  }
  if (key === "snapUnit" || key === "shapeLockAspect") {
    if (typeof value !== "boolean") throw new DrawToolError("OUT_OF_RANGE", `invalid ${key}`);
    return value;
  }
  if (key === "shapeFill") {
    if (!SHAPE_FILLS.includes(value)) throw new DrawToolError("OUT_OF_RANGE", `unknown shapeFill ${String(value)}`);
    return value;
  }
  if (key === "shapeRadius") return validateInt(value, 0, 2048, "shapeRadius");
  throw new DrawToolError("INVALID_STATE", `unknown setting ${String(key)}`);
}
