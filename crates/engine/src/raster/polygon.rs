//! PIL polygon scanline engine, ported from `src/core/raster/polygon.js`.
//!
//! This is a port of Pillow's `Draw.c` (`ImagingDrawPolygon`,
//! `ImagingDrawWideLine`, `ImagingDrawLine`), not a generic rasteriser. The
//! golden fixtures were baked from real PIL output, so every rounding decision
//! below matters:
//!
//! * **`f32`, not `f64`.** `dx` and every intersection coordinate are C `float`s.
//!   [`fround`] reproduces `Math.fround`; multiplying or adding in `f32`
//!   directly gives the same bit pattern because the f64 result of two f32
//!   operands is exact before rounding.
//! * **Three different roundings.** [`round_up`] and [`round_down`] are the
//!   scanline span bounds, [`round_c`] is C `roundf`, and [`py_round`] is
//!   Python's banker's rounding. They are not interchangeable.
//!
//! The n×n-square-along-a-Bresenham-line model that most people reach for is
//! wrong here: measured against Pillow 12.1.0 over 9 geometries × widths 2..7 it
//! is exact on 1 case, while the rotated-quad model below is exact on 54/54.

use crate::errors::{DrawToolError, EngineResult};
use crate::raster::mask::{Mask, assert_mask_size};

/// `Math.fround` — round an f64 to the nearest f32.
#[inline]
pub fn fround(v: f64) -> f32 {
    v as f32
}

/// `Math.fround(Math.fround((y - y0) * dx) + x0)` — the intersection abscissa,
/// spelled exactly as the JS wrote it.
///
/// The inner product is computed in `f32` because two f32 operands have an
/// exactly-representable f64 product, so an f32 multiply and an fround of the
/// exact product agree bit for bit. The same argument covers the outer `+ x0`,
/// which is why that explicit `fround` is load-bearing rather than noise.
#[inline]
pub fn inter_x(y: i32, e: &Edge) -> f32 {
    let inner = (y - e.y0) as f32 * e.dx;
    fround(inner as f64 + e.x0 as f64)
}

/// PIL's `roundUp`: half away from zero, toward +∞ for positives.
#[inline]
pub fn round_up(f: f32) -> i32 {
    if f >= 0.0 {
        (f + 0.5).floor() as i32
    } else {
        -((-f + 0.5).floor() as i32)
    }
}

/// PIL's `roundDown`.
#[inline]
pub fn round_down(f: f32) -> i32 {
    if f >= 0.0 {
        (f - 0.5).ceil() as i32
    } else {
        -((-f - 0.5).ceil() as i32)
    }
}

/// C `roundf`: half away from zero.
#[inline]
pub fn round_c(f: f32) -> i32 {
    if f >= 0.0 {
        (f + 0.5).floor() as i32
    } else {
        -((-f + 0.5).floor() as i32)
    }
}

/// Python `round()`: half to even.
#[inline]
pub fn py_round(v: f64) -> i64 {
    let f = v.floor();
    let r = v - f;
    if r > 0.5 {
        (f + 1.0) as i64
    } else if r < 0.5 {
        f as i64
    } else if (f as i64) % 2 == 0 {
        // Exactly .5 → Python rounds to even, which keeps `f`.
        f as i64
    } else {
        (f + 1.0) as i64
    }
}

/// One polygon edge. `dx` is the f32 slope `(x1-x0)/(y1-y0)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub xmin: i32,
    pub xmax: i32,
    pub ymin: i32,
    pub ymax: i32,
    pub d: i32,
    pub dx: f32,
    pub x0: i32,
    pub y0: i32,
}

