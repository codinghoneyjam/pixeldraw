// <META - FILE SUMMARY - Discard-changes confirm dialog>
import { openModal, addButton } from "./dialog_core.js";

// <META - ROLE : Discard-changes confirm | L1-17>
export function confirmDiscardChanges() {
  if (typeof document === "undefined" || !document.getElementById("dt-dialog-root")) return Promise.resolve(false);
  return new Promise((resolve) => {
    openModal({
      title: "변경 버리기",
      onAction: (v) => resolve(v === true),
      build(body, bar, close) {
        const p = document.createElement("p");
        p.textContent = "저장하지 않은 변경이 있습니다. 버리시겠습니까?";
        body.append(p);
        addButton(bar, "취소", () => close(false));
        addButton(bar, "버리기", () => close(true));
      },
    });
  });
}
