// <META - FILE SUMMARY - Edit actions: requestUndo, requestRedo>

// <META - ROLE : Undo with pending tool check | L1-12>
export function requestUndo(session, toolManager) {
  try {
    const t = toolManager ? toolManager.active : null;
    if (t && typeof t.hasPending === "function" && t.hasPending()) {
      t.discardPending();
      return;
    }
  } catch { /* fall through */ }
  session.undo();
}

// <META - ROLE : Redo with pending tool check | L14-22>
export function requestRedo(session, toolManager) {
  try {
    const t = toolManager ? toolManager.active : null;
    if (t && typeof t.hasPending === "function" && t.hasPending()) return;
  } catch { /* fall through */ }
  session.redo();
}
