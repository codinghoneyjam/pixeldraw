import { UNDO_MAX_BYTES, UNDO_MAX_STEPS } from "../../core/constants.js";

export class UndoManager {
  constructor({ limitSteps = UNDO_MAX_STEPS, limitBytes = UNDO_MAX_BYTES } = {}) {
    this.limitSteps = limitSteps;
    this.limitBytes = limitBytes;
    this._doc = null;
    this._undo = [];
    this._redo = [];
    this._totalBytes = 0;
    this._savedTop = null;
    this._savedLost = false;
    this._mergeBroken = false;
    this._subs = [];
  }

  setDocument(doc) {
    this._doc = doc;
    this._undo = [];
    this._redo = [];
    this._totalBytes = 0;
    this._savedTop = null;
    this._savedLost = false;
    this._mergeBroken = true;
  }

  _recalcBytes() {
    let total = 0;
    for (const cmd of this._undo) total += cmd.byteSize();
    this._totalBytes = total;
  }

  _enforceBudget() {
    while (this._undo.length > 1 && (this._undo.length > this.limitSteps || this._totalBytes > this.limitBytes)) {
      const [evicted] = this._undo.splice(0, 1);
      this._totalBytes -= evicted.byteSize();
      if (evicted === this._savedTop) {
        this._savedTop = null;
        this._savedLost = true;
      }
    }
  }

  _notify(kind, command) {
    for (const fn of this._subs.slice()) {
      try {
        fn({ kind, command });
      } catch (err) {
        console.error(err);
      }
    }
  }

  commit(cmd, { applied = false } = {}) {
    if (!applied) {
      cmd.do(this._doc);
    }
    const top = this._undo.length > 0 ? this._undo[this._undo.length - 1] : null;
    if (!this._mergeBroken && top && typeof cmd.mergeWith === "function" && cmd.mergeWith(top)) {
      this._redo = [];
      this._recalcBytes();
      this._notify("commit", top);
      return;
    }
    this._undo.push(cmd);
    this._totalBytes += cmd.byteSize();
    this._redo = [];
    this._enforceBudget();
    this._mergeBroken = typeof cmd.mergeWith !== "function";
    this._notify("commit", cmd);
  }

  undo() {
    if (this._undo.length === 0) return false;
    const cmd = this._undo[this._undo.length - 1];
    this._undo.pop();
    try {
      cmd.undo(this._doc);
    } catch (err) {
      this._undo.push(cmd);
      throw err;
    }
    this._totalBytes -= cmd.byteSize();
    this._redo.push(cmd);
    this._mergeBroken = true;
    this._notify("undo", cmd);
    return true;
  }

  redo() {
    if (this._redo.length === 0) return false;
    const cmd = this._redo[this._redo.length - 1];
    this._redo.pop();
    try {
      cmd.do(this._doc);
    } catch (err) {
      this._redo.push(cmd);
      throw err;
    }
    this._undo.push(cmd);
    this._totalBytes += cmd.byteSize();
    this._enforceBudget();
    this._mergeBroken = true;
    this._notify("redo", cmd);
    return true;
  }

  canUndo() {
    return this._undo.length > 0;
  }

  canRedo() {
    return this._redo.length > 0;
  }

  undoLabel() {
    return this._undo.length > 0 ? this._undo[this._undo.length - 1].label : null;
  }

  redoLabel() {
    return this._redo.length > 0 ? this._redo[this._redo.length - 1].label : null;
  }

  breakMerge() {
    this._mergeBroken = true;
  }

  markSaved() {
    this._savedTop = this._undo.length > 0 ? this._undo[this._undo.length - 1] : null;
    this._savedLost = false;
  }

  isDirty() {
    if (this._savedLost) return true;
    const top = this._undo.length > 0 ? this._undo[this._undo.length - 1] : null;
    return top !== this._savedTop;
  }

  clear() {
    this._undo = [];
    this._redo = [];
    this._totalBytes = 0;
    this._savedTop = null;
    this._savedLost = false;
    this._mergeBroken = true;
    this._notify("clear", null);
  }

  subscribe(fn) {
    this._subs.push(fn);
    return () => {
      const i = this._subs.indexOf(fn);
      if (i !== -1) this._subs.splice(i, 1);
    };
  }
}
