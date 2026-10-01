// <META - FILE SUMMARY - Shared modal shell: openModal, addButton, numberField>

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
