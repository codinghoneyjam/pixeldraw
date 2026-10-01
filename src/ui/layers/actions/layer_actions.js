// <META - FILE SUMMARY - Layer actions: add, duplicate, remove, mergeDown, up, down>

// <META - ROLE : Layer CRUD action handlers | L1-30>
export function layerAdd(session) { session.addLayer(); }

export function layerDuplicate(session) {
  session.duplicateLayer(session.doc.activeLayerId);
}

export function layerRemove(session) {
  session.removeLayer(session.doc.activeLayerId);
}

export function layerMergeDown(session) {
  session.mergeDown(session.doc.activeLayerId);
}

export function layerUp(session) {
  const d = session.doc;
  session.moveLayer(d.activeLayerId, d.indexOf(d.activeLayerId) + 1);
}

export function layerDown(session) {
  const d = session.doc;
  session.moveLayer(d.activeLayerId, d.indexOf(d.activeLayerId) - 1);
}
