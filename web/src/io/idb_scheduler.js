// <META - FILE SUMMARY - Debounce scheduler with force flush cap>

const DEBOUNCE_MS = 800;
const FORCE_MS = 5000;

// <META - ROLE : Scheduler for debounce with force cap | L8-42>
export class FlushScheduler {
  constructor() {
    this._timer = null;
    this._forceTimer = null;
  }

  schedule(callback) {
    if (!this._timer) {
      this._timer = setTimeout(() => {
        this._timer = null;
        if (this._forceTimer) {
          clearTimeout(this._forceTimer);
          this._forceTimer = null;
        }
        callback();
      }, DEBOUNCE_MS);
    }
    if (!this._forceTimer) {
      this._forceTimer = setTimeout(() => {
        this._forceTimer = null;
        if (this._timer) {
          clearTimeout(this._timer);
          this._timer = null;
        }
        callback();
      }, FORCE_MS);
    }
  }

  cancel() {
    if (this._timer) {
      clearTimeout(this._timer);
      this._timer = null;
    }
    if (this._forceTimer) {
      clearTimeout(this._forceTimer);
      this._forceTimer = null;
    }
  }
}

export const _INTERNALS = { DEBOUNCE_MS, FORCE_MS };
