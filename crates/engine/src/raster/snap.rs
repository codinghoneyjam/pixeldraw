//! Shape geometry helpers the tools need while dragging — ported from
//! `src/core/raster/raster_snap.js`.
//!
//! These are pure integer/float functions with no document or DOM access, which
//! is why they live next to the rasterisers rather than with the tools: the
//! golden `angle_snap`·`resize`·`drag_bbox`·`unit_snap` groups (37 cases) exercise
//! them directly, exactly the way `tests/shape_geom.test.mjs` does on the JS side.
//!
//! **Coordinate domain.** The JS tree spoke `Number` here. This port keeps `i64`
//! for the box functions, because a drag can legitimately produce a box whose
//! origin is thousands of pixels off-canvas (`oob_drawing.test.mjs` paints at
//! `-5000`), and `resize_bbox`'s aspect-lock branch multiplies two extents.
//! Pixel writes then clip, as they always did.

use crate::blend::rnd;

/// `angleSnap`'s default step, in degrees.
pub const DEFAULT_ANGLE_STEP: f64 = 15.0;

/// Snap a drag angle to a `step`-degree grid, keeping the length.
///
/// `Math.atan2` / `Math.hypot` / `Math.cos` / `Math.sin` all map to the `f64`
/// methods of the same name, and [`rnd`] is `Math.floor(v + 0.5)` — so the
/// operation order below is what keeps the result bit-identical to JS.
///
/// Returns `f64` rather than an integer type on purpose: `rnd` produces
/// integral values, but re-typing them here would hide that from the next
/// reader. Callers that want pixels cast with `as i64`, which is exact.
pub fn angle_snap(x0: i32, y0: i32, x1: i32, y1: i32, step: f64) -> [f64; 2] {
    let (x0, y0, x1, y1) = (x0 as f64, y0 as f64, x1 as f64, y1 as f64);
    let dx = x1 - x0;
    let dy = y1 - y0;
    if dx == 0.0 && dy == 0.0 {
        return [x1, y1];
    }
    let st = (step * std::f64::consts::PI) / 180.0;
    let s = rnd(dy.atan2(dx) / st) * st;
    let l = dx.hypot(dy);
    [x0 + rnd(l * s.cos()), y0 + rnd(l * s.sin())]
}

/// [`angle_snap`] at the default 15° step.
pub fn angle_snap_deg(x0: i32, y0: i32, x1: i32, y1: i32) -> [f64; 2] {
    angle_snap(x0, y0, x1, y1, DEFAULT_ANGLE_STEP)
}

/// Fresh-draw bounding box from a drag vector: `[x, y, w, h]`.
///
/// `lock` forces a square, `center` grows the box symmetrically around `p0`.
/// A drag of zero extent is a 1×1 box, not a 0×0 one — the JS rule, and the
/// reason a click draws a single pixel.
pub fn drag_bbox(p0: (i64, i64), p1: (i64, i64), lock: bool, center: bool) -> [i64; 4] {
    let (ax, ay) = p0;
    let (bx, by) = p1;
    let dx = bx - ax;
    let dy = by - ay;
    let adx = dx.abs();
    let ady = dy.abs();
    if center {
        let mut w = 2 * adx + 1;
        let mut h = 2 * ady + 1;
        if lock {
            w = w.max(h);
            h = w;
        }
        return [ax - (w - 1).div_euclid(2), ay - (h - 1).div_euclid(2), w, h];
    }
    let mut w = adx + 1;
    let mut h = ady + 1;
    if lock {
        w = w.max(h);
        h = w;
    }
    [
        if dx >= 0 { ax } else { ax - w + 1 },
        if dy >= 0 { ay } else { ay - h + 1 },
        w,
        h,
    ]
}

