// <META - FILE SUMMARY - New-document dialog with tile-step inputs>
import { openModal, addButton, numberField } from "./dialogs/dialog_core.js";

const TILE_MIN = 0.5;
const TILE_MAX_W = 30;
const TILE_MAX_H = 17;

// <META - ROLE : New-document dialog with tile-step inputs | L1-88>
export function showNewDocumentDialog() {
  if (typeof document === "undefined" || !document.getElementById("dt-dialog-root")) return Promise.resolve(null);
  return new Promise((resolve) => {
    let createBtn = null;
    let reason = null;
    const state = { wTiles: 8, hTiles: 8, background: "transparent", name: "Untitled" };
    openModal({
      title: "새 문서",
      onAction: (v) => resolve(v),
      build(body, bar, close) {
        const presets = [
          ["1슬롯 128×128", 2, 2],
          ["16슬롯 512×512", 8, 8],
          ["64슬롯 1024×1024", 16, 16],
          ["풀HD 1920×1088", 30, 17],
        ];
        const presetRow = document.createElement("div");
        presetRow.className = "dt-presets";
        const wInput = numberField(body, "dt-new-w", "너비(타일)", state.wTiles, 0.5);
        const hInput = numberField(body, "dt-new-h", "높이(타일)", state.hTiles, 0.5);
        const pxInfo = document.createElement("p");
        pxInfo.className = "dt-px-info";
        const bgLabel = document.createElement("label");
        bgLabel.setAttribute("for", "dt-new-bg");
        bgLabel.textContent = "배경";
        const bg = document.createElement("select");
        bg.id = "dt-new-bg";
        for (const [v, label] of [["transparent", "투명"], ["#ffffff", "흰색"], ["#000000", "검정"]]) {
          const o = document.createElement("option");
          o.value = v;
          o.textContent = label;
          bg.append(o);
        }
        const nameInput = numberField(body, "dt-new-name", "문서 이름", state.name);
        nameInput.type = "text";
        body.append(bgLabel);
        body.append(bg);
        body.append(pxInfo);
        reason = document.createElement("p");
        reason.className = "dt-form-error";
        reason.hidden = true;
        body.append(reason);
        const sync = () => {
          const w = Number(wInput.value);
          const h = Number(hInput.value);
          state.wTiles = w;
          state.hTiles = h;
          const wPx = Math.round(w * 64);
          const hPx = Math.round(h * 64);
          pxInfo.textContent = `${wPx} × ${hPx} px`;
          const okW = w >= TILE_MIN && w <= TILE_MAX_W && Number.isFinite(w);
          const okH = h >= TILE_MIN && h <= TILE_MAX_H && Number.isFinite(h);
          const okPx = wPx >= 32 && wPx <= 1920 && hPx >= 32 && hPx <= 1088 && wPx % 32 === 0 && hPx % 32 === 0;
          const ok = okW && okH && okPx;
          if (createBtn) createBtn.disabled = !ok;
          if (reason) {
            reason.hidden = ok;
            if (!ok) reason.textContent = `범위: 너비 ${TILE_MIN}–${TILE_MAX_W}, 높이 ${TILE_MIN}–${TILE_MAX_H} 타일 (32px 배수 결과 필요)`;
          }
        };
        for (const [label, wt, ht] of presets) {
          const b = document.createElement("button");
          b.type = "button";
          b.textContent = label;
          b.addEventListener("click", () => {
            wInput.value = String(wt);
            hInput.value = String(ht);
            sync();
          });
          presetRow.append(b);
        }
        body.prepend(presetRow);
        wInput.addEventListener("input", sync);
        hInput.addEventListener("input", sync);
        sync();
        addButton(bar, "취소", () => close(null));
        createBtn = addButton(bar, "만들기", () => {
          const wPx = Math.round(Number(wInput.value) * 64);
          const hPx = Math.round(Number(hInput.value) * 64);
          close({ widthPx: wPx, heightPx: hPx, background: bg.value, name: String(nameInput.value || "Untitled") });
        });
        sync();
      },
    });
  });
}
