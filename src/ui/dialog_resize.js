// <META - FILE SUMMARY - Canvas resize dialog>
import { openModal, addButton, numberField } from "./dialogs/dialog_core.js";

// <META - ROLE : Canvas resize dialog | L1-32>
export function showResizeCanvasDialog(current = { widthPx: 512, heightPx: 512 }) {
  if (typeof document === "undefined" || !document.getElementById("dt-dialog-root")) return Promise.resolve(null);
  return new Promise((resolve) => {
    openModal({
      title: "캔버스 크기",
      onAction: (v) => resolve(v),
      build(body, bar, close) {
        const w = numberField(body, "dt-resize-w", "너비(px, 32px 배수)", current.widthPx, 32);
        const h = numberField(body, "dt-resize-h", "높이(px, 32px 배수)", current.heightPx, 32);
        const err = document.createElement("p");
        err.className = "dt-form-error";
        err.hidden = true;
        body.append(err);
        let okBtn = null;
        const sync = () => {
          const wv = Number(w.value);
          const hv = Number(h.value);
          const ok = Number.isInteger(wv) && Number.isInteger(hv)
            && wv >= 32 && wv <= 1920 && hv >= 32 && hv <= 1088 && wv % 32 === 0 && hv % 32 === 0;
          if (okBtn) okBtn.disabled = !ok;
          err.hidden = ok;
          if (!ok) err.textContent = "32–1920 × 32–1088, 32의 배수여야 합니다";
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
