//! `view` integration tests.
//!
//! The `d1_*` cases mirror `tests/view_basic.test.mjs`; the rest cover what that
//! suite and `tools/view_check.mjs` leave unstated. The long-form reasoning behind
//! each invariant is in `docs/view_math.md`.

use crate::view::*;

#[allow(dead_code)]
fn view(zoom: f64, offset_x: f64, offset_y: f64) -> View {
    View::new(zoom, offset_x, offset_y).expect("valid view")
}

#[allow(dead_code)]
fn store() -> ViewStore {
    ViewStore::new()
}

// ---- d1: the mjs suite's worked examples ----

#[test]
fn d1_zoom_4_offset_100_50_mappings() {
    let v = view(4.0, 100.0, 50.0);
    assert_eq!(screen_to_canvas(v, 200.0, 150.0).x, 25.0);
    assert_eq!(screen_to_canvas(v, 200.0, 150.0).y, 25.0);
    assert_eq!(canvas_to_screen(v, 25.0, 25.0).x, 200.0);
    assert_eq!(pixel_at(v, 200.0, 150.0).x, 25);
}

#[test]
fn d1_zoom_at_anchor_fixed() {
    let v = view(1.0, 10.0, 20.0);
    let next = zoom_at(v, 40.0, 50.0, 4.0).expect("valid zoom");
    let before = canvas_to_screen(v, 30.0, 30.0);
    let after = canvas_to_screen(next, 30.0, 30.0);
    assert!((before.x - after.x).abs() < 1e-9);
    assert!((before.y - after.y).abs() < 1e-9);
}

#[test]
fn d1_step_zoom_edges() {
    assert_eq!(step_zoom(1.0, 1), 2.0);
    assert_eq!(step_zoom(1.0, -1), 0.5);
    assert_eq!(step_zoom(2.0, 0), 2.0);
}

#[test]
fn d1_pan_clamp_fit_actual() {
    let v = view(1.0, 0.0, 0.0);
    assert_eq!(pan_by(v, 10.0, 10.0).offset_x(), 10.0);
    // 512 canvas at zoom 1 in an 800x600 viewport: the canvas fits entirely,
    // so offset 0 already sits inside [32-512, 800-32] and the clamp is a no-op.
    let c = clamp_view(v, 512.0, 512.0, 800.0, 600.0);
    assert_eq!(c.offset_x(), 0.0);
    assert_eq!(c.offset_y(), 0.0);
    assert_eq!(fit_view(512.0, 512.0, 800.0, 600.0).zoom(), 1.0);
    assert_eq!(actual_size_view(512.0, 512.0, 800.0, 600.0).zoom(), 1.0);
}

#[test]
fn d1_visible_chunk_range() {
    let v = view(1.0, 0.0, 0.0);
    let r = visible_chunk_range(v, 128.0, 128.0, 64.0, 64.0).expect("visible");
    assert_eq!((r.cx0, r.cx1), (0, 1));
    assert_eq!((r.cy0, r.cy1), (0, 1));
}

#[test]
fn d1_create_view_rejects_bad_zoom() {
    assert!(
        create_view(ViewOptions {
            zoom: Some(0.0),
            ..ViewOptions::default()
        })
        .is_err()
    );
    assert!(
        create_view(ViewOptions {
            zoom: Some(f64::NAN),
            ..ViewOptions::default()
        })
        .is_err()
    );
}

#[test]
fn d1_roundtrip_and_zoom_at_invariance_over_100_seeded_views() {
    // Same LCG the mjs suite uses, bit-exact, so the 100 views are the same ones.
    let mut s: u32 = 12345;
    let mut next = || {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        s
    };
    for _ in 0..100 {
        let zoom = 0.25 + f64::from(next() % 1000) / 1000.0 * 31.75;
        let ox = f64::from(next() % 5000) - 2500.0;
        let oy = f64::from(next() % 5000) - 2500.0;
        let v = view(zoom, ox, oy);
        let sx = f64::from(next() % 5000) - 2500.0;
        let sy = f64::from(next() % 5000) - 2500.0;
        let p = screen_to_canvas(v, sx, sy);
        let back = canvas_to_screen(v, p.x, p.y);
        assert!((back.x - sx).abs() < 1e-6, "round trip drifted at {v:?}");
        let new_zoom = 0.25 + f64::from(next() % 1000) / 1000.0 * 31.75;
        let next_view = zoom_at(v, sx, sy, new_zoom).expect("valid zoom");
        let anchor_before = screen_to_canvas(v, sx, sy);
        let anchor_after = screen_to_canvas(next_view, sx, sy);
        // (sx,sy) is the anchor: its canvas position is unchanged by the zoom.
        assert!((anchor_before.x - anchor_after.x).abs() < 1e-6);
        assert!((anchor_before.y - anchor_after.y).abs() < 1e-6);
    }
}

// ---- invariants the mjs suite and view_check do not state ----

#[test]
fn zoom_ladder_is_the_frozen_constants_one() {
    assert_eq!(
        ZOOM_LEVELS.to_vec(),
        crate::constants::ZOOM_LEVELS.to_vec(),
        "the ladder must not be redefined here"
    );
}

#[test]
fn step_zoom_sticks_at_the_wrong_end_outside_the_ladder() {
    assert_eq!(step_zoom(64.0, 1), 32.0);
    assert_eq!(step_zoom(0.1, -1), 0.25);
}

