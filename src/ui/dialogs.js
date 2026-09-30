// <META - FILE SUMMARY - Promise dialogs: focus trap, Esc cancel, aria-modal, null-safe>
function dialogRoot() {
  if (typeof document === "undefined") return null;
  return document.getElementById("dt-dialog-root");
}

function focusables(box) {
  return [...box.querySelectorAll("button,[href],input,select,textarea,[tabindex]:not([tabindex='-1'])")]
    .filter((n) => !n.disabled && n.getAttribute("aria-hidden") !== "true");
}

// <META - ROLE : Generic modal shell returning close(result) | L14-68>
function openModal({ title, build, onAction }) {
  const root = dialogRoot();
  if (!root) return { close: () => {}, element: null };
  const overlay = document.createElement("div");
  overlay.className = "dt-dialog-overlay";
  const box = document.createElement("div");
  box.className = "dt-dialog";
  box.setAttribute("role", "dialog");
  box.setAttribute("aria-modal", "true");
  box.setAttribute("aria-label", title);
  box.tabIndex = -1;
  const titleEl = document.createElement("h2");
  titleEl.textContent = title;
  box.append(titleEl);
  const body = document.createElement("div");
  body.className = "dt-dialog-body";
  box.append(body);
  const bar = document.createElement("div");
  bar.className = "dt-dialog-buttons";
  box.append(bar);
  overlay.append(box);
  root.append(overlay);
  let settled = false;
  const prevFocus = document.activeElement;
  function close(result) {
    if (settled) return;
    settled = true;
    overlay.remove();
    document.removeEventListener("keydown", onKey, true);
    if (prevFocus && typeof prevFocus.focus === "function") {
      try { prevFocus.focus(); } catch { /* ignore */ }
    }
    onAction(result);
  }
  function onKey(e) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close(null);
    } else if (e.key === "Tab") {
      const list = focusables(box);
      if (list.length === 0) return;
      const first = list[0];
      const last = list[list.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }
  document.addEventListener("keydown", onKey, true);
  overlay.addEventListener("pointerdown", (e) => {
    if (e.target === overlay) close(null);
  });
  const api = build(body, bar, close);
  const first = focusables(box)[0] ?? box;
  try { first.focus(); } catch { /* ignore */ }
  return { close, element: box, extra: api };
}

function addButton(bar, label, onClick, opts = {}) {
  const b = document.createElement("button");
  b.type = "button";
  b.textContent = label;
  if (opts.disabled) b.disabled = true;
  if (opts.id) b.id = opts.id;
  b.addEventListener("click", onClick);
  bar.append(b);
  return b;
}

function numberField(body, id, label, value, step) {
  const wrap = document.createElement("label");
  wrap.setAttribute("for", id);
  wrap.textContent = label;
  const input = document.createElement("input");
  input.type = "number";
  input.id = id;
  input.value = String(value);
  if (step !== undefined) input.step = String(step);
  body.append(wrap);
  body.append(input);
  return input;
}

const TILE_MIN = 0.5;
const TILE_MAX_W = 30;
const TILE_MAX_H = 17;

// <META - ROLE : New-document dialog with tile-step inputs | L110-190>
export function showNewDocumentDialog() {
  if (!dialogRoot()) return Promise.resolve(null);
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

// <META - ROLE : Discard-changes confirm | L192-210>
export function confirmDiscardChanges() {
  if (!dialogRoot()) return Promise.resolve(false);
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

// <META - ROLE : Autosave restore choice | L212-232>
export function showRestoreDialog(info = {}) {
  if (!dialogRoot()) return Promise.resolve("discard");
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

// <META - ROLE : Canvas resize dialog (SHOULD) | L234-268>
export function showResizeCanvasDialog(current = { widthPx: 512, heightPx: 512 }) {
  if (!dialogRoot()) return Promise.resolve(null);
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

// <META - ROLE : Progress indicator with aria-busy | L270-306>
export function showProgress(text) {
  if (typeof document === "undefined") return { update() {}, close() {} };
  const app = document.getElementById("dt-app");
  const root = dialogRoot();
  const state = { closed: false, prevBusy: app ? app.getAttribute("aria-busy") : null };
  if (app) app.setAttribute("aria-busy", "true");
  let bar = null;
  let label = null;
  if (root) {
    const wrap = document.createElement("div");
    wrap.className = "dt-progress";
    wrap.setAttribute("role", "status");
    label = document.createElement("span");
    label.textContent = text;
    const track = document.createElement("div");
    track.className = "dt-progress-track";
    bar = document.createElement("div");
    bar.className = "dt-progress-bar";
    track.append(bar);
    wrap.append(label);
    wrap.append(track);
    root.append(wrap);
    state.node = wrap;
  }
  return {
    update(pct) {
      if (state.closed || !bar) return;
      const v = Math.max(0, Math.min(1, Number(pct) || 0));
      bar.style.width = `${Math.round(v * 100)}%`;
    },
    close() {
      if (state.closed) return;
      state.closed = true;
      if (state.node) state.node.remove();
      if (app) {
        if (state.prevBusy === null) app.removeAttribute("aria-busy");
        else app.setAttribute("aria-busy", state.prevBusy);
      }
    },
  };
}
