//! Drag, resize and move over pending geometry.

use pixel_engine::blend::rnd;
use pixel_engine::raster::shape_raster::ShapeKind;
use pixel_engine::raster::snap::{angle_snap_deg, drag_bbox, resize_bbox, unit_snap};

use super::types::{Bbox, DragFlags, PendingGeom, Pt};

fn snap_unit_i(v: i64) -> i64 {
    // `rnd(v/32)*32` — `rnd` is f64→f64, the truncation the JS relied on.
    (rnd(v as f64 / 32.0) * 32.0) as i64
}

/// Fresh-draw geometry from a drag vector.
pub fn drag_geom(
    kind: ShapeKind,
    p0: Pt,
    p1: Pt,
    flags: DragFlags,
    shape_radius: i32,
) -> PendingGeom {
    if kind == ShapeKind::Line {
        let (x, y) = if flags.shift {
            let s = angle_snap_deg(p0.0, p0.1, p1.0, p1.1);
            (s[0] as i32, s[1] as i32)
        } else {
            p1
        };
        return PendingGeom {
            bbox: None,
            p0: Some(p0),
            p1: Some((x, y)),
            radius: 0,
            points: vec![],
        };
    }
    let mut b = drag_bbox(
        (p0.0 as i64, p0.1 as i64),
        (p1.0 as i64, p1.1 as i64),
        flags.lock,
        flags.center,
    );
    if flags.snap {
        b = unit_snap(b);
    }
    PendingGeom {
        bbox: Some(Bbox {
            x: b[0] as i32,
            y: b[1] as i32,
            w: b[2] as i32,
            h: b[3] as i32,
        }),
        p0: None,
        p1: None,
        radius: shape_radius,
        points: vec![],
    }
}

/// Pending resize from a handle drag. `shift_snap` is the up-event's shift.
pub fn resize_geom(
    kind: ShapeKind,
    orig: &PendingGeom,
    handle_id: &str,
    end: Option<u8>,
    p: Pt,
    flags: DragFlags,
    shift_snap: bool,
) -> PendingGeom {
    if kind == ShapeKind::Line {
        let fixed = if end == Some(0) { orig.p1 } else { orig.p0 }.unwrap_or((0, 0));
        let (x, y) = if shift_snap {
            let s = angle_snap_deg(fixed.0, fixed.1, p.0, p.1);
            (s[0] as i32, s[1] as i32)
        } else {
            p
        };
        let mut next = orig.clone();
        if end == Some(0) {
            next.p0 = Some((x, y));
        } else {
            next.p1 = Some((x, y));
        }
        return next;
    }
    if kind == ShapeKind::Polygon {
        // Bad ids keep `orig` — the JS `isInteger && in range` guard.
        if handle_id.len() < 2 || !handle_id.starts_with('v') {
            return orig.clone();
        }
        let idx: usize = handle_id[1..].parse().unwrap_or(usize::MAX);
        if idx >= orig.points.len() {
            return orig.clone();
        }
        let np = if flags.snap {
            (
                snap_unit_i(p.0 as i64) as i32,
                snap_unit_i(p.1 as i64) as i32,
            )
        } else {
            p
        };
        let mut next = orig.clone();
        next.points[idx] = np;
        return next;
    }
    let b = orig.bbox.unwrap_or(Bbox {
        x: 0,
        y: 0,
        w: 1,
        h: 1,
    });
    let mut r = resize_bbox(
        [b.x as i64, b.y as i64, b.w as i64, b.h as i64],
        handle_id,
        (p.0 as i64, p.1 as i64),
        flags.lock,
        flags.center,
        1,
    );
    if flags.snap {
        r = unit_snap(r);
    }
    let mut next = orig.clone();
    next.bbox = Some(Bbox {
        x: r[0] as i32,
        y: r[1] as i32,
        w: r[2] as i32,
        h: r[3] as i32,
    });
    next
}

/// Pending move by a delta, with optional unit snap.
pub fn move_geom(kind: ShapeKind, orig: &PendingGeom, dx: i32, dy: i32, snap: bool) -> PendingGeom {
    let sh = |q: Pt| {
        if snap {
            (
                snap_unit_i(q.0 as i64 + dx as i64) as i32,
                snap_unit_i(q.1 as i64 + dy as i64) as i32,
            )
        } else {
            (q.0 + dx, q.1 + dy)
        }
    };
    let mut next = orig.clone();
    match kind {
        ShapeKind::Line => {
            next.p0 = orig.p0.map(sh);
            next.p1 = orig.p1.map(sh);
        }
        ShapeKind::Polygon => {
            next.points = orig.points.iter().copied().map(sh).collect();
        }
        _ => {
            if let Some(b) = orig.bbox {
                next.bbox = Some(Bbox {
                    x: sh((b.x, 0)).0,
                    y: sh((0, b.y)).1,
                    w: b.w,
                    h: b.h,
                });
            }
        }
    }
    next
}

/// One placement point snapped to the 32px grid when snapping.
pub fn snap_place_point(p: Pt, snap: bool) -> Pt {
    if !snap {
        return p;
    }
    (
        snap_unit_i(p.0 as i64) as i32,
        snap_unit_i(p.1 as i64) as i32,
    )
}
