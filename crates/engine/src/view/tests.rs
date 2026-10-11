//! Tests for [`super`].
//!
//! The `d1_*` group is `tests/view_basic.test.mjs`'s first describe block
//! (`D1 view worked examples`) ported case for case, including its 100-view
//! LCG-swept round-trip. Everything after it is an invariant from
//! `tools/view_check.mjs` / `reference/view_ref.mjs` that the mjs suite does
//! not already cover.

use super::*;
use crate::constants::{CHUNK_PX, ZOOM_LEVELS};
use crate::errors::CATALOG;

/// `createView(...).unwrap()` — the constructor cannot fail for these values,
/// and tests are allowed to say so.
fn view(zoom: f64, offset_x: f64, offset_y: f64) -> View {
    View::new(zoom, offset_x, offset_y).expect("valid view")
}

/// `pixelAt` against a literal, so the assertion reads like the mjs.
fn pixel(v: View, sx: f64, sy: f64) -> Pixel {
    pixel_at(v, sx, sy)
}

/// A store with the default 800×600 viewport and a 512×512 canvas attached.
fn store() -> ViewStore {
    let mut s = ViewStore::new();
    assert!(s.set_canvas(512, 512).is_ok());
    s
}

// --- D1: tests/view_basic.test.mjs, ported case for case ---------------------

#[test]
fn d1_zoom_4_offset_100_50_mappings() {
    let v = view(4.0, 100.0, 50.0);
    assert_eq!(
        screen_to_canvas(v, 140.0, 90.0),
        CanvasPoint { x: 10.0, y: 10.0 }
    );
    assert_eq!(pixel(v, 143.9, 93.9), Pixel { x: 10, y: 10 });
    // Left/above the canvas: negative, and never clamped (view_check L7).
    assert_eq!(pixel(v, 99.0, 49.0), Pixel { x: -1, y: -1 });
    assert_eq!(
        canvas_to_screen(v, 10.0, 10.0),
        ScreenPoint { x: 140.0, y: 90.0 }
    );
}

#[test]
fn d1_zoom_at_anchor_fixed() {
    // The mjs case is zoomAt(createView({ zoom: 2 }), 100, 100, 4): k = 2, so
    // the anchor sits at 100 and the origin lands at 100 - 100*2 = -100.
    assert_eq!(
        zoom_at(view(2.0, 0.0, 0.0), 100.0, 100.0, 4.0).ok(),
        Some(view(4.0, -100.0, -100.0))
    );
    let v = view(4.0, 100.0, 50.0);
    let z = zoom_at(v, 300.0, 200.0, 8.0).expect("8 is a valid zoom");
    assert_eq!(
        screen_to_canvas(z, 300.0, 200.0),
        screen_to_canvas(v, 300.0, 200.0)
    );
}

#[test]
fn d1_step_zoom_edges() {
    assert_eq!(step_zoom(1.0, 1), 2.0);
    assert_eq!(step_zoom(32.0, 1), 32.0);
    assert_eq!(step_zoom(0.25, -1), 0.25);
    assert_eq!(step_zoom(5.0, 1), 6.0);
    assert_eq!(step_zoom(5.0, -1), 4.0);
}

#[test]
fn d1_pan_clamp_fit_actual() {
    assert_eq!(pan_by(view(2.0, 0.0, 0.0), 5.0, -7.0), view(2.0, 5.0, -7.0));
    assert_eq!(
        clamp_view(view(1.0, 5000.0, -5000.0), 512.0, 512.0, 800.0, 600.0),
        view(1.0, 768.0, -480.0)
    );
    assert_eq!(fit_view(512.0, 512.0, 800.0, 600.0), view(1.0, 144.0, 44.0));
    assert_eq!(
        fit_view(1920.0, 1088.0, 1200.0, 700.0),
        view(0.5, 120.0, 78.0)
    );
    assert_eq!(
        actual_size_view(512.0, 512.0, 800.0, 600.0),
        view(1.0, 144.0, 44.0)
    );
}

