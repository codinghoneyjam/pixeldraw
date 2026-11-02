// <META - FILE SUMMARY - Pending shape construction: polygon placing flow + numeric accessors>
//
// Split out of shape.js: everything that builds or edits the pending object
// outside drag/resize -- the polygon multi-click placing flow and the
// option-bar numeric get/set accessors. shape.js keeps the pointer verbs and
// delegates here. Functions take the tool and touch its underscore state,
// the same convention shape_render.js already uses.

import { DrawToolError } from "../../core/errors.js";
import { snapPlacePoint } from "./shape_geom.js";
import { parsePendingValue } from "./shape_render.js";

// <META - ROLE : polygon vertex append; clicking the first vertex closes | L14-28>
export function placePoint(tool, ev, p) {
  const pts = tool._place.points;
  const first = pts[0];
  const tol = 6 / tool._zoom();
  const fx = ev.fx ?? p.x;
  const fy = ev.fy ?? p.y;
  if (pts.length >= 3 && Math.abs(fx - first.x) <= tol && Math.abs(fy - first.y) <= tol) {
    finishPlacing(tool);
    return;
  }
  pts.push(snapPlacePoint(p, tool._session.settings.snapUnit));
  tool._emitChanged();
  tool._render();
}

// <META - ROLE : placing with >= 3 vertices becomes pending | L31-48>
export function finishPlacing(tool) {
  if (!tool._place || tool._place.points.length < 3) return false;
  tool._pending = {
    layerId: tool._place.layerId,
    color: tool._session.settings.primaryColor,
    bbox: null,
    p0: null,
    p1: null,
    points: tool._place.points.map((q) => ({ ...q })),
  };
  tool._place = null;
  tool._hoverPt = null;
  tool._mode = "pending";
  tool._emitChanged();
  tool._render();
  return true;
}

// <META - ROLE : numeric pending accessors for the option bar | L51-78>
export function readPending(tool) {
  if (!tool._pending) return null;
  const p = tool._pending;
  if (tool._kind === "line") return { x0: p.p0.x, y0: p.p0.y, x1: p.p1.x, y1: p.p1.y };
  if (tool._kind === "polygon") return { points: p.points.map((q) => ({ x: q.x, y: q.y })) };
  return { x: p.bbox.x, y: p.bbox.y, w: p.bbox.w, h: p.bbox.h, radius: p.radius };
}

export function writePending(tool, v) {
  if (v === null || v === undefined) {
    tool.discardPending();
    return;
  }
  if (!tool._session.doc) throw new DrawToolError("INVALID_STATE", "no document loaded");
  const layerId = tool._pending ? tool._pending.layerId : tool._session.doc.activeLayerId;
  // Editing an EXISTING pending shape keeps its captured colour, so nudging the
  // bbox in the option bar after a colour change still cannot recolour it.
  const color = (tool._pending && tool._pending.color) || tool._session.settings.primaryColor;
  const geom = parsePendingValue(tool, v);
  tool._pending = { layerId, color, ...geom };
  tool._draw = null;
  tool._op = null;
  tool._place = null;
  tool._hoverPt = null;
  tool._mode = "pending";
  tool._emitChanged();
  tool._render();
}
