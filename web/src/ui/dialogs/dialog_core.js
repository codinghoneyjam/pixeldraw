// <META - FILE SUMMARY - Shared modal shell: openModal, addButton, numberField, confirmDiscardChanges, showRestoreDialog, showProgress>

function dialogRoot() {
  if (typeof document === "undefined") return null;
  return document.getElementById("dt-dialog-root");
}

function focusables(box) {
  return [...box.querySelectorAll("button,[href],input,select,textarea,[tabindex]:not([tabindex='-1'])")]
    .filter((n) => !n.disabled && n.getAttribute("aria-hidden") !== "true");
}

// <META - ROLE : Generic modal shell returning close(result) | L14-68>
export function openModal({ title, build, onAction }) {
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

export function addButton(bar, label, onClick, opts = {}) {
  const b = document.createElement("button");
  b.type = "button";
  b.textContent = label;
  if (opts.disabled) b.disabled = true;
  if (opts.id) b.id = opts.id;
  b.addEventListener("click", onClick);
  bar.append(b);
  return b;
}

export function numberField(body, id, label, value, step) {
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

// <META - ROLE : Discard-changes confirm dialog | L102-119>
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

export function showLayerExportFormatDialog() {
  if (typeof document === "undefined" || !dialogRoot()) return Promise.resolve(null);
  return new Promise((resolve) => {
    openModal({
      title: "레이어 저장 형식",
      onAction: resolve,
      build(body, bar, close) {
        const p = document.createElement("p");
        p.textContent = "픽셀 청크는 무손실이며, 명령 형식은 현재 픽셀을 rect/pixel 명령으로 근사합니다.";
        body.append(p);
        addButton(bar, "픽셀 청크 JSON", () => close("chunks"));
        addButton(bar, "명령 레이어 JSON", () => close("commands"));
        addButton(bar, "취소", () => close(null));
      },
    });
  });
}

// <META - ROLE : Autosave restore choice dialog | L121-141>
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

// <META - ROLE : Progress indicator with aria-busy | L143-186>
export function showProgress(text) {
  if (typeof document === "undefined") return { update() {}, close() {} };
  const app = document.getElementById("dt-app");
  const root = document.getElementById("dt-dialog-root");
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