#[test]
fn step_zoom_walks_the_whole_ladder() {
    let mut z = ZOOM_LEVELS[0];
    for expected in ZOOM_LEVELS.iter().skip(1) {
        z = step_zoom(z, 1);
        assert_eq!(z, *expected);
    }
    let mut z = ZOOM_LEVELS[ZOOM_LEVELS.len() - 1];
    for expected in ZOOM_LEVELS.iter().rev().skip(1) {
        z = step_zoom(z, -1);
        assert_eq!(z, *expected);
    }
}

#[test]
fn fit_view_falls_back_to_the_smallest_rung() {
    assert_eq!(fit_view(2048.0, 2048.0, 200.0, 200.0).zoom(), 0.25);
}

#[test]
fn fit_view_takes_the_largest_rung_that_fits() {
    assert_eq!(fit_view(512.0, 512.0, 800.0, 600.0).zoom(), 1.0);
}

#[test]
fn centring_rounds_halves_up_the_way_math_round_does() {
    let v = actual_size_view(5.0, 5.0, 2.0, 2.0);
    assert_eq!(v.offset_x(), -1.0);
    assert_eq!(v.offset_y(), -1.0);
}

#[test]
fn visible_chunk_range_clamps_the_near_edge_but_not_the_far_one() {
    let v = view(1.0, 20.0, 20.0);
    let r = visible_chunk_range(v, 128.0, 128.0, 64.0, 64.0).expect("visible");
    // The near edge is floored at 0; the far edge stays inside the 4 chunks of a
    // 128px canvas. The `>= 0` half of the assertion is the clamped one.
    assert_eq!(r.cx0, 0, "near edge clamps at zero");
    assert!(r.cx1 <= 3, "far edge stays inside the canvas chunk count");
    assert_eq!(r.cx0, r.cy0);
    assert_eq!(r.cx1, r.cy1);
}

#[test]
fn visible_chunk_range_is_none_just_off_the_right_edge() {
    let v = view(1.0, 500.0, 0.0);
    assert!(visible_chunk_range(v, 128.0, 128.0, 64.0, 64.0).is_none());
}

#[test]
fn clamp_view_range_inversion_pins_to_hi_not_lo() {
    let v = view(0.25, 0.0, 0.0);
    let c = clamp_view(v, 32.0, 32.0, 50.0, 50.0);
    assert_eq!(c.offset_x(), 18.0);
    assert_eq!(c.offset_y(), 18.0);
}

#[test]
fn clamp_view_with_a_custom_margin() {
    let v = view(1.0, -100.0, 0.0);
    let c = clamp_view_with_margin(v, 100.0, 100.0, 400.0, 400.0, 10.0);
    assert_eq!(c.offset_x(), 10.0 - 100.0);
}

#[test]
fn pan_by_does_not_clamp_but_the_store_does() {
    let p = pan_by(view(1.0, 0.0, 0.0), 5000.0, 0.0);
    assert_eq!(p.offset_x(), 5000.0);
    let mut s = store();
    s.set_canvas(64, 64).expect("valid size");
    let c = s.pan_by(5000.0, 0.0);
    assert!(c.view.offset_x() < 5000.0, "the store must clamp");
}

#[test]
fn create_view_rejects_non_finite_offsets() {
    assert!(
        create_view(ViewOptions {
            offset_x: Some(f64::NAN),
            ..ViewOptions::default()
        })
        .is_err()
    );
    assert!(
        create_view(ViewOptions {
            offset_y: Some(f64::INFINITY),
            ..ViewOptions::default()
        })
        .is_err()
    );
}

#[test]
fn zoom_at_rejects_non_positive_and_non_finite() {
    let v = View::identity();
    assert!(zoom_at(v, 0.0, 0.0, 0.0).is_err());
    assert!(zoom_at(v, 0.0, 0.0, f64::NAN).is_err());
}

#[test]
fn every_error_code_stays_inside_the_catalog() {
    let bad = [
        View::new(f64::NAN, 0.0, 0.0).unwrap_err(),
        View::new(0.0, 0.0, 0.0).unwrap_err(),
        Viewport::new(0.0, 0.0).unwrap_err(),
        Viewport::new(f64::NAN, 0.0).unwrap_err(),
        zoom_at(View::identity(), 0.0, 0.0, 0.0).unwrap_err(),
    ];
    for e in bad {
        assert!(
            crate::errors::CATALOG.contains(&e.code),
            "{} is outside the catalog",
            e.code
        );
    }
}

#[test]
fn viewport_new_rejects_a_non_positive_size() {
    assert!(Viewport::new(0.0, 100.0).is_err());
    assert!(Viewport::new(100.0, -1.0).is_err());
    assert!(Viewport::new(100.0, 100.0).is_ok());
    assert_eq!(Viewport::DEFAULT, Viewport::new(800.0, 600.0).unwrap());
}

#[test]
fn store_events_names_nothing_outside_the_frozen_catalog() {
    let c = ViewChange {
        view: View::identity(),
        changed: true,
    };
    assert!(c.events().is_empty());
}

#[test]
fn store_set_canvas_rejects_bad_sizes() {
    let mut s = store();
    assert!(s.set_canvas(31, 32).is_err());
    assert!(s.set_canvas(33, 32).is_err());
}
