import { ChunkStore } from "../../core/chunkstore.js";
import { isValidCanvasSize } from "../../core/constants.js";
import { DrawToolError } from "../../core/errors.js";
import { BLEND_MODES } from "../../core/blend.js";

// The canonical list lives in core/blend.js (contract §1-1); re-exported here so
// `features/layers` consumers keep one import site.
export { BLEND_MODES };
export const LAYER_PROP_FIELDS = Object.freeze(["name", "visible", "locked", "opacity", "blend"]);

export function validateLayerName(name) {
  if (typeof name !== "string" || name.length === 0 || name.length > 128 || name.trim().length === 0) {
    throw new DrawToolError("INVALID_STATE", "layer name must be 1-128 non-blank chars");
  }
}

export function validateLayerOpacity(opacity) {
  if (typeof opacity !== "number" || !Number.isFinite(opacity) || opacity < 0 || opacity > 1) {
    throw new DrawToolError("OUT_OF_RANGE", "layer opacity must be a finite number in [0,1]");
  }
}

export function validateLayerBlend(blend) {
  // Membership, not just a non-empty string: schema/vector import also lands here,
  // and "dissolve" must be rejected rather than silently demoted.
  if (!BLEND_MODES.includes(blend)) {
    throw new DrawToolError("INVALID_STATE", `unknown layer blend ${String(blend)}`);
  }
}

export class Layer {
  constructor(id, name, visible, locked, opacity, blend, store) {
    validateLayerName(name);
    validateLayerOpacity(opacity);
    validateLayerBlend(blend);
    this.id = id;
    this.name = name;
    this.visible = visible;
    this.locked = locked;
    this.opacity = opacity;
    this.blend = blend;
    this.store = store;
  }

  static create({ id, name, widthPx, heightPx }) {
    if (!isValidCanvasSize(widthPx, heightPx)) {
      throw new DrawToolError("CANVAS_SIZE_INVALID", `invalid canvas size ${widthPx}x${heightPx}`);
    }
    validateLayerName(name);
    return new Layer(id, name, true, false, 1, "normal", new ChunkStore(widthPx, heightPx));
  }

  cloneWith({ id, name }) {
    validateLayerName(name);
    return new Layer(id, name, this.visible, this.locked, this.opacity, this.blend, this.store.clone());
  }

  setProp(field, value) {
    if (field === "name") {
      validateLayerName(value);
      this.name = value;
    } else if (field === "visible") {
      this.visible = value === true;
    } else if (field === "locked") {
      this.locked = value === true;
    } else if (field === "opacity") {
      validateLayerOpacity(value);
      this.opacity = value;
    } else if (field === "blend") {
      validateLayerBlend(value);
      this.blend = value;
    } else {
      throw new DrawToolError("INVALID_STATE", `unknown layer field ${String(field)}`);
    }
  }
}
