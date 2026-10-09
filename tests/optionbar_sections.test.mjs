// Optionbar section tests — fake DOM, no browser needed (slice-13 regression net).
//
// These cover the seam the optionbar split created: the shell owns the generic
// `[data-for-tools]` visibility loop and the `data-section` -> feature-module
// delegation, while each block owns its own enable/disable rules. A failure here
// means a block either never shows or shows controls it should have disabled.
import { describe, it, before, after } from "node:test";
import assert from "node:assert/strict";
import { mountOptions } from "../src/ui/optionbar/panel_options.js";
import { ZOOM_LEVELS } from "../src/core/constants.js";
import { installDom, makeOptionbar, withDom } from "./helpers/fake_optionbar.js";
import { makeSession, envFor } from "./helpers/tool_session.js";
import { PenTool } from "../src/features/pen/pen.js";
import { ShapeTool } from "../src/features/shape/shape.js";
import { ToolManager } from "../src/tools/tool_manager.js";

// The optionbar reads document.activeElement while syncing, so the fake DOM has
// to stay installed for the whole file, not just around the mount.
let restoreDom;
before(() => { restoreDom = installDom(); });
after(() => { restoreDom(); });

// <META - ROLE : mount the optionbar against a live session + shape tool | L21-33>
function mount() {
  const session = makeSession();
  const getView = () => envFor(session).getView;
  const toolManager = new ToolManager({ session, getView: envFor(session).getView, requestRender: () => {} });
  for (const kind of ["line", "rect", "rrect", "ellipse", "polygon"]) {
    toolManager.register(new ShapeTool({ session, getView: envFor(session).getView, requestRender: () => {} }, kind));
  }
  toolManager.register(new PenTool(envFor(session)));
  const bar = makeOptionbar();
  const view = { get: () => ({ zoom: 1 }), subscribe: () => () => {}, zoomTo: () => {}, fit: () => {}, actual: () => {} };
  const dispose = mountOptions(bar.root, { session, toolManager, view });
  return { session, toolManager, bar, dispose, view };
}

function hidden(bar, index) {
  return bar.sections[index].hasAttribute("hidden");
}

describe("optionbar section visibility", () => {
  it("shows exactly the sections that serve the active tool", () => {
    const { session, bar, dispose } = mount();
    // view section serves every tool; shape only the five shape kinds.
    assert.equal(hidden(bar, 0), false, "view section shows for pen");
    assert.equal(hidden(bar, 2), true, "shape section hidden for pen");

    session.setSetting("activeTool", "rect");
    assert.equal(hidden(bar, 0), false);
    assert.equal(hidden(bar, 2), false, "shape section shows for rect");

    session.setSetting("activeTool", "hand");
    assert.equal(hidden(bar, 2), true, "shape section hides again for hand");
    dispose();
  });

  it("pen size label tracks the setting", () => {
    const { session, bar, dispose } = mount();
    assert.equal(bar.penSizeLabel.textContent, "1px");
    session.setSetting("penSize", 12);
    assert.equal(bar.penSizeLabel.textContent, "12px");
    dispose();
  });

  it("grid select reflects and writes gridMode", () => {
    const { session, bar, dispose } = mount();
    assert.equal(bar.gridSel.value, "off");
    session.setSetting("gridMode", "tile");
    assert.equal(bar.gridSel.value, "tile");
    bar.gridSel.value = "pixel";
    bar.gridSel.emit("change");
    assert.equal(session.settings.gridMode, "pixel");
    dispose();
  });

  it("zoom select is built once from ZOOM_LEVELS", () => {
    const { bar, dispose } = mount();
    assert.equal(bar.zoomSel.children.length, ZOOM_LEVELS.length);
    assert.equal(bar.zoomSel.children[0].value, "0.25");
    assert.equal(bar.zoomSel.children[0].textContent, "25%");
    dispose();
  });
});