#[test]
fn d1_visible_chunk_range() {
    assert_eq!(
        visible_chunk_range(View::identity(), 512.0, 512.0, 800.0, 600.0),
        Some(VisibleChunkRange {
            cx0: 0,
            cy0: 0,
            cx1: 15,
            cy1: 15,
        })
    );
    assert_eq!(
        visible_chunk_range(view(4.0, -64.0, 0.0), 512.0, 512.0, 256.0, 256.0),
        Some(VisibleChunkRange {
            cx0: 0,
            cy0: 0,
            cx1: 2,
            cy1: 1,
        })
    );
    assert_eq!(
        visible_chunk_range(view(1.0, 900.0, 0.0), 512.0, 512.0, 800.0, 600.0),
        None
    );
}

#[test]
fn d1_create_view_rejects_bad_zoom() {
    assert!(
        create_view(ViewOptions {
            zoom: Some(0.0),
            offset_x: None,
            offset_y: None,
        })
        .is_err()
    );
    assert!(zoom_at(View::identity(), 0.0, 0.0, -2.0).is_err());
}

/// The mjs `roundtrip + zoomAt invariance over 100 seeded views` case.
///
/// The LCG is ported bit-exactly (`s * 1664525 + 1013904223 >>> 0`), so the
/// 100 views, the probe points and the new zooms are the same sequence the JS
/// generated. Nothing here is fuzzy-matched on purpose: the round trip must be
/// exact, which is only true because `canvas_to_screen` multiplies before it
/// adds and `screen_to_canvas` divides the same difference back.
#[test]
fn d1_roundtrip_and_zoom_at_invariance_over_100_seeded_views() {
    let mut state: u32 = 1234;
    let mut rand = || {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        state as f64 / 4294967296.0
    };

    for _ in 0..100 {
        let v = view(
            ZOOM_LEVELS[(rand() * ZOOM_LEVELS.len() as f64).floor() as usize],
            (rand() * 4000.0).floor() as i64 as f64 - 2000.0,
            (rand() * 4000.0).floor() as i64 as f64 - 2000.0,
        );
        let (px, py) = (rand() * 512.0, rand() * 512.0);
        let s = canvas_to_screen(v, px, py);
        assert_eq!(
            pixel_at(v, s.x, s.y),
            Pixel {
                x: px.floor() as i64,
                y: py.floor() as i64,
            },
            "round trip drifted for view {v:?} at {px},{py}"
        );

        let nz = ZOOM_LEVELS[(rand() * ZOOM_LEVELS.len() as f64).floor() as usize];
        let (ax, ay) = (rand() * 800.0, rand() * 600.0);
        let before = screen_to_canvas(v, ax, ay);
        let after = screen_to_canvas(
            zoom_at(v, ax, ay, nz).expect("ladder zoom is valid"),
            ax,
            ay,
        );
        assert!(
            (before.x - after.x).abs() < 1e-9 && (before.y - after.y).abs() < 1e-9,
            "anchor drifted for view {v:?} -> zoom {nz}"
        );
    }
}

// --- tools/view_check.mjs / reference/view_ref.mjs additions ----------------

#[test]
fn zoom_ladder_is_the_frozen_constants_one() {
    // view.js re-exports ZOOM_LEVELS rather than restating it; make sure this
    // module really is that array and not a copy of it.
    assert_eq!(ZOOM_LEVELS.len(), 12);
    assert_eq!(
        ZOOM_LEVELS,
        [
            0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0
        ]
    );
    // Frozen order is load-bearing: `step_zoom` walks the indices, so a
    // reordered ladder would silently change which zoom comes next.
    for pair in ZOOM_LEVELS.windows(2) {
        assert!(pair[0] < pair[1], "ladder is not increasing: {pair:?}");
    }
}

