// <META - FILE SUMMARY - Collapsible panel sections: persist/restore via localStorage>

const COLLAPSE_STORE_KEY = "dt.settings.v1";
const COLLAPSE_FIELD = "collapsedPanels";

function readJson(key) {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return null;
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function writeJson(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch { /* ignore */ }
}

// <META - ROLE : Load the collapsed-section id map, tolerate a missing/corrupt store | L18-24>
function loadCollapsed() {
  const saved = readJson(COLLAPSE_STORE_KEY);
  const field = saved && typeof saved === "object" ? saved[COLLAPSE_FIELD] : null;
  return field && typeof field === "object" && !Array.isArray(field) ? { ...field } : {};
}

// <META - ROLE : Merge one id into the collapsed map, keeping every other setting | L26-32>
function persistCollapsed(id, collapsed) {
  const saved = readJson(COLLAPSE_STORE_KEY);
  const base = saved && typeof saved === "object" && !Array.isArray(saved) ? saved : {};
  const next = { ...base, [COLLAPSE_FIELD]: { ...loadCollapsed(), [id]: collapsed } };
  writeJson(COLLAPSE_STORE_KEY, next);
}

// <META - ROLE : Apply collapsed state to a section, syncing data attr + aria | L34-39>
function applyCollapsed(section, head, collapsed) {
  section.dataset.collapsed = collapsed ? "true" : "false";
  head.setAttribute("aria-expanded", collapsed ? "false" : "true");
}

// <META - ROLE : Wire collapsible panel sections, persist state, return dispose | L41-73>
export function mountCollapsibleSections(roots, deps = {}) {
  if (typeof document === "undefined") return () => {};
  const { onExpand = null } = deps;
  const list = Array.isArray(roots) ? roots : [roots];
  const disposers = [];
  const stored = loadCollapsed();
  for (const root of list) {
    if (!root || typeof root.querySelectorAll !== "function") continue;
    for (const section of root.querySelectorAll("section[data-collapsed]")) {
      const head = section.querySelector(".dt-panel-head");
      const body = section.querySelector(".dt-panel-body");
      if (!head || !body) continue;
      const id = section.id || "";
      const onClick = () => {
        const collapsed = section.dataset.collapsed !== "true";
        applyCollapsed(section, head, collapsed);
        if (id) persistCollapsed(id, collapsed);
        if (!collapsed && typeof onExpand === "function") {
          try { onExpand(id, section); } catch { /* ignore */ }
        }
      };
      head.addEventListener("click", onClick);
      disposers.push(() => head.removeEventListener("click", onClick));
      applyCollapsed(section, head, stored[id] === true);
    }
  }
  return () => {
    for (const d of disposers) {
      try { d(); } catch { /* ignore */ }
    }
    disposers.length = 0;
  };
}