describe("optionbar shape block enable matrix", () => {
  it("line disables the fill radios, rrect is the only tool with a radius", () => {
    const { session, bar, dispose } = mount();
    session.setSetting("activeTool", "line");
    const { outline, fill, radius, lock, snap, x, commit, cancel } = bar.shape;
    assert.equal(outline.disabled, true, "line cannot fill");
    assert.equal(fill.disabled, true);
    assert.equal(radius.disabled, true, "radius is rrect-only");
    assert.equal(lock.disabled, false);
    assert.equal(snap.disabled, false);
    // No pending geometry yet, so nothing is editable or committable.
    assert.equal(x.disabled, true);
    assert.equal(commit.disabled, true);
    assert.equal(cancel.disabled, true);

    session.setSetting("activeTool", "rrect");
    assert.equal(outline.disabled, false, "rrect can fill");
    assert.equal(radius.disabled, false, "rrect owns the radius");
    dispose();
  });

  it("TOOL_STATE fills the bbox and enables commit, and polygon keeps the box blank", () => {
    const { session, toolManager, bar, dispose } = mount();
    session.setSetting("activeTool", "rect");
    const tool = toolManager.get("rect");
    tool.setPending({ x: 0, y: 0, w: 4, h: 4 });
    session.emitToolState("rect", tool.getPending());
    assert.equal(bar.shape.x.value, "0");
    assert.equal(bar.shape.y.value, "0");
    assert.equal(bar.shape.w.value, "4");
    assert.equal(bar.shape.h.value, "4");
    assert.equal(bar.shape.commit.disabled, false, "pending makes bbox + commit live");

    // A vertex-based pending carries points, not a bbox: fields stay blank but
    // the shape is still committable.
    session.setSetting("activeTool", "polygon");
    const poly = toolManager.get("polygon");
    poly.setPending({ points: [{ x: 0, y: 0 }, { x: 8, y: 8 }, { x: 0, y: 8 }] });
    session.emitToolState("polygon", poly.getPending());
    assert.equal(bar.shape.x.value, "", "polygon keeps the bbox fields blank");
    assert.equal(bar.shape.commit.disabled, false, "polygon pending is still committable");
    dispose();
  });

  it("TOOL_STATE for another tool is ignored", () => {
    const { session, bar, dispose } = mount();
    session.setSetting("activeTool", "pen");
    session.emitToolState("rect", { x: 1, y: 2, w: 3, h: 4 });
    assert.equal(bar.shape.x.value, "", "inactive tool must not drive the section");
    assert.equal(bar.shape.commit.disabled, true);
    dispose();
  });

  it("bbox edit writes back through setPending", () => {
    const { session, toolManager, bar, dispose } = mount();
    session.setSetting("activeTool", "rect");
    const tool = toolManager.get("rect");
    tool.setPending({ x: 0, y: 0, w: 4, h: 4 });
    session.emitToolState("rect", tool.getPending());
    bar.shape.x.value = "9";
    bar.shape.y.value = "9";
    bar.shape.w.value = "2";
    bar.shape.h.value = "2";
    bar.shape.x.emit("change");
    assert.deepEqual(tool.getPending(), { x: 9, y: 9, w: 2, h: 2, radius: 8 });
    dispose();
  });
});

describe("optionbar mount lifecycle", () => {
  it("dispose stops every update it started", () => {
    const { session, bar, dispose } = mount();
    session.setSetting("penSize", 12);
    assert.equal(bar.penSizeLabel.textContent, "12px");
    dispose();
    session.setSetting("penSize", 20);
    assert.equal(bar.penSizeLabel.textContent, "12px", "dispose leaves no live listener");
    session.setSetting("gridMode", "tile");
    assert.equal(bar.gridSel.value, "off", "view block stopped listening too");
  });

  it("mount with a null root is a no-op", () => {
    const off = withDom(() => mountOptions(null, {}));
    assert.equal(typeof off, "function");
    off();
  });
});