#[test]
fn step_zoom_sticks_at_the_wrong_end_outside_the_ladder() {
    // Above the top rung, `find` matches nothing and the JS fell back to
    // ZOOM_LEVELS.at(-1) — so zooming *in* from 64× moves *down* to 32×.
    assert_eq!(step_zoom(64.0, 1), 32.0);
    assert_eq!(step_zoom(1000.0, 1), 32.0);
    // Below the bottom rung the same happens in reverse.
    assert_eq!(step_zoom(0.1, -1), 0.25);
    assert_eq!(step_zoom(0.0001, -1), 0.25);
}

#[test]
fn step_zoom_zero_direction_is_identity() {
    assert_eq!(step_zoom(5.0, 0), 5.0);
    assert_eq!(step_zoom(7.5, 0), 7.5);
    assert_eq!(step_zoom(64.0, 0), 64.0);
}

#[test]
fn step_zoom_walks_the_whole_ladder() {
    for (i, &level) in ZOOM_LEVELS.iter().enumerate() {
        let up = step_zoom(level, 1);
        let down = step_zoom(level, -1);
        assert_eq!(up, ZOOM_LEVELS[(i + 1).min(11)], "up from rung {i}");
        assert_eq!(down, ZOOM_LEVELS[i.saturating_sub(1)], "down from rung {i}");
    }
}

#[test]
fn fit_view_falls_back_to_the_smallest_rung() {
    // want < 0.25: no rung qualifies, so the ladder floor wins rather than a
    // continuous scale the ladder does not contain.
    assert_eq!(fit_view(2048.0, 1088.0, 100.0, 100.0).zoom(), 0.25);
}

#[test]
fn fit_view_takes_the_largest_rung_that_fits() {
    // want = min(768/1024, 568/1024) = 0.5547 -> the 0.5 rung.
    assert_eq!(fit_view(1024.0, 1024.0, 800.0, 600.0).zoom(), 0.5);
    // want = min(1216/512, 568/512) = 1.109 -> the 1 rung.
    assert_eq!(fit_view(512.0, 512.0, 1248.0, 600.0).zoom(), 1.0);
}

#[test]
fn centring_rounds_halves_up_the_way_math_round_does() {
    // `Math.round(-1.5) === -1` but `(-1.5f64).round() === -3`'s sibling
    // `(-1.5f64).round() === -2`. These are the only two cases where the two
    // rules can diverge, so they pin round_half_up directly.
    assert_eq!(actual_size_view(5.0, 5.0, 2.0, 2.0).offset_x(), -1.0);
    assert_eq!(actual_size_view(5.0, 5.0, 2.0, 2.0).offset_y(), -1.0);
    // Positive half: both rules agree at 2, but the case keeps the pair honest.
    assert_eq!(actual_size_view(1.0, 1.0, 4.0, 4.0).offset_x(), 2.0);
    // A 1024-wide canvas at 0.5× in an 800-wide viewport: (800-512)/2 = 144.
    assert_eq!(fit_view(1024.0, 1024.0, 800.0, 600.0).offset_x(), 144.0);
}

#[test]
fn visible_chunk_range_epsilon_stops_the_far_edge_over_reporting() {
    // The far edge lands exactly on canvas x=64 (chunk 2's first pixel is 64,
    // so chunk 2 is off screen). Without the 1e-9 the floor gives 2.
    assert_eq!(
        visible_chunk_range(View::identity(), 128.0, 128.0, 64.0, 64.0),
        Some(VisibleChunkRange {
            cx0: 0,
            cy0: 0,
            cx1: 1,
            cy1: 1,
        })
    );
}

