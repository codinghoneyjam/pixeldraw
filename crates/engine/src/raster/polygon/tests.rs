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
