//! Handle layout and grab tests.

use pixel_engine::raster::shape_raster::ShapeKind;

use super::types::{Bbox, HANDLE_IDS, Handle, PendingGeom, Pt};

fn dist_seg(px: f64, py: f64, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 as f64 - a.0 as f64, b.1 as f64 - a.1 as f64);
    let len2 = dx * dx + dy * dy;
    let mut t = if len2 == 0.0 {
        0.0
    } else {
        ((px - a.0 as f64) * dx + (py - a.1 as f64) * dy) / len2
    };
    t = t.clamp(0.0, 1.0);
    ((px - (a.0 as f64 + t * dx)).powi(2) + (py - (a.1 as f64 + t * dy)).powi(2)).sqrt()
}

/// Handle layout: line endpoints, polygon vertices, box corners/edges.
pub fn layout_handles(kind: ShapeKind, geom: &PendingGeom) -> Vec<Handle> {
    match kind {
        ShapeKind::Line => {
            let (p0, p1) = (geom.p0.unwrap_or((0, 0)), geom.p1.unwrap_or((0, 0)));
            vec![
                Handle {
                    id: "p0".to_string(),
                    end: Some(0),
                    x: p0.0 as f64,
                    y: p0.1 as f64,
                },
                Handle {
                    id: "p1".to_string(),
                    end: Some(1),
                    x: p1.0 as f64,
                    y: p1.1 as f64,
                },
            ]
        }
        ShapeKind::Polygon => geom
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| Handle {
                id: format!("v{i}"),
                end: None,
                x: p.0 as f64,
                y: p.1 as f64,
            })
            .collect(),
        _ => {
            let b = geom.bbox.unwrap_or(Bbox {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
            });
            let x1 = b.x + b.w - 1;
            let y1 = b.y + b.h - 1;
            let xm = (b.x + x1) as f64 / 2.0;
            let ym = (b.y + y1) as f64 / 2.0;
            let pts: [(&str, f64, f64); 8] = [
                ("nw", b.x as f64, b.y as f64),
                ("n", xm, b.y as f64),
                ("ne", x1 as f64, b.y as f64),
                ("e", x1 as f64, ym),
                ("se", x1 as f64, y1 as f64),
                ("s", xm, y1 as f64),
                ("sw", b.x as f64, y1 as f64),
                ("w", b.x as f64, ym),
            ];
            HANDLE_IDS
                .iter()
                .map(|id| {
                    let (_, x, y) = pts
                        .iter()
                        .find(|(hid, _, _)| hid == id)
                        .expect("handle table covers HANDLE_IDS");
                    Handle {
                        id: id.to_string(),
                        end: None,
                        x: *x,
                        y: *y,
                    }
                })
                .collect()
        }
    }
}

/// First handle within `tol` canvas px of the float point, like the JS.
pub fn hit_handle(
    kind: ShapeKind,
    geom: &PendingGeom,
    fx: f64,
    fy: f64,
    tol: f64,
) -> Option<Handle> {
    layout_handles(kind, geom)
        .into_iter()
        .find(|h| (fx - h.x).abs() <= tol && (fy - h.y).abs() <= tol)
}

/// Even-odd point-in-polygon, float test point like the JS.
fn point_in_polygon(pts: &[Pt], fx: f64, fy: f64) -> bool {
    let mut inside = false;
    let mut j = pts.len().wrapping_sub(1);
    for i in 0..pts.len() {
        let (xi, yi) = (pts[i].0 as f64, pts[i].1 as f64);
        let (xj, yj) = (pts[j].0 as f64, pts[j].1 as f64);
        if (yi > fy) != (yj > fy) && fx < (xj - xi) * (fy - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Grab test: box containment, line distance, polygon fill-or-edge.
pub fn inside_shape(kind: ShapeKind, geom: &PendingGeom, fx: f64, fy: f64, grab: f64) -> bool {
    match kind {
        ShapeKind::Line => {
            let (p0, p1) = (geom.p0.unwrap_or((0, 0)), geom.p1.unwrap_or((0, 0)));
            dist_seg(fx, fy, p0, p1) <= grab
        }
        ShapeKind::Polygon => {
            if point_in_polygon(&geom.points, fx, fy) {
                return true;
            }
            for i in 0..geom.points.len() {
                let b = geom.points[(i + 1) % geom.points.len()];
                if dist_seg(fx, fy, geom.points[i], b) <= grab {
                    return true;
                }
            }
            false
        }
        _ => {
            let b = geom.bbox.unwrap_or(Bbox {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
            });
            fx >= b.x as f64
                && fx <= (b.x + b.w - 1) as f64
                && fy >= b.y as f64
                && fy <= (b.y + b.h - 1) as f64
        }
    }
}
