// <META - FILE SUMMARY - Repaint canvas-backed panel content after collapsed section reopens>
import { paintPreview } from "../../features/pen/panel_brush.js";
import { paintThumb } from "../../features/layers/panel_layers.js";

// <META - ROLE : Repaint canvas-backed panel content after a collapsed section reopens | L1-16>
export function refreshPanelCanvases(session) {
  const s = session.settings;
  const preview = document.getElementById("dt-brush-preview");
  if (preview) paintPreview(preview, s.penSize, s.primaryColor);
  const list = document.getElementById("dt-layer-list");
  const doc = session.doc;
  if (!list || !doc) return;
  for (const li of list.querySelectorAll("li[data-layer-id]")) {
    const canvas = li.querySelector("canvas.dt-layer-thumb");
    if (canvas) paintThumb(canvas, doc.findLayer(li.dataset.layerId));
  }
}
