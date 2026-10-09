// <META - FILE SUMMARY - Action id -> handler table built from domain action modules>
import { doSave, doOpen, doExportLayer, doImportLayer, doExportPng, doNew, doImportRecipe, doExportVector, doImportVector } from "./file_actions.js";
import { requestUndo, requestRedo, canvasResize } from "./edit_actions.js";
import { zoomIn, zoomOut, fit, actual, gridCycle } from "../../features/viewport/view_actions.js";
import { colorSwap, colorReset } from "../../features/color/color_actions.js";
import { brushStep } from "../../features/pen/brush_actions.js";
import { layerAdd, layerDuplicate, layerRemove, layerMergeDown, layerUp, layerDown } from "../../features/layers/layer_actions.js";

// <META - ROLE : Build the menubar/shortcut action table for a live session | L8-38>
export function buildActions({ session, toolManager, viewStore, runAction, toast }) {
  return {
    "file.new": () => runAction(() => doNew(session)),
    "file.open": () => runAction(() => doOpen(session, toast)),
    "file.save": () => runAction(() => doSave(session, toast)),
    "file.exportLayer": () => runAction(() => doExportLayer(session, toast)),
    "file.importLayer": () => runAction(() => doImportLayer(session, toast)),
    "file.exportPng": () => runAction(() => doExportPng(session, toast)),
    "file.importRecipe": () => runAction(() => doImportRecipe(session, toast)),
    "file.exportVector": () => runAction(() => doExportVector(session, toast)),
    "file.importVector": () => runAction(() => doImportVector(session, toast)),
    "edit.undo": () => runAction(() => requestUndo(session, toolManager)),
    "edit.redo": () => runAction(() => requestRedo(session, toolManager)),
    "canvas.resize": () => runAction(() => canvasResize(session)),
    "view.zoomIn": () => zoomIn(viewStore),
    "view.zoomOut": () => zoomOut(viewStore),
    "view.fit": () => fit(viewStore),
    "view.actual": () => actual(viewStore),
    "view.gridCycle": () => runAction(() => gridCycle(session)),
    "layer.add": () => runAction(() => layerAdd(session)),
    "layer.duplicate": () => runAction(() => layerDuplicate(session)),
    "layer.remove": () => runAction(() => layerRemove(session)),
    "layer.mergeDown": () => runAction(() => layerMergeDown(session)),
    "layer.up": () => runAction(() => layerUp(session)),
    "layer.down": () => runAction(() => layerDown(session)),
    "brush.step": (delta) => runAction(() => brushStep(session, delta)),
    "color.swap": () => runAction(() => colorSwap(session)),
    "color.reset": () => runAction(() => colorReset(session)),
  };
}
