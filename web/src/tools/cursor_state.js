// <META - FILE SUMMARY - Cursor state machine: the outside > panning > space > tool priority ladder>
//
// Split out of input_controller.js: the whole reason to move this out is that the
// ladder is a pure decision and the DOM write is a two-line tail. Keeping them
// apart makes the ladder testable in Node with a plain object, no fake host.
// DOM-free by contract (docs/contract.md section 4), same as the rest of the
// pointer-logic half of src/tools/.

// <META - ROLE : the four fixed outcomes of the ladder | L15-17>
export const DEFAULT_CURSOR = "default";
export const PAN_CURSOR = "grabbing";
export const SPACE_CURSOR = "grab";

// <META - ROLE : pick the winning cursor: outside beats panning beats space beats tool | L19-27>
export function resolveCursor({ inside = true, panning = false, spaceDown = false, toolCursor = null } = {}) {
  if (!inside) return DEFAULT_CURSOR;
  if (panning) return PAN_CURSOR;
  if (spaceDown) return SPACE_CURSOR;
  return typeof toolCursor === "string" && toolCursor.length > 0 ? toolCursor : DEFAULT_CURSOR;
}

// <META - ROLE : read an InputController's cursor inputs, tolerating any missing piece | L29-44>
export function readCursorState(ctrl = {}) {
  return {
    inside: ctrl._inside !== false,
    panning: ctrl._panning === true,
    spaceDown: ctrl._spaceDown === true,
    toolCursor: readToolCursor(ctrl),
  };
}

// <META - ROLE : pull the active tool's cursor, falling back when the tool throws | L46-53>
export function readToolCursor(ctrl = {}) {
  try {
    const c = ctrl.toolManager?.cursor;
    return typeof c === "string" && c.length > 0 ? c : DEFAULT_CURSOR;
  } catch {
    return DEFAULT_CURSOR;
  }
}

// <META - ROLE : write the winning cursor onto the host element, no-op without one | L55-64>
export function applyCursor(ctrl = {}) {
  const next = resolveCursor(readCursorState(ctrl));
  try {
    if (ctrl.host) ctrl.host.style.cursor = next;
  } catch {
    // ignore (node, or a host that was torn down mid-gesture)
  }
  return next;
}

// <META - ROLE : set pointer presence, then always re-apply the ladder | L66-70>
export function markInside(ctrl = {}, inside = true) {
  ctrl._inside = inside === true;
  applyCursor(ctrl);
  return ctrl._inside;
}