#[test]
fn visible_chunk_range_clamps_the_near_edge_but_not_the_far_one() {
    // Panned far into the canvas, past a chunk boundary. Near edges start
    // mid-chunk and are floored; the far edges are capped by the canvas.
    let r = visible_chunk_range(view(1.0, -1000.0, -1000.0), 2048.0, 1088.0, 800.0, 600.0)
        .expect("still on screen");
    // Near edges start mid-chunk (canvas 1000,1000 -> chunk 31) and are floored.
    assert_eq!(r.cx0, 31);
    assert_eq!(r.cy0, 31);
    // X is viewport-limited: the far edge is canvas 1800, i.e. chunk 56.
    assert_eq!(r.cx1, 56);
    // Y is canvas-limited: the far edge is canvas 1600, past the 1088 bottom,
    // so the cap 1088/32-1 == 33 binds first.
    assert_eq!(r.cy1, 33);
    assert_eq!(r.cy1, (1088.0 / CHUNK_PX as f64) as u32 - 1);
    assert!(r.cx1 < (2048.0 / CHUNK_PX as f64) as u32 - 1);
}

#[test]
fn visible_chunk_range_is_none_just_off_the_right_edge() {
    // Canvas 512 wide, canvas origin at screen 600 in a 512-wide viewport:
    // the whole canvas sits to the right of the viewport.
    assert_eq!(
        visible_chunk_range(view(1.0, 600.0, 0.0), 512.0, 512.0, 512.0, 512.0),
        None
    );
}

#[test]
fn clamp_view_range_inversion_pins_to_hi_not_lo() {
    // canvas*zoom = 8 and viewport width = 50, so
    // lo = 32-8 = 24 > hi = 50-32 = 18. Math.min(Math.max(val, lo), hi) is
    // what the JS computed, and it lands on hi.
    let v = clamp_view(view(0.25, 0.0, 0.0), 32.0, 32.0, 50.0, 50.0);
    assert_eq!(v.offset_x(), 18.0);
    assert_eq!(v.offset_y(), 18.0);
    // The zoom is untouched: clamping only moves the offsets.
    assert_eq!(v.zoom(), 0.25);
}

#[test]
fn clamp_view_with_a_custom_margin() {
    // margin 0 collapses both ranges to the canvas/viewport edges: lo is
    // -canvas, hi is the full viewport width.
    assert_eq!(
        clamp_view_with_margin(view(1.0, 5000.0, -5000.0), 512.0, 512.0, 800.0, 600.0, 0.0),
        view(1.0, 800.0, -512.0)
    );
}

#[test]
fn pan_by_does_not_clamp_but_the_store_does() {
    let far = pan_by(view(1.0, 0.0, 0.0), 1e6, -1e6);
    assert_eq!(far.offset_x(), 1e6);
    assert_eq!(far.offset_y(), -1e6);
    // ...and committing the same view through the store pulls it back in.
    let change = store().set(far);
    assert_eq!(change.view.offset_x(), 768.0);
    assert_eq!(change.view.offset_y(), -480.0);
}

#[test]
fn create_view_defaults_absent_fields_like_the_js_nullish_operator() {
    assert_eq!(
        create_view(ViewOptions::default()).ok(),
        Some(View::identity())
    );
    assert_eq!(
        create_view(ViewOptions {
            zoom: None,
            offset_x: Some(7.0),
            offset_y: None,
        })
        .ok(),
        Some(view(1.0, 7.0, 0.0))
    );
}

#[test]
fn create_view_rejects_non_finite_offsets() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(View::new(1.0, bad, 0.0).is_err());
        assert!(View::new(1.0, 0.0, bad).is_err());
        assert!(View::new(bad, 0.0, 0.0).is_err());
    }
}

#[test]
fn zoom_at_rejects_non_positive_and_non_finite() {
    for bad in [-2.0, 0.0, f64::NAN, f64::INFINITY] {
        assert!(zoom_at(View::identity(), 0.0, 0.0, bad).is_err());
    }
}

