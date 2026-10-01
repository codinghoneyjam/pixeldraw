// <META - FILE SUMMARY - Progress indicator with aria-busy>

// <META - ROLE : Progress indicator with aria-busy | L1-38>
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