/// Resize a pending box by dragging one of its eight handles.
///
/// Ported from the reference port, so the anchor/lo-hi arithmetic is identical.
/// `min_size` is 1 everywhere in the app; it is a parameter because the
/// reference implementation had one.
pub fn resize_bbox(
    b: [i64; 4],
    handle: &str,
    p: (i64, i64),
    lock: bool,
    center: bool,
    min_size: i64,
) -> [i64; 4] {
    let [x, y, w, h] = b;
    let (px, py) = p;
    let hx = if handle.contains('e') {
        1
    } else if handle.contains('w') {
        -1
    } else {
        0
    };
    let hy = if handle.contains('s') {
        1
    } else if handle.contains('n') {
        -1
    } else {
        0
    };
    let axis = |pos: i64, size: i64, hd: i64, pv: i64| -> (i64, i64) {
        if hd == 0 {
            return (pos, size);
        }
        if center {
            let delta = if hd == 1 {
                pv - (pos + size - 1)
            } else {
                pos - pv
            };
            let mut ns = size + 2 * delta;
            if ns < min_size {
                ns = min_size;
            }
            let at = if ns == min_size && delta < 0 {
                pos + (size - ns).div_euclid(2)
            } else {
                pos - delta
            };
            return (at, ns);
        }
        let anchor = if hd == 1 { pos } else { pos + size - 1 };
        let lo = anchor.min(pv);
        let hi = anchor.max(pv);
        (lo, hi - lo + 1)
    };
    let (mut nx, mut nw) = axis(x, w, hx, px);
    let (mut ny, mut nh) = axis(y, h, hy, py);
    if lock && (hx != 0 || hy != 0) {
        if hx != 0 && hy != 0 {
            // The reference compares cross products instead of ratios, so the
            // choice does not depend on the rounding order.
            if nw * h > nh * w {
                nh = min_size.max(rnd((nw * h) as f64 / w as f64) as i64);
            } else {
                nw = min_size.max(rnd((nh * w) as f64 / h as f64) as i64);
            }
        } else if hx != 0 {
            nh = min_size.max(rnd((nw * h) as f64 / w as f64) as i64);
        } else {
            nw = min_size.max(rnd((nh * w) as f64 / h as f64) as i64);
        }
        let place = |pos: i64, size: i64, hd: i64, pv: i64, ns: i64| -> i64 {
            if center || hd == 0 {
                return pos + (size - ns).div_euclid(2);
            }
            let anchor = if hd == 1 { pos } else { pos + size - 1 };
            if pv >= anchor {
                anchor
            } else {
                anchor - ns + 1
            }
        };
        nx = place(x, w, hx, px, nw);
        ny = place(y, h, hy, py, nh);
    }
    [nx, ny, nw, nh]
}