/// `polyAddEdge` — fill the extent fields and the slope.
pub fn poly_add_edge(mut e: Edge, x0: i32, y0: i32, x1: i32, y1: i32) -> Edge {
    if x0 <= x1 {
        e.xmin = x0;
        e.xmax = x1;
    } else {
        e.xmin = x1;
        e.xmax = x0;
    }
    if y0 <= y1 {
        e.ymin = y0;
        e.ymax = y1;
    } else {
        e.ymin = y1;
        e.ymax = y0;
    }
    if y0 == y1 {
        e.d = 0;
        e.dx = 0.0;
    } else {
        // fround of the f64 quotient, matching JS exactly.
        e.dx = fround((x1 - x0) as f64 / (y1 - y0) as f64);
        e.d = if y0 == e.ymin { 1 } else { -1 };
    }
    e.x0 = x0;
    e.y0 = y0;
    e
}

fn empty_edge() -> Edge {
    Edge {
        xmin: 0,
        xmax: 0,
        ymin: 0,
        ymax: 0,
        d: 0,
        dx: 0.0,
        x0: 0,
        y0: 0,
    }
}

/// Edge build with the consecutive-horizontal-merge rule from `ImagingDrawPolygon`.
///
/// Runs of same-`y` vertices collapse into one wider horizontal edge, which is
/// what keeps a staircase top edge from being filled twice.
pub fn poly_build_edges(pts: &[[i32; 2]]) -> Vec<Edge> {
    let mut e: Vec<Edge> = Vec::new();
    for i in 0..pts.len().saturating_sub(1) {
        let [x0, y0] = pts[i];
        let [x1, y1] = pts[i + 1];
        if y0 == y1 && i != 0 && y0 == pts[i - 1][1] {
            let last = e.last_mut().expect("edge exists for a run");
            if x1 > x0 && x0 > pts[i - 1][0] {
                last.xmax = x1;
                continue;
            }
            if x1 < x0 && x0 < pts[i - 1][0] {
                last.xmin = x1;
                continue;
            }
        }
        e.push(poly_add_edge(empty_edge(), x0, y0, x1, y1));
    }
    if let (Some(last), Some(first)) = (pts.last(), pts.first())
        && (last[0] != first[0] || last[1] != first[1])
    {
        e.push(poly_add_edge(
            empty_edge(),
            last[0],
            last[1],
            first[0],
            first[1],
        ));
    }
    e
}

/// Clipped horizontal span write.
fn poly_hline(data: &mut [u8], w: usize, h: usize, mut x0: i32, y: i32, mut x1: i32, val: u8) {
    if y < 0 || y >= h as i32 {
        return;
    }
    if x0 < 0 {
        x0 = 0;
    } else if x0 >= w as i32 {
        return;
    }
    if x1 < 0 {
        return;
    } else if x1 >= w as i32 {
        x1 = w as i32 - 1;
    }
    if x0 <= x1 {
        let row = y as usize * w;
        for x in x0..=x1 {
            data[row + x as usize] = val;
        }
    }
}

