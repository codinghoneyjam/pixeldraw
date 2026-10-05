// <META - FILE SUMMARY - ShapeTool render side: commit write, preview spec, paint layers, overlay.
//
// Split out of shape.js: this half of the tool owns raster output (history write,
// preview masks, canvas overlay). shape.js keeps the interactive pending state
// machine and delegates its render-facing methods here.

import { applyShape } from "../core/raster/shape_raster.js";
import { buildSpec, drawSpec, geomFromSpec, layoutHandles, parseBoxGeom, parseLineGeom } from "./shape_geom.js";
import { computePaintLayers, packOf, renderOverlay } from "./shape_overlay.js";

// <META - ROLE : colour captured WHEN THE SHAPE WAS DRAWN | L13-19>
//
// The pending shape deliberately survives pointer-up so it can still be resized,
// and every later apply (confirm button, resize-drag release, numeric bbox edit)
// used to read `settings.primaryColor` fresh -- so changing colour after drawing
// recoloured the object just made. `pending.color` is the snapshot that stops that.
export function buildColor(tool) {
  const snap = tool._pending && tool._pending.color;
  return snap || tool._session.settings.primaryColor;
}

// <META - ROLE : commit spec for the pending primitive | L21-23>
export function buildPendingSpec(tool) {
  return buildSpec(tool._kind, tool._pending, tool._session.settings);
}

// <META - ROLE : numeric bbox/line edit from the option bar; keeps the captured colour | L25-27>
export function parsePendingValue(tool, v) {
  return tool._kind === "line" ? parseLineGeom(v) : parseBoxGeom(v, tool._session.settings.shapeRadius);
}

// <META - ROLE : apply the pending shape as one history entry | L29-65>
export function commitPending(tool) {
  if (!tool._pending) return false;
  const layerId = tool._pending.layerId;
  const label = tool._label();
  let edit;
  try {
    edit = tool._session.beginEdit({ layerId, label });
  } catch (err) {
    if (err && (err.code === "LAYER_LOCKED" || err.code === "LAYER_HIDDEN" || err.code === "LAYER_NOT_FOUND")) {
      tool.discardPending();
      return false;
    }
    throw err;
  }
  // Only the FOREGROUND colour may paint. The background slot is a buffer only, so
  // it is passed as the same colour rather than being allowed to write pixels.
  const color = buildColor(tool);
  applyShape(edit.writer, buildPendingSpec(tool), packOf(color), packOf(color));
  edit.commit(label);
  tool._pending = null;
  tool._op = null;
  tool._mode = "idle";
  tool._emitChanged();
  tool._render();
  return true;
}

// <META - ROLE : spec for the in-progress drag, else the pending shape | L67-70>
export function previewSpec(tool) {
  if (tool._mode === "drawing" && tool._draw) return drawSpec(tool._kind, tool._draw, tool._session.settings);
  return tool._pending ? buildPendingSpec(tool) : null;
}

// <META - ROLE : cached paint layers for the preview, keyed by spec + colour | L72-80>
export function previewPaintLayers(tool, spec) {
  // Cache key must include the captured colour, otherwise a colour change would not
  // invalidate the cached layers and the preview would keep the old colour.
  const key = JSON.stringify(spec) + "|" + buildColor(tool);
  if (tool._masks.key !== key) {
    tool._masks = { key, layers: computePaintLayers(spec, buildColor(tool)) };
  }
  return tool._masks.layers;
}

// <META - ROLE : WYSIWYG preview pixels into a scratch writer | L82-88>
export function paintPreview(tool, writer) {
  const spec = previewSpec(tool);
  if (!spec) return 0;
  const color = buildColor(tool);
  return applyShape(writer, spec, packOf(color), packOf(color));
}

// <META - ROLE : canvas overlay: preview pixels, dashed bbox, handles | L90-102>
export function renderToolOverlay(tool, ctx, info = {}) {
  const spec = previewSpec(tool);
  if (!spec || !ctx) return;
  const view = info.view ?? tool._view() ?? { zoom: 1, offsetX: 0, offsetY: 0 };
  const dpr = info.dpr ?? 1;
  const box = spec.kind === "line"
    ? { x: Math.min(spec.p0.x, spec.p1.x), y: Math.min(spec.p0.y, spec.p1.y), w: Math.abs(spec.p1.x - spec.p0.x) + 1, h: Math.abs(spec.p1.y - spec.p0.y) + 1 }
    : { ...spec.bbox };
  renderOverlay(ctx, spec, previewPaintLayers(tool, spec), box, layoutHandles(tool._kind, geomFromSpec(spec)), view, dpr, tool._tmp);
}