#[test]
fn every_error_code_stays_inside_the_catalog() {
    let made = [
        View::new(0.0, 0.0, 0.0).unwrap_err(),
        View::new(1.0, f64::NAN, 0.0).unwrap_err(),
        zoom_at(View::identity(), 0.0, 0.0, -1.0).unwrap_err(),
        Viewport::new(0.0, 600.0).unwrap_err(),
        Viewport::new(f64::NAN, 600.0).unwrap_err(),
        ViewStore::new().set_canvas(31, 32).unwrap_err(),
    ];
    for e in made {
        assert!(
            CATALOG.contains(&e.code),
            "{} is outside the catalog",
            e.code
        );
    }
}

#[test]
fn viewport_new_rejects_a_non_positive_size() {
    assert!(Viewport::new(800.0, 600.0).is_ok());
    let e = Viewport::new(0.0, 0.0).unwrap_err();
    assert_eq!(e.code, CODE_VIEWPORT_INVALID);
    assert_eq!(e.message, "viewport size must be positive, got 0x0");
    // The JS's message for the same guard in document.js L62.
    let e = Viewport::new(-3.0, 8.0).unwrap_err();
    assert_eq!(e.code, CODE_VIEWPORT_INVALID);
}

// --- ViewStore (view_store.js) ----------------------------------------------

#[test]
fn store_defaults_to_800x600_and_zoom_1() {
    let s = ViewStore::new();
    assert_eq!(s.viewport(), Viewport::DEFAULT);
    assert_eq!(s.view(), View::identity());
    assert_eq!(s.canvas(), None);
    // `Default` is the same thing, and clippy's new_without_default wants it.
    assert_eq!(ViewStore::default().viewport(), Viewport::DEFAULT);
}

#[test]
fn store_set_clamps_against_the_attached_canvas() {
    // At zoom 2 the whole 512x512 canvas occupies 1024 CSS px, so the Y range
    // is [32 - 1024, 600 - 32] = [-992, 568] — not the zoom-1 [-480, 568].
    let change = store().set(view(2.0, 5000.0, -5000.0));
    assert_eq!(change.view, view(2.0, 768.0, -992.0));
    assert!(change.changed);
}

#[test]
fn store_without_a_canvas_does_not_clamp() {
    // The JS's `if (!doc) return { ...v }`: until the app reports a document,
    // the store holds whatever it was handed.
    let change = ViewStore::new().set(view(1.0, 1e6, -1e6));
    assert_eq!(change.view, view(1.0, 1e6, -1e6));
}

#[test]
fn store_zoom_to_ignores_bad_zoom_like_the_js() {
    // `zoomTo` in view_store.js returns silently where `zoomAt` throws.
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut s = store();
        let change = s.zoom_to(bad);
        assert_eq!(
            change.view,
            View::identity(),
            "zoom_to({bad}) moved the view"
        );
        assert!(!change.changed, "zoom_to({bad}) claimed a change");
    }
}

#[test]
fn store_zoom_in_out_walks_the_ladder_from_every_rung() {
    for i in 0..ZOOM_LEVELS.len() {
        // Not asserting `changed` here: rung 2 is 1.0, which is exactly the
        // store's starting view, so that one step is a genuine no-op.
        let mut s = store();
        s.set(view(ZOOM_LEVELS[i], 0.0, 0.0));
        assert_eq!(s.zoom_in().view.zoom(), ZOOM_LEVELS[(i + 1).min(11)]);
        let mut s = store();
        s.set(view(ZOOM_LEVELS[i], 0.0, 0.0));
        assert_eq!(s.zoom_out().view.zoom(), ZOOM_LEVELS[i.saturating_sub(1)]);
    }
}

#[test]
fn store_zoom_in_anchors_on_the_viewport_centre() {
    // The anchor is 400x300 (the JS fallback viewport), so the canvas point
    // under the middle of the viewport is the one that stays put.
    let mut s = store();
    s.zoom_in();
    let before = screen_to_canvas(View::identity(), 400.0, 300.0);
    let after = screen_to_canvas(s.view(), 400.0, 300.0);
    assert!((before.x - after.x).abs() < 1e-9);
    assert!((before.y - after.y).abs() < 1e-9);
}

