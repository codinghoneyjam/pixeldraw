// <META - FILE SUMMARY - Minimal fake DOM mirroring index.html's optionbar, for section tests>
// The optionbar is the only ui block whose logic depends on real element
// plumbing (visibility per tool, disabled matrices, a select rebuilt at mount).
// Node has no DOM, so the tests drive it through this fake instead: it answers
// exactly the queries the mounts use and records listener wiring so dispose can
// be asserted.
export class FakeNode {
  constructor(tag = "div") {
    this.tag = tag;
    this.attrs = {};
    this.dataset = {};
    this.children = [];
    this.listeners = new Map();
    this.style = {};
    this.value = "";
    this.textContent = "";
    this.checked = false;
    this.disabled = false;
    this.hidden = false;
  }

  addEventListener(type, fn) {
    if (!this.listeners.has(type)) this.listeners.set(type, []);
    this.listeners.get(type).push(fn);
  }

  removeEventListener(type, fn) {
    const list = this.listeners.get(type) ?? [];
    const i = list.indexOf(fn);
    if (i >= 0) list.splice(i, 1);
  }

  // <META - ROLE : fire every handler of one event type | L38-41>
  emit(type, event = {}) {
    for (const fn of [...(this.listeners.get(type) ?? [])]) fn({ target: this, ...event });
  }

  setAttribute(k, v) { this.attrs[k] = String(v); }
  getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; }
  hasAttribute(name) { return name in this.attrs; }
  removeAttribute(name) { delete this.attrs[name]; }

  // <META - ROLE : hidden toggling, the only attribute the mounts write | L49-54>
  toggleAttribute(name, force) {
    const on = force === undefined ? !this.hasAttribute(name) : !!force;
    if (on) this.attrs[name] = "";
    else delete this.attrs[name];
    return on;
  }

  append(...nodes) { this.children.push(...nodes); }
  replaceChildren(...nodes) { this.children = [...nodes]; }

  contains(node) {
    if (node === this) return true;
    return this.children.some((c) => c && typeof c.contains === "function" && c.contains(node));
  }

  // <META - ROLE : total handlers still attached, for the dispose assertion | L68-70>
  get listenerCount() {
    let n = 0;
    for (const l of this.listeners.values()) n += l.length;
    return n;
  }
}

function section(tools, name) {
  const el = new FakeNode("section");
  el.dataset.forTools = tools.join(" ");
  if (name) el.dataset.section = name;
  return el;
}

// <META - ROLE : Build the optionbar tree and an id index, shaped like index.html | L74-118>
export function makeOptionbar() {
  const byId = new Map();
  const sections = [];
  const mk = (id, tag = "input") => {
    const n = new FakeNode(tag);
    if (id) byId.set("#" + id, n);
    return n;
  };

  const gridSel = mk("dt-grid-mode", "select");
  const zoomSel = mk("dt-zoom-select", "select");
  const penSizeLabel = mk("dt-option-pen-size", "span");

  // section(tools, name, ...children) - `append` returns void like the real one,
  // so children are passed straight to the constructor of the entry.
  const add = (tools, name, children) => {
    const el = section(tools, name);
    el.append(...children);
    sections.push(el);
    return el;
  };

  add(
    ["pen", "eraser", "fill", "eyedropper", "line", "rect", "rrect", "ellipse", "polygon", "hand"],
    "view",
    [gridSel, mk("dt-zoom-fit", "button"), mk("dt-zoom-actual", "button"), zoomSel],
  );
  add(["pen", "eraser"], null, [penSizeLabel]);

  const shapeControls = [
    mk("dt-shape-fill-toggle", "button"), mk("dt-shape-radius"),
    mk("dt-shape-lock"), mk("dt-snap-unit"),
    mk("dt-shape-x"), mk("dt-shape-y"), mk("dt-shape-w"), mk("dt-shape-h"),
    mk("dt-shape-commit", "button"), mk("dt-shape-cancel", "button"),
  ];
  // Named aliases: the array order is index.html's markup order, but tests read
  // far clearer through names than through five skipped slots.
  const shape = {
    fillToggle: shapeControls[0], radius: shapeControls[1],
    lock: shapeControls[2], snap: shapeControls[3],
    x: shapeControls[4], y: shapeControls[5], w: shapeControls[6], h: shapeControls[7],
    commit: shapeControls[8], cancel: shapeControls[9],
  };
  add(["line", "rect", "rrect", "ellipse", "polygon"], "shape", shapeControls);
  add(["fill"], null, []);
  add(["eyedropper"], null, []);
  add(["hand"], null, []);

  const root = new FakeNode("div");
  root.append(...sections);
  root.querySelector = (sel) => byId.get(sel) ?? null;
  root.querySelectorAll = (sel) => (sel === "[data-for-tools]" ? sections : []);
  return { root, sections, byId, penSizeLabel, gridSel, zoomSel, shapeControls, shape };
}

// <META - ROLE : Install a document/window pair, return a restore handle | L120-134>
// The optionbar mounts read `document.activeElement` to avoid stealing a focused
// field, so assertions must run while the fake is installed - it is not enough
// to wrap the mount. Tests install it in `before` and restore in `after`.
export function installDom() {
  const doc = new FakeNode("#document");
  doc.body = new FakeNode("body");
  doc.createElement = (tag) => new FakeNode(tag);
  doc.activeElement = null;
  const win = { innerWidth: 1280, innerHeight: 800 };
  const prev = { doc: globalThis.document, win: globalThis.window };
  globalThis.document = doc;
  globalThis.window = win;
  return () => {
    globalThis.document = prev.doc;
    globalThis.window = prev.win;
  };
}

// <META - ROLE : Run fn with a document/window pair installed | L136-145>
export function withDom(fn) {
  const restore = installDom();
  try {
    return fn();
  } finally {
    restore();
  }
}
