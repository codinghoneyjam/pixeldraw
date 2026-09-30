export function toolEvent(partial = {}) {
  const x = partial.x ?? 0;
  const y = partial.y ?? 0;
  return {
    x,
    y,
    fx: partial.fx ?? x,
    fy: partial.fy ?? y,
    sx: partial.sx ?? x,
    sy: partial.sy ?? y,
    button: partial.button ?? 0,
    shift: partial.shift ?? false,
    ctrl: partial.ctrl ?? false,
    alt: partial.alt ?? false,
    pointerId: partial.pointerId ?? 1,
    pointerType: partial.pointerType ?? "mouse",
    coalesced: Array.isArray(partial.coalesced) && partial.coalesced.length > 0 ? partial.coalesced : [{ x, y }],
    timeStamp: partial.timeStamp ?? 0,
  };
}
