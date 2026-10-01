// <META - FILE SUMMARY - View store: zoom/offset state with clamp + subscribe>
import { actualSizeView, clampView, fitView, stepZoom, zoomAt } from "../render/view.js";

// <META - ROLE : View store owned by app with clamp + subscribe | L1-60>
export function createViewStore(host, session) {
  let view = { zoom: 1, offsetX: 0, offsetY: 0 };
  const subs = new Set();
  function viewport() {
    let w = 800;
    let h = 600;
    try {
      if (host && host.clientWidth > 0) w = host.clientWidth;
      if (host && host.clientHeight > 0) h = host.clientHeight;
    } catch { /* ignore */ }
    return { w, h };
  }
  function clamp(v) {
    const doc = session.doc;
    if (!doc) return { ...v };
    const vp = viewport();
    try {
      return clampView(v, doc.canvas.widthPx, doc.canvas.heightPx, vp.w, vp.h);
    } catch {
      return { ...v };
    }
  }
  function emit() {
    for (const fn of subs) {
      try { fn(view); } catch { /* ignore */ }
    }
  }
  return {
    get() {
      return { ...view };
    },
    set(v) {
      view = clamp({ ...v });
      emit();
    },
    subscribe(fn) {
      subs.add(fn);
      return () => subs.delete(fn);
    },
    center() {
      const vp = viewport();
      return { x: vp.w / 2, y: vp.h / 2 };
    },
    zoomTo(z) {
      if (!Number.isFinite(z) || z <= 0) return;
      const c = this.center();
      this.set(zoomAt(view, c.x, c.y, z));
    },
    zoomIn() {
      this.zoomTo(stepZoom(view.zoom, 1));
    },
    zoomOut() {
      this.zoomTo(stepZoom(view.zoom, -1));
    },
    fit() {
      const doc = session.doc;
      if (!doc) return;
      const vp = viewport();
      this.set(fitView(doc.canvas.widthPx, doc.canvas.heightPx, vp.w, vp.h));
    },
    actual() {
      const doc = session.doc;
      if (!doc) return;
      const vp = viewport();
      this.set(actualSizeView(doc.canvas.widthPx, doc.canvas.heightPx, vp.w, vp.h));
    },
  };
}
