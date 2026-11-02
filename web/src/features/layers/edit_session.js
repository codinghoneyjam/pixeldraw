import { PixelWriter } from "../../core/chunkstore.js";
import { DrawToolError } from "../../core/errors.js";
import { EVENTS } from "../../core/events.js";
import { PaintCommand } from "./commands.js";

export function beginEdit(session, { layerId, label = "연필" } = {}) {
  const doc = session._requireDoc();
  if (session.isEditing) throw new DrawToolError("INVALID_STATE", "already editing");
  const layer = doc.getLayer(layerId ?? doc.activeLayerId);
  if (layer.locked) {
    session.notify("warn", "레이어가 잠겨 있습니다", "LAYER_LOCKED");
    throw new DrawToolError("LAYER_LOCKED", "layer is locked");
  }
  if (!layer.visible) {
    session.notify("warn", "레이어가 숨겨져 있습니다", "LAYER_HIDDEN");
    throw new DrawToolError("LAYER_HIDDEN", "layer is hidden");
  }
  const writer = new PixelWriter(layer.store, layer.id);
  const edit = {
    writer,
    label,
    flush() {
      const dirty = writer.takeDirty();
      if (dirty.length > 0) {
        session.dispatchEvent(
          new CustomEvent(EVENTS.PIXELS_CHANGED, { detail: { layerId: layer.id, chunks: dirty } }),
        );
      }
      return dirty;
    },
    commit(commitLabel) {
      if (session._edit !== edit) throw new DrawToolError("INVALID_STATE", "edit is not active");
      const cs = writer.finish();
      const dirty = writer.takeDirty();
      if (dirty.length > 0) {
        session.dispatchEvent(
          new CustomEvent(EVENTS.PIXELS_CHANGED, { detail: { layerId: layer.id, chunks: dirty } }),
        );
      }
      session._edit = null;
      if (cs) {
        session.record(new PaintCommand(cs, commitLabel ?? label));
        return true;
      }
      return false;
    },
    cancel() {
      if (session._edit !== edit) throw new DrawToolError("INVALID_STATE", "edit is not active");
      const dirty = writer.discard();
      if (dirty.length > 0) {
        session.dispatchEvent(
          new CustomEvent(EVENTS.PIXELS_CHANGED, { detail: { layerId: layer.id, chunks: dirty } }),
        );
      }
      session._edit = null;
    },
  };
  session._edit = edit;
  return edit;
}
