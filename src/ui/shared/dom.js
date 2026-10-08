// <META - FILE SUMMARY - Minimal DOM helpers, Node-safe (stub el without document)>
function isBrowser() {
  return typeof document !== "undefined";
}

// <META - ROLE : Create element or Node-safe stub | L8-32>
export function el(tag, attrs = {}, children = []) {
  if (!isBrowser()) return { tag, attrs: { ...attrs }, children: [...children] };
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === undefined || v === null || v === false) continue;
    if (k === "text") node.textContent = String(v);
    else if (k === "html") node.innerHTML = String(v);
    else if (k.startsWith("on") && typeof v === "function") {
      node.addEventListener(k.slice(2).toLowerCase(), v);
    } else if (typeof v === "boolean") {
      if (v) node.setAttribute(k, "");
    } else node.setAttribute(k, String(v));
  }
  for (const c of children) {
    if (c === null || c === undefined || c === false) continue;
    node.append(c);
  }
  return node;
}

// <META - ROLE : Query helpers | L34-44>
export function qs(root, sel) {
  const base = root ?? (isBrowser() ? document : null);
  if (!base || typeof base.querySelector !== "function") return null;
  return base.querySelector(sel);
}

export function qsa(root, sel) {
  const base = root ?? (isBrowser() ? document : null);
  if (!base || typeof base.querySelectorAll !== "function") return [];
  return [...base.querySelectorAll(sel)];
}

// <META - ROLE : Event attach returning detach | L46-52>
export function on(target, type, fn, opts) {
  if (!target || typeof target.addEventListener !== "function") return () => {};
  target.addEventListener(type, fn, opts);
  return () => target.removeEventListener(type, fn, opts);
}

// <META - ROLE : Toggle hidden attribute | L54-60>
export function setHidden(node, hidden) {
  if (!node || typeof node.toggleAttribute !== "function") return;
  node.toggleAttribute("hidden", hidden === true);
}

export function clearChildren(node) {
  if (!node || typeof node.replaceChildren !== "function") return;
  try {
    node.replaceChildren();
  } catch { /* ignore */ }
}
