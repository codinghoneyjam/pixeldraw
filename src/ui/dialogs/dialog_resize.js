// <META - FILE SUMMARY - Canvas resize dialog>
// Bounds come from core/constants.js so the dialog can never accept a size the
// document layer would then reject (isValidCanvasSize reads the same numbers).
import { openModal, addButton, numberField } from "./dialog_core.js";
import { MIN_SIZE_PX, MAX_W_PX, MAX_H_PX, UNIT_PX } from "../../core/constants.js";

const RANGE_TEXT = `${MIN_SIZE_PX}–${MAX_W_PX} × ${MIN_SIZE_PX}–${MAX_H_PX}, ${UNIT_PX}의 배수여야 합니다`;

// <META - ROLE : Canvas resize dialog | L10-38>
export function showResizeCanvasDialog(current = { widthPx: 512, heightPx: 512 }) {
  if (typeof document === "undefined" || !document.getElementById("dt-dialog-root")) return Promise.resolve(null);
  return new Promise((resolve) => {
    openModal({
      title: "캔버스 크기",
      onAction: (v) => resolve(v),
      build(body, bar, close) {
        const w = numberField(body, "dt-resize-w", "너비(px, 32px 배수)", current.widthPx, UNIT_PX);
        const h = numberField(body, "dt-resize-h", "높이(px, 32px 배수)", current.heightPx, UNIT_PX);
        const err = document.createElement("p");
        err.className = "dt-form-error";
        err.hidden = true;
        body.append(err);
        let okBtn = null;
        const sync = () => {
          const wv = Number(w.value);
          const hv = Number(h.value);
          const ok = Number.isInteger(wv) && Number.isInteger(hv)
            && wv >= MIN_SIZE_PX && wv <= MAX_W_PX
            && hv >= MIN_SIZE_PX && hv <= MAX_H_PX
            && wv % UNIT_PX === 0 && hv % UNIT_PX === 0;
          if (okBtn) okBtn.disabled = !ok;
          err.hidden = ok;
          if (!ok) err.textContent = RANGE_TEXT;
        };
        w.addEventListener("input", sync);
        h.addEventListener("input", sync);
        addButton(bar, "취소", () => close(null));
        okBtn = addButton(bar, "확인", () => close({ widthPx: Number(w.value), heightPx: Number(h.value) }));
        sync();
      },
    });
  });
}
