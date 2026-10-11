use super::angle::{dot_rect, round_half_up};
use super::*;
use crate::fixtures::{group, rows_to_mask, usize_field};
use crate::raster::mask::Mask;

fn box_of(case: &serde_json::Value) -> Box2 {
    let b = case["box"].as_array().expect("box");
    let a = [b[0][0].as_f64().unwrap(), b[0][1].as_f64().unwrap()];
    let c = [b[1][0].as_f64().unwrap(), b[1][1].as_f64().unwrap()];
    Box2::from_corners(a, c)
}

// <META - ROLE : golden arc group, 2 cases with the pinned tolerance | L230-250>
#[test]
fn golden_arc_cases_within_tolerance() {
    for case in group("arc") {
        let b = box_of(case);
        let start = crate::fixtures::f64_field(case, "start");
        let end = crate::fixtures::f64_field(case, "end");
        let width = case["width"].as_i64().unwrap() as i32;
        let w = usize_field(case, "w");
        let h = usize_field(case, "h");
        let got = arc_mask(b, start, end, width, w, h).expect("valid size");
        let want = rows_to_mask(case);
        let (missing, extra) = near_parity::diff(&want, &grid_of(&got));
        assert_eq!(
            missing, 0,
            "arc {} must cover every PIL pixel",
            case["name"]
        );
        assert!(
            extra <= near_parity::ARC_MAX_EXTRA,
            "arc {} overshoot {} > {}",
            case["name"],
            extra,
            near_parity::ARC_MAX_EXTRA
        );
    }
}

#[test]
fn golden_chord_fill_within_tolerance() {
    for case in group("chord") {
        if case.get("width").is_some() {
            continue; // the outline case, below
        }
        let b = box_of(case);
        let start = crate::fixtures::f64_field(case, "start");
        let end = crate::fixtures::f64_field(case, "end");
        let w = usize_field(case, "w");
        let h = usize_field(case, "h");
        let got = chord_fill_mask(b, start, end, w, h).expect("valid size");
        let want = rows_to_mask(case);
        let (missing, extra) = near_parity::diff(&want, &grid_of(&got));
        assert_eq!(
            missing, 0,
            "chord fill {} drops {} PIL pixels",
            case["name"], missing
        );
        assert!(
            extra <= near_parity::CHORD_FILL_MAX_EXTRA,
            "chord fill {} overshoot {} > {}",
            case["name"],
            extra,
            near_parity::CHORD_FILL_MAX_EXTRA
        );
    }
}

#[test]
fn golden_chord_outline_within_tolerance() {
    for case in group("chord") {
        let Some(width) = case.get("width").and_then(|v| v.as_i64()) else {
            continue; // the fill case, above
        };
        let b = box_of(case);
        let start = crate::fixtures::f64_field(case, "start");
        let end = crate::fixtures::f64_field(case, "end");
        let w = usize_field(case, "w");
        let h = usize_field(case, "h");
        let got = chord_outline_mask(b, start, end, width as i32, w, h).expect("valid size");
        let want = rows_to_mask(case);
        let (missing, extra) = near_parity::diff(&want, &grid_of(&got));
        assert!(
            missing <= near_parity::CHORD_OUTLINE_MAX_MISSING,
            "chord outline {} drops {} > 16",
            case["name"],
            missing
        );
        assert!(
            extra <= near_parity::CHORD_OUTLINE_MAX_EXTRA,
            "chord outline {} overshoot {} > 40",
            case["name"],
            extra
        );
    }
}

fn grid_of(m: &Mask) -> Vec<Vec<bool>> {
    m.data
        .chunks(m.w)
        .map(|row| row.iter().map(|&v| v == 1).collect())
        .collect()
}

#[test]
fn round_half_up_matches_js_math_round() {
    // JS Math.round rounds toward +Infinity on negatives, f64::round does not.
    assert_eq!(round_half_up(2.5), 3);
    assert_eq!(round_half_up(-2.5), -2);
    assert_eq!(round_half_up(-0.5), 0);
    assert_eq!(round_half_up(0.5), 1);
    assert_eq!(round_half_up(-2.4), -2);
}

#[test]
fn end_angle_normalises_forward() {
    // start 300 -> end 60 must sweep the long way through 0/360.
    let b = Box2::from_corners([0.0, 0.0], [10.0, 10.0]);
    let pts = arc_ellipse_points(b, 300.0, 60.0, 0.5);
    // 120 degrees / 0.5 = 240 steps.
    assert_eq!(pts.len(), 241);
}

#[test]
fn degenerate_segment_paints_a_dot() {
    let mut data = vec![0u8; 25];
    // A single sample point renders as a 2x2 dot for width 2.
    dot_rect(&mut data, 5, 5, 2, 2, 2);
    assert_eq!(data.iter().filter(|&&v| v == 1).count(), 4);
    let rows = grid_of_unwrap(&data, 5, 5);
    let rows_with_any = rows.iter().filter(|r| r.iter().any(|&v| v)).count();
    assert_eq!(rows_with_any, 2);
}

fn grid_of_unwrap(data: &[u8], w: usize, _h: usize) -> Vec<Vec<bool>> {
    data.chunks(w)
        .map(|r| r.iter().map(|&v| v == 1).collect())
        .collect()
}

#[test]
fn box_corners_become_centre_and_radii() {
    let b = Box2::from_corners([1.0, 1.0], [15.0, 9.0]);
    assert_eq!((b.cx, b.cy), (8.0, 5.0));
    assert_eq!((b.rx, b.ry), (7.0, 4.0));
}

#[test]
fn mask_matches_the_fixture_row_count() {
    for case in group("arc") {
        let m = arc_mask(
            box_of(case),
            crate::fixtures::f64_field(case, "start"),
            crate::fixtures::f64_field(case, "end"),
            case["width"].as_i64().unwrap() as i32,
            usize_field(case, "w"),
            usize_field(case, "h"),
        )
        .expect("valid size");
        assert_eq!(m.rows().len(), rows_to_mask(case).len());
    }
}