#[test]
fn store_fit_and_actual_are_noops_without_a_canvas() {
    let mut s = ViewStore::new();
    assert!(!s.fit().changed);
    assert!(!s.actual_size().changed);
    assert_eq!(s.view(), View::identity());
    assert_eq!(s.canvas(), None);
}

#[test]
fn store_fit_and_actual_recentre_with_a_canvas() {
    let mut s = store();
    let change = s.fit();
    assert_eq!(change.view, view(1.0, 144.0, 44.0));
    let change = s.actual_size();
    assert_eq!(change.view, view(1.0, 144.0, 44.0));
    // A canvas that does not fit picks the largest rung that does. 1440x1088
    // in 800x600: want = min(768/1440, 568/1088) = 0.522 -> the 0.5 rung, and
    // it centres inside it.
    assert!(s.set_canvas(1440, 1088).is_ok());
    let change = s.fit();
    assert_eq!(change.view, view(0.5, 40.0, 28.0));
}

#[test]
fn store_pan_by_clamps() {
    let change = store().pan_by(1e6, -1e6);
    assert_eq!(change.view, view(1.0, 768.0, -480.0));
}

#[test]
fn store_set_viewport_reclamps_like_the_window_resize_handler() {
    // ui/app.js L160: `renderer.resize(); viewStore.set(viewStore.get())` —
    // the view is committed unchanged purely so the clamp re-runs.
    let mut s = store();
    assert_eq!(s.set(view(1.0, 500.0, 0.0)).view, view(1.0, 500.0, 0.0));
    let change = s.set_viewport(200.0, 200.0);
    // X hits the new right bound (200 - 32); Y was already at 0 and stays there.
    assert_eq!(change.view, view(1.0, 168.0, 0.0));
    assert_eq!(
        s.viewport(),
        Viewport::new(200.0, 200.0).expect("200x200 is a real viewport")
    );
}

#[test]
fn store_set_viewport_falls_back_to_the_default_for_bad_sizes() {
    // The JS's `try {} catch {}` around host.clientWidth: a bogus size never
    // raised, it silently became 800x600.
    let mut s = store();
    for bad in [(0.0, 0.0), (-1.0, 600.0), (f64::NAN, 600.0), (800.0, -2.0)] {
        s.set_viewport(bad.0, bad.1);
        assert_eq!(
            s.viewport(),
            Viewport::DEFAULT,
            "size {bad:?} was not ignored"
        );
    }
}

#[test]
fn store_events_names_nothing_outside_the_frozen_catalog() {
    // The JS store's subscribers were the renderer's frame scheduler and the
    // status bar; neither is a catalog signal, so this returns nothing at all.
    let change = store().set(view(1.0, 0.0, 0.0));
    assert!(!change.changed);
    assert!(change.events().is_empty());
    let change = store().set(view(1.0, 1e6, 0.0));
    assert!(change.changed);
    assert!(change.events().is_empty());
    // ...and if a signal is ever wired up, it has to come from the catalog.
    for name in change.events() {
        assert!(
            crate::events::EVENTS.contains(name),
            "{name} is not a frozen event name"
        );
    }
}

#[test]
fn store_default_view_is_identity_and_set_canvas_rejects_bad_sizes() {
    let mut s = ViewStore::new();
    assert_eq!(s.view(), View::identity());
    // 31 is below MIN_SIZE_PX; the constants guard raises CANVAS_SIZE_INVALID.
    assert!(s.set_canvas(31, 32).is_err());
    assert_eq!(s.canvas(), None);
    assert!(s.set_canvas(2048, 1088).is_ok());
    assert_eq!(s.canvas(), Some((2048, 1088)));
    s.clear_canvas();
    assert_eq!(s.canvas(), None);
}
