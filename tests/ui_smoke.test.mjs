// UI smoke — Node-safe (no DOM, no jsdom).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { STRINGS } from "../src/ui/strings.js";
import { ICONS, iconFor, hasIcon } from "../src/ui/toolbar/icons.js";
import { el, qs, qsa, on, setHidden } from "../src/ui/dom.js";
import { DEFAULT_PALETTE, mountColor } from "../src/ui/color/panel_color.js";
import { SIZE_PRESETS, mountBrush, paintPreview } from "../src/ui/brush/panel_brush.js";
import { mountLayers, paintThumb } from "../src/ui/layers/panel_layers.js";
import { mountOptions } from "../src/ui/optionbar/panel_options.js";
import { mountStatus } from "../src/ui/statusbar/statusbar.js";
import {
  confirmDiscardChanges,
  showNewDocumentDialog,
  showProgress,
  showResizeCanvasDialog,
  showRestoreDialog,
} from "../src/ui/dialogs.js";
import { createShortcuts, isEditableTarget, resolveShortcut } from "../src/ui/toolbar/shortcuts.js";

describe("strings", () => {
  it("has Korean catalog", () => {
    assert.equal(typeof STRINGS.tools.pen, "string");
    for (const id of ["pen", "eraser", "fill", "eyedropper", "line", "rect", "rrect", "ellipse", "polygon", "hand"]) {
      assert.ok(STRINGS.tools[id], id);
      assert.ok(STRINGS.toolShortcuts[id], id);
    }
    for (const a of ["file.new", "file.open", "file.save", "file.exportPng", "edit.undo", "edit.redo", "view.fit", "layer.add"]) {
      assert.ok(STRINGS.actions[a], a);
    }
  });
});

describe("icons", () => {
  it("inline SVG with viewBox + currentColor, fallback char", () => {
    for (const id of ["pen", "eraser", "fill", "eyedropper", "line", "rect", "rrect", "ellipse", "polygon", "hand"]) {
      assert.equal(hasIcon(id), true, id);
      const s = iconFor(id);
      assert.ok(s.includes("viewBox=\"0 0 24 24\""), id);
      assert.ok(s.includes("currentColor"), id);
    }
    assert.equal(typeof iconFor("nope"), "string");
  });
});

describe("dom helpers without DOM", () => {
  it("stub el + null-safe queries", () => {
    const stub = el("div", { id: "a" }, []);
    assert.equal(stub.tag, "div");
    assert.equal(qs(null, "#x"), null);
    assert.deepEqual(qsa(null, "#x"), []);
    const off = on(null, "click", () => {});
    assert.equal(typeof off, "function");
    off();
    setHidden(null, true);
  });
});

describe("panel modules import without DOM", () => {
  it("mount with null root returns dispose", () => {
    for (const mount of [mountColor, mountBrush, mountLayers, mountOptions, mountStatus]) {
      const dispose = mount(null, {});
      assert.equal(typeof dispose, "function");
      dispose();
    }
  });
  it("palette + presets exact", () => {
    assert.equal(DEFAULT_PALETTE.length, 32);
    assert.equal(DEFAULT_PALETTE[0], "#000000");
    assert.equal(DEFAULT_PALETTE[31], "#3a2a1a");
    assert.deepEqual([...SIZE_PRESETS], [1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64]);
  });
  it("paint helpers null-safe", () => {
    assert.equal(paintPreview(null, 3, "#000000"), false);
    assert.equal(paintThumb(null, null), false);
  });
});

describe("shortcuts pure map", () => {
  it("tool keys", () => {
    const cases = { b: "pen", e: "eraser", g: "fill", i: "eyedropper", l: "line", r: "rect", u: "rrect", o: "ellipse", p: "polygon", h: "hand" };
    for (const [key, tool] of Object.entries(cases)) {
      assert.deepEqual(resolveShortcut({ key }), { kind: "tool", tool }, key);
    }
  });
  it("menu + view + layer combos", () => {
    assert.deepEqual(resolveShortcut({ key: "n", ctrl: true }), { kind: "action", action: "file.new" });
    assert.deepEqual(resolveShortcut({ key: "o", ctrl: true }), { kind: "action", action: "file.open" });
    assert.deepEqual(resolveShortcut({ key: "s", ctrl: true }), { kind: "action", action: "file.save" });
    assert.deepEqual(resolveShortcut({ key: "e", ctrl: true }), { kind: "action", action: "file.exportPng" });
    assert.deepEqual(resolveShortcut({ key: "z", ctrl: true }), { kind: "undo" });
    assert.deepEqual(resolveShortcut({ key: "y", ctrl: true }), { kind: "redo" });
    assert.deepEqual(resolveShortcut({ key: "Z", ctrl: true, shift: true }), { kind: "redo" });
    assert.deepEqual(resolveShortcut({ key: "+" }), { kind: "action", action: "view.zoomIn" });
    assert.deepEqual(resolveShortcut({ key: "-" }), { kind: "action", action: "view.zoomOut" });
    assert.deepEqual(resolveShortcut({ key: "0" }), { kind: "action", action: "view.fit" });
    assert.deepEqual(resolveShortcut({ key: "1" }), { kind: "action", action: "view.actual" });
    assert.deepEqual(resolveShortcut({ key: "N", ctrl: true, shift: true }), { kind: "action", action: "layer.add" });
    assert.deepEqual(resolveShortcut({ key: "j", ctrl: true }), { kind: "action", action: "layer.duplicate" });
    assert.deepEqual(resolveShortcut({ key: "x" }), { kind: "swapColors" });
    assert.deepEqual(resolveShortcut({ key: "d" }), { kind: "resetColors" });
    assert.deepEqual(resolveShortcut({ key: "[" }), { kind: "penSize", delta: -1 });
    assert.deepEqual(resolveShortcut({ key: "]", shift: true }), { kind: "penSize", delta: 5 });
  });
  it("handler delegates toolManager first, undo/redos via callbacks", () => {
    let delegated = 0;
    let undos = 0;
    const fakeManager = { keyDown: () => { delegated++; return true; } };
    const { handleKeyDown, dispose } = createShortcuts({
      toolManager: fakeManager,
      requestUndo: () => { undos++; },
    });
    assert.equal(handleKeyDown({ key: "Enter", shift: false, ctrl: false, preventDefault: () => {} }), true);
    assert.equal(delegated, 1);
    assert.equal(handleKeyDown({ key: " ", preventDefault: () => {} }), false);
    assert.equal(isEditableTarget(null), false);
    dispose();
  });
});

describe("dialogs null-safe", () => {
  it("resolve without DOM", async () => {
    assert.equal(await showNewDocumentDialog(), null);
    assert.equal(await confirmDiscardChanges(), false);
    assert.equal(await showRestoreDialog({}), "discard");
    assert.equal(await showResizeCanvasDialog(), null);
    const p = showProgress("x");
    p.update(0.5);
    p.close();
  });
});

describe("icons export surface", () => {
  it("ICONS object present", () => {
    assert.ok(ICONS.pen);
  });
});