/// Snap a box to the 32px unit grid, never smaller than one unit.
pub fn unit_snap(b: [i64; 4]) -> [i64; 4] {
    const UNIT: f64 = crate::constants::UNIT_PX as f64;
    const ONE_UNIT: i64 = crate::constants::MIN_SIZE_PX as i64;
    let [x, y, w, h] = b.map(|v| v as f64);
    let r = |v: f64| (rnd(v) as i64) * ONE_UNIT;
    [
        r(x / UNIT),
        r(y / UNIT),
        ONE_UNIT.max(r(w / UNIT)),
        ONE_UNIT.max(r(h / UNIT)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::group;

    fn pair(case: &serde_json::Value, key: &str) -> (i64, i64) {
        let a = case[key].as_array().expect(key);
        (a[0].as_i64().unwrap(), a[1].as_i64().unwrap())
    }

    fn quad(case: &serde_json::Value, key: &str) -> [i64; 4] {
        let a = case[key].as_array().expect(key);
        [
            a[0].as_i64().unwrap(),
            a[1].as_i64().unwrap(),
            a[2].as_i64().unwrap(),
            a[3].as_i64().unwrap(),
        ]
    }

    // <META - ROLE : golden angle_snap group, all 8 cases | L201-207>
    #[test]
    fn golden_angle_snap_cases() {
        for case in group("angle_snap") {
            let p0 = pair(case, "p0");
            let p1 = pair(case, "p1");
            let got = angle_snap_deg(p0.0 as i32, p0.1 as i32, p1.0 as i32, p1.1 as i32);
            let want = pair(case, "expect");
            assert_eq!(
                [got[0] as i64, got[1] as i64],
                [want.0, want.1],
                "angle_snap {p0:?} -> {p1:?}"
            );
        }
    }

    // <META - ROLE : golden drag_bbox group, all 8 cases | L240-248>
    #[test]
    fn golden_drag_bbox_cases() {
        for case in group("drag_bbox") {
            let got = drag_bbox(
                pair(case, "p0"),
                pair(case, "p1"),
                case["lock"].as_bool().unwrap(),
                case["center"].as_bool().unwrap(),
            );
            assert_eq!(got, quad(case, "expect"), "drag_bbox {case}");
        }
    }

    // <META - ROLE : golden resize group, all 15 cases | L210-237>
    #[test]
    fn golden_resize_cases() {
        for case in group("resize") {
            let got = resize_bbox(
                quad(case, "bbox"),
                crate::fixtures::str_field(case, "handle"),
                pair(case, "pointer"),
                case["lock"].as_bool().unwrap(),
                case["center"].as_bool().unwrap(),
                1,
            );
            assert_eq!(got, quad(case, "expect"), "resize {case}");
        }
    }

    // <META - ROLE : golden unit_snap group, all 6 cases | L251-253>
    #[test]
    fn golden_unit_snap_cases() {
        for case in group("unit_snap") {
            let got = unit_snap(quad(case, "bbox"));
            assert_eq!(got, quad(case, "expect"), "unit_snap {case}");
        }
    }

    #[test]
    fn angle_snap_of_a_stationary_drag_is_the_point_itself() {
        assert_eq!(angle_snap_deg(10, 10, 10, 10), [10.0, 10.0]);
    }

    #[test]
    fn zero_extent_drag_is_one_pixel() {
        assert_eq!(drag_bbox((7, 7), (7, 7), false, false), [7, 7, 1, 1]);
    }

    #[test]
    fn locked_drag_is_a_square() {
        let b = drag_bbox((0, 0), (4, 9), true, false);
        assert_eq!(b[2], b[3]);
        assert_eq!(b[2], 10);
    }

    #[test]
    fn centered_drag_grows_symmetrically() {
        // w = 2*|dx| + 1, so the origin sits |dx| back from p0.
        assert_eq!(drag_bbox((10, 10), (14, 12), false, true), [6, 8, 9, 5]);
    }

    #[test]
    fn unit_snap_never_shrinks_below_one_unit() {
        assert_eq!(unit_snap([0, 0, 1, 1]), [0, 0, 32, 32]);
        // Exactly on-grid boxes are left alone.
        assert_eq!(unit_snap([64, 96, 128, 160]), [64, 96, 128, 160]);
    }

    #[test]
    fn resize_handles_are_parsed_like_the_js_strings() {
        let b = [10, 10, 20, 10];
        // "e" moves the right edge; the opposite corner stays put.
        assert_eq!(
            resize_bbox(b, "e", (40, 14), false, false, 1),
            [10, 10, 31, 10]
        );
        // "nw" moves the corner; the opposite corner stays put.
        assert_eq!(
            resize_bbox(b, "nw", (3, 2), false, false, 1),
            [3, 2, 27, 18]
        );
        // A handle naming no edge is a no-op, exactly like the JS `includes` test.
        // ("move" would match 'e' — the JS test had the same sharp edge.)
        assert_eq!(resize_bbox(b, "q", (40, 40), false, false, 1), b);
    }

    #[test]
    fn resize_can_invert_a_box_past_its_anchor() {
        // Dragging the east handle west of the origin gives a narrow box; the
        // JS reference does the same and lets the tools clamp afterwards.
        let b = resize_bbox([10, 10, 20, 10], "e", (5, 14), false, false, 1);
        assert_eq!(b[2], 6);
    }
}