/// `polygon_generic` — the scanline fill, ported line by line.
///
/// The vertex-degeneracy adjustment (the block that nudges `xx[j-1]` by ±1 when
/// two edges meet exactly on a scanline) is the part most easily lost in a
/// rewrite; it is what makes a shared vertex not open a gap.
pub fn polygon_generic(data: &mut [u8], w: usize, h: usize, edges: &[Edge]) {
    let mut table: Vec<Edge> = Vec::new();
    let mut ymin = h as i32 - 1;
    let mut ymax = 0;
    for e in edges {
        if e.ymin == e.ymax {
            poly_hline(data, w, h, e.xmin, e.ymin, e.xmax, 1);
        } else {
            table.push(*e);
        }
    }
    for e in edges {
        if ymin > e.ymin {
            ymin = e.ymin;
        }
        if ymax < e.ymax {
            ymax = e.ymax;
        }
    }
    if ymin < 0 {
        ymin = 0;
    }
    if ymax > h as i32 {
        ymax = h as i32;
    }
    let mut xx = vec![0f32; table.len() * 2];
    for y in ymin..=ymax {
        let mut j = 0usize;
        for i in 0..table.len() {
            let cur = table[i];
            if y >= cur.ymin && y <= cur.ymax {
                // fround(fround(fround(y - y0) * dx) + x0)
                let inter = inter_x(y, &cur);
                xx[j] = inter;
                j += 1;
                if y == cur.ymax && y < ymax {
                    xx[j] = xx[j - 1];
                    j += 1;
                } else if (y == cur.ymin || y == cur.ymax) && cur.dx != 0.0 {
                    for other in table.iter().take(i) {
                        if (y != other.ymin && y != other.ymax) || other.dx == 0.0 {
                            continue;
                        }
                        if round_c(xx[j - 1]) == round_c(inter_x(y, other)) {
                            let offset = if y == cur.ymax { -1 } else { 1 };
                            let adj = inter_x(y + offset, &cur);
                            if y + offset >= other.ymin && y + offset <= other.ymax {
                                let adj_o = inter_x(y + offset, other);
                                if xx[j - 1] > adj + 1.0 && xx[j - 1] > adj_o + 1.0 {
                                    xx[j - 1] = round_c(adj.max(adj_o)) as f32 + 1.0;
                                } else if xx[j - 1] < adj - 1.0 && xx[j - 1] < adj_o - 1.0 {
                                    xx[j - 1] = round_c(adj.min(adj_o)) as f32 - 1.0;
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
        let mut vals: Vec<f32> = xx[..j].to_vec();
        vals.sort_unstable_by(|a, b| a.total_cmp(b));
        let mut i = 1;
        while i < j {
            poly_hline(data, w, h, round_up(vals[i - 1]), y, round_down(vals[i]), 1);
            i += 2;
        }
    }
}

/// PIL `line8`: the 1px line path, excluding each segment's far endpoint.
///
/// Eight parameters because the signature mirrors Pillow's `ImagingDrawLine`
/// argument for argument — keeping the order identical is the port's review aid.
#[allow(clippy::too_many_arguments)]
pub fn line8(
    data: &mut [u8],
    w: usize,
    h: usize,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    val: u8,
) {
    let mut dx = x1 - x0;
    let xs;
    let mut dy = y1 - y0;
    let ys;
    if dx < 0 {
        dx = -dx;
        xs = -1;
    } else {
        xs = 1;
    }
    if dy < 0 {
        dy = -dy;
        ys = -1;
    } else {
        ys = 1;
    }
    let n;
    let plot = |data: &mut [u8], x: i32, y: i32| {
        if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
            data[y as usize * w + x as usize] = val;
        }
    };
    if dx == 0 {
        for _ in 0..dy {
            plot(data, x0, y0);
            y0 += ys;
        }
    } else if dy == 0 {
        for _ in 0..dx {
            plot(data, x0, y0);
            x0 += xs;
        }
    } else if dx > dy {
        n = dx;
        dy += dy;
        let mut e = dy - dx;
        dx += dx;
        for _ in 0..n {
            plot(data, x0, y0);
            if e >= 0 {
                y0 += ys;
                e -= dx;
            }
            e += dy;
            x0 += xs;
        }
    } else {
        n = dy;
        dx += dx;
        let mut e = dx - dy;
        dy += dy;
        for _ in 0..n {
            plot(data, x0, y0);
            if e >= 0 {
                x0 += xs;
                e -= dy;
            }
            e += dx;
            y0 += ys;
        }
    }
}

/// `ImagingDrawWideLine`'s rotated quad, as four edges.
///
/// The widening is perpendicular to the segment, so the quad is a rotated
/// rectangle — not an axis-aligned one.
pub fn wide_line_quad_edges(x0: i32, y0: i32, x1: i32, y1: i32, width: i32) -> Vec<Edge> {
    let dx = (x1 - x0) as f64;
    let dy = (y1 - y0) as f64;
    let big = dx.hypot(dy);
    let small = (width - 1) as f64 / 2.0;
    let ratio_max = round_up(small as f32) as f64 / big;
    let ratio_min = round_down(small as f32) as f64 / big;
    let dxmin = round_down((ratio_min * dy) as f32);
    let dxmax = round_down((ratio_max * dy) as f32);
    let dymin = round_down((ratio_min * dx) as f32);
    let dymax = round_down((ratio_max * dx) as f32);
    let v = [
        [x0 - dxmin, y0 + dymax],
        [x1 - dxmin, y1 + dymax],
        [x1 + dxmax, y1 - dymin],
        [x0 + dxmax, y0 - dymin],
    ];
    let mut e = Vec::with_capacity(4);
    for i in 0..4 {
        let a = v[i];
        let b = v[(i + 1) % 4];
        e.push(poly_add_edge(empty_edge(), a[0], a[1], b[0], b[1]));
    }
    e
}

/// Integer bbox of the points, inclusive.
pub fn polygon_bbox(pts: &[[i32; 2]]) -> EngineResult<[i32; 4]> {
    if pts.is_empty() {
        return Err(DrawToolError::invalid_state(
            "polygonBBox needs a non-empty points array",
        ));
    }
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for [x, y] in pts {
        if *x < min_x {
            min_x = *x;
        }
        if *y < min_y {
            min_y = *y;
        }
        if *x > max_x {
            max_x = *x;
        }
        if *y > max_y {
            max_y = *y;
        }
    }
    Ok([min_x, min_y, max_x - min_x + 1, max_y - min_y + 1])
}

/// Polygon fill mask over a `w × h` canvas, matching the PIL oracle.
///
/// Fill-only ground truth: the legacy bakers call `d.polygon` with
/// `outline=None`, so the scanline fill keeps its boundary ring.
pub fn polygon_mask(pts: &[[i32; 2]], w: usize, h: usize) -> EngineResult<Mask> {
    assert_mask_size(w, h)?;
    let mut m = Mask::new(w, h);
    polygon_generic(&mut m.data, w, h, &poly_build_edges(pts));
    Ok(m)
}

/// Polygon outline mask on a square `w × w` canvas.
///
/// Models `d.line` over the closed path (centred stroke), which coincides with
/// the oracle's padded rendering on interior shapes — all golden cases.
pub fn polygon_outline_mask(pts: &[[i32; 2]], w: usize, width: i32) -> EngineResult<Mask> {
    assert_mask_size(w, w)?;
    let mut m = Mask::new(w, w);
    let mut path = pts.to_vec();
    if let Some(first) = pts.first() {
        path.push(*first);
    }
    for i in 0..path.len().saturating_sub(1) {
        let [x0, y0] = path[i];
        let [x1, y1] = path[i + 1];
        if width <= 1 {
            line8(&mut m.data, w, w, x0, y0, x1, y1, 1);
        } else {
            polygon_generic(
                &mut m.data,
                w,
                w,
                &wide_line_quad_edges(x0, y0, x1, y1, width),
            );
        }
    }
    Ok(m)
}

/// Inset outline ring for the polygon kind.
///
/// PIL paints `d.polygon` outlines strictly INSIDE the fill boundary, unlike
/// `d.line` which straddles its path. Modelled as the fill minus its Euclidean
/// erosion by the closed disk of radius `width` — **not** Manhattan: PIL offsets
/// wide lines perpendicular to the path, so a Manhattan erosion under-thins
/// 45-degree slants (measured on the `diamond_gem` hexagon with width 3:
/// Euclidean 0 missing / 0 extra, Manhattan 24 missing).
pub fn polygon_inset_ring(fill: &Mask, width: i32) -> Mask {
    let (w, h) = (fill.w, fill.h);
    let r = width.max(0);
    let mut offsets: Vec<(i32, i32)> = Vec::new();
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                offsets.push((dx, dy));
            }
        }
    }
    let is_set = |x: i32, y: i32| {
        x >= 0 && y >= 0 && x < w as i32 && y < h as i32 && fill.is_set(x as usize, y as usize)
    };
    let mut out = Mask::new(w, h);
    for y in 0..h {
        for x in 0..w {
            if !fill.is_set(x, y) {
                continue;
            }
            let mut eroded = true;
            for &(dx, dy) in &offsets {
                if !is_set(x as i32 + dx, y as i32 + dy) {
                    eroded = false;
                    break;
                }
            }
            out.set(x, y, (!eroded) as u8);
        }
    }
    out
}

/// Fillet polygon corners — exact port of the legacy `geometry.py fillet_polygon`.
///
/// `steps` arc samples per corner; the default 8 is what the fixtures used.
pub fn fillet_polygon(pts: &[[i32; 2]], radius: f64, steps: usize) -> Vec<[i32; 2]> {
    if radius <= 0.0 || pts.len() < 3 {
        return pts.to_vec();
    }
    // `fillet_polygon` works in plain `[x, y]` pairs.
    let local: Vec<[i32; 2]> = pts.to_vec();
    let n = local.len();
    let mut new_pts: Vec<[i32; 2]> = Vec::new();
    for i in 0..n {
        let p_prev = local[(i + n - 1) % n];
        let p_curr = local[i];
        let p_next = local[(i + 1) % n];
        let v1x = (p_prev[0] - p_curr[0]) as f64;
        let v1y = (p_prev[1] - p_curr[1]) as f64;
        let v2x = (p_next[0] - p_curr[0]) as f64;
        let v2y = (p_next[1] - p_curr[1]) as f64;
        let l1 = v1x.hypot(v1y);
        let l2 = v2x.hypot(v2y);
        if l1 == 0.0 || l2 == 0.0 {
            new_pts.push(p_curr);
            continue;
        }
        let (u1x, u1y) = (v1x / l1, v1y / l1);
        let (u2x, u2y) = (v2x / l2, v2y / l2);
        let dot = (u1x * u2x + u1y * u2y).clamp(-0.999, 0.999);
        let half_angle = dot.acos() / 2.0;
        let tan_half = half_angle.tan();
        if tan_half < 0.001 {
            new_pts.push(p_curr);
            continue;
        }
        let max_r = radius.min(l1 * 0.45 * tan_half).min(l2 * 0.45 * tan_half);
        let tangent_len = max_r / tan_half;
        let p_start_x = p_curr[0] as f64 + u1x * tangent_len;
        let p_start_y = p_curr[1] as f64 + u1y * tangent_len;
        let p_end_x = p_curr[0] as f64 + u2x * tangent_len;
        let p_end_y = p_curr[1] as f64 + u2y * tangent_len;
        let bis_x = u1x + u2x;
        let bis_y = u1y + u2y;
        let b_len = bis_x.hypot(bis_y);
        if b_len < 0.001 {
            new_pts.push(p_curr);
            continue;
        }
        let (w_x, w_y) = (bis_x / b_len, bis_y / b_len);
        let center_dist = max_r / half_angle.sin();
        let cx = p_curr[0] as f64 + w_x * center_dist;
        let cy = p_curr[1] as f64 + w_y * center_dist;

        let a_start = (p_start_y - cy).atan2(p_start_x - cx);
        let a_end = (p_end_y - cy).atan2(p_end_x - cx);

        let diff = ((a_end - a_start + std::f64::consts::PI + 2.0 * std::f64::consts::PI)
            % (2.0 * std::f64::consts::PI))
            - std::f64::consts::PI;
        for s in 0..=steps {
            let ang = a_start + diff * (s as f64 / steps as f64);
            new_pts.push([
                py_round(cx + max_r * ang.cos()) as i32,
                py_round(cy + max_r * ang.sin()) as i32,
            ]);
        }
    }
    new_pts
}

/// Accepts `[x, y]` or `{x, y}` fixtures and returns integer pairs.
pub fn points_from_json(value: &serde_json::Value) -> Vec<[i32; 2]> {
    let arr = value["pts"]
        .as_array()
        .or_else(|| value["points"].as_array())
        .expect("a `pts` or `points` array");
    arr.iter()
        .map(|p| {
            if let Some(a) = p.as_array() {
                [a[0].as_i64().unwrap() as i32, a[1].as_i64().unwrap() as i32]
            } else {
                [
                    p["x"].as_i64().unwrap() as i32,
                    p["y"].as_i64().unwrap() as i32,
                ]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{group, i64_field, rows_to_mask, usize_field};

    // <META - ROLE : golden polygon group, 6 cases | L300-320>
    #[test]
    fn golden_polygon_cases() {
        for case in group("polygon") {
            let pts = points_from_json(case);
            let w = usize_field(case, "w");
            let h = usize_field(case, "h");
            let got = polygon_mask(&pts, w, h).expect("valid size");
            let want = rows_to_mask(case);
            assert_eq!(
                got.rows(),
                crate::fixtures::mask_to_rows(&want),
                "polygon {:?}",
                case["name"]
            );
        }
    }

    #[test]
    fn golden_polygon_outline_cases() {
        for case in group("polygon_outline") {
            let pts = points_from_json(case);
            let w = usize_field(case, "w");
            let width = i64_field(case, "width") as i32;
            let got = polygon_outline_mask(&pts, w, width).expect("valid size");
            let want = rows_to_mask(case);
            assert_eq!(
                got.rows(),
                crate::fixtures::mask_to_rows(&want),
                "polygon_outline w={width}"
            );
        }
    }

    #[test]
    fn golden_fillet_cases() {
        for case in group("fillet") {
            let pts = points_from_json(case);
            let radius = crate::fixtures::f64_field(case, "radius");
            let w = usize_field(case, "w");
            let h = usize_field(case, "h");
            let filleted = fillet_polygon(&pts, radius, 8);
            let got = polygon_mask(&filleted, w, h).expect("valid size");
            let want = rows_to_mask(case);
            assert_eq!(
                got.rows(),
                crate::fixtures::mask_to_rows(&want),
                "fillet r={radius}"
            );
        }
    }

    #[test]
    fn roundings_are_not_interchangeable() {
        assert_eq!(round_up(2.5), 3);
        assert_eq!(round_up(-2.5), -3);
        assert_eq!(round_down(2.5), 2);
        assert_eq!(round_down(-2.5), -2);
        assert_eq!(round_c(2.5), 3);
        assert_eq!(round_c(-2.5), -3);
        // Python: half to even.
        assert_eq!(py_round(2.5), 2);
        assert_eq!(py_round(3.5), 4);
        assert_eq!(py_round(-2.5), -2);
    }

    #[test]
    fn wide_line_quad_is_rotated_not_axis_aligned() {
        // A 45-degree segment must produce a diamond-ish quad, i.e. the
        // perpendicular offsets must show up in both axes.
        let edges = wide_line_quad_edges(0, 0, 10, 10, 4);
        assert_eq!(edges.len(), 4);
        let xs: Vec<i32> = edges.iter().map(|e| e.xmin).collect();
        let ys: Vec<i32> = edges.iter().map(|e| e.ymin).collect();
        assert!(xs.iter().any(|&x| x != 0), "the quad leans in x: {xs:?}");
        assert!(ys.iter().any(|&y| y != 0), "the quad leans in y: {ys:?}");
    }

    #[test]
    fn polygon_mask_keeps_its_boundary_ring() {
        let pts = [[2, 2], [8, 2], [8, 8], [2, 8]];
        let m = polygon_mask(&pts, 12, 12).expect("valid size");
        assert!(m.is_set(2, 2), "the corner pixel belongs to the fill");
        assert_eq!(m.data.iter().filter(|&&v| v == 1).count(), 49);
    }

    #[test]
    fn empty_points_raise_the_contract_code() {
        match polygon_bbox(&[]) {
            Err(e) => assert_eq!(e.code, crate::errors::CODE_INVALID_STATE),
            Ok(_) => panic!("empty points must be rejected"),
        }
    }
}
