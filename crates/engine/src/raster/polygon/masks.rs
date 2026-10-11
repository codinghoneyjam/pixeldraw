//! The public mask builders — fill, outline, inset ring.
//!
//! `line8` and the wide-line quad are the two PIL stroke regimes; the mask
//! builders on top of them are what the golden fixture groups exercise. The
//! rounding and f32 traps live in `scanline.rs`, the rationale in
//! `docs/raster_internals.md`.

use super::scanline::{
    Edge, empty_edge, poly_add_edge, poly_build_edges, polygon_generic, round_down, round_up,
};
use crate::errors::{DrawToolError, EngineResult};
use crate::raster::mask::{Mask, assert_mask_size};

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
