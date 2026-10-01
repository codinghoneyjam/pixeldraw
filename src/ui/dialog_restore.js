// <META - FILE SUMMARY - Autosave restore choice dialog>
import { openModal, addButton } from "./dialog_core.js";

// <META - ROLE : Autosave restore choice | L1-20>
export function showRestoreDialog(info = {}) {
  if (typeof document === "undefined" || !document.getElementById("dt-dialog-root")) return Promise.resolve("discard");
  return new Promise((resolve) => {
    openModal({
      title: "자동저장 복원",
      onAction: (v) => resolve(v === "restore" ? "restore" : "discard"),
      build(body, bar, close) {
        const p = document.createElement("p");
        const when = info.updatedAt ? new Date(info.updatedAt).toLocaleString() : "";
        p.textContent = `자동저장된 문서${info.name ? ` "${info.name}"` : ""}${when ? ` (${when})` : ""}가 있습니다. 복원하시겠습니까?`;
        body.append(p);
        addButton(bar, "버리기", () => close("discard"));
        addButton(bar, "복원", () => close("restore"));
      },
    });
  });
}
