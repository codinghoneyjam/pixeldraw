// Shared source-over blend (docs/core.md, blend.js). Exact op order — do not reorder.

export function rnd(v) {
  return Math.floor(v + 0.5);
}

export function over(dst, src, opacity) {
  const sa = (src[3] / 255) * opacity;
  if (sa === 0) return [dst[0], dst[1], dst[2], dst[3]];
  const da = dst[3] / 255;
  const oa = sa + da * (1 - sa);
  const c0 = rnd((src[0] * sa + dst[0] * da * (1 - sa)) / oa);
  const c1 = rnd((src[1] * sa + dst[1] * da * (1 - sa)) / oa);
  const c2 = rnd((src[2] * sa + dst[2] * da * (1 - sa)) / oa);
  const a = rnd(oa * 255);
  if (a === 0) return [0, 0, 0, 0];
  return [c0, c1, c2, a];
}
