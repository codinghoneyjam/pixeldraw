//! Radial-gradient acceptance: commit label, radius floor, `min` clamp,
//! edge modes, colour snapshots, overlay ring.

use pixel_engine::model::{NewDocumentOptions, Session, SettingInput};
use pixel_engine::pixel::pack_rgba;
use pixel_engine::view::View;

use crate::input::{Button, RawPointer, build_tool_event};
use crate::tools::api::{CursorKind, OverlayInfo, Tool, ToolCx};
use crate::tools::overlay::OverlayOp;

use super::GradientTool;

const W: u32 = 64;
const H: u32 = 64;

fn session64() -> Session {
    let mut session = Session::new();
    session
        .new_document(NewDocumentOptions::default().size(W, H))
        .expect("new document");
    session
}

fn view1() -> View {
    View::new(1.0, 0.0, 0.0).expect("valid view")
}

fn set_colors(session: &mut Session, primary: &str, secondary: &str) {
    session
        .set_setting("primaryColor", SettingInput::Str(primary))
        .expect("primary");
    session
        .set_setting("secondaryColor", SettingInput::Str(secondary))
        .expect("secondary");
}

fn pixel(session: &Session, x: u32, y: u32) -> u32 {
    let doc = session.doc().expect("doc");
    let layer = doc.get_layer(&doc.active_layer_id).expect("active");
    layer.store.get_pixel(x, y)
}

fn painted_count(session: &Session) -> usize {
    let doc = session.doc().expect("doc");
    let layer = doc.get_layer(&doc.active_layer_id).expect("active");
    let mut n = 0;
    for y in 0..H {
        for x in 0..W {
            if layer.store.get_pixel(x, y) != 0 {
                n += 1;
            }
        }
    }
    n
}

fn drag(tool: &mut GradientTool, session: &mut Session, x0: f64, y0: f64, x1: f64, y1: f64) {
    let mut render = false;
    let view = view1();
    let mut cx = ToolCx {
        session,
        view,
        need_render: &mut render,
    };
    tool.pointer_down(
        &build_tool_event(&RawPointer::new((x0, y0)), &view),
        &mut cx,
    );
    tool.pointer_move(
        &build_tool_event(&RawPointer::new((x1, y1)), &view),
        &mut cx,
    );
    tool.pointer_up(
        &build_tool_event(&RawPointer::new((x1, y1)), &view),
        &mut cx,
    );
}

#[test]
fn drag_commits_one_undo_step_labelled_gradient() {
    let mut session = session64();
    set_colors(&mut session, "#ff0000", "#0000ff");
    let mut tool = GradientTool::new();
    assert_eq!(tool.id(), "radial_gradient");
    assert_eq!(tool.cursor(), CursorKind::Crosshair);
    drag(&mut tool, &mut session, 32.0, 32.0, 40.0, 32.0);
    assert!(!session.has_stroke());
    assert!(painted_count(&session) > 0, "the gradient painted");
    assert_eq!(
        session.history().undo_label().as_deref(),
        Some("방사형 그라디언트"),
        "the JS history label, verbatim"
    );
    assert!(session.undo().expect("undo"));
    assert_eq!(painted_count(&session), 0, "undo restores transparency");
}

#[test]
fn sub_pixel_radius_is_ignored() {
    let mut session = session64();
    let mut tool = GradientTool::new();
    drag(&mut tool, &mut session, 10.0, 10.0, 10.0, 10.0);
    assert_eq!(painted_count(&session), 0);
    assert!(!session.undo().expect("undo"), "no history entry");
}

#[test]
fn inner_radius_clamps_to_the_drag_radius() {
    // `r0 = min(innerRadius, radius)`: with r0 == r1 the span collapses and
    // the centre pixel takes the inner stop exactly.
    let mut session = session64();
    set_colors(&mut session, "#ff0000", "#0000ff");
    let mut tool = GradientTool::new();
    assert!(tool.set_inner_radius(100.0));
    assert_eq!(tool.inner_radius(), 100.0);
    drag(&mut tool, &mut session, 32.0, 32.0, 40.0, 32.0);
    assert_eq!(
        pixel(&session, 32, 32),
        pack_rgba(255.0, 0.0, 0.0, 255.0),
        "the centre pixel is the inner stop"
    );
}

#[test]
fn edge_mode_decides_the_outer_alpha() {
    // At d == r1 the stop is exactly t == 1: transparent leaves the pixel
    // empty, solid paints the outer colour at full alpha.
    let mut transparent = session64();
    set_colors(&mut transparent, "#ff0000", "#0000ff");
    let mut tool = GradientTool::new();
    drag(&mut tool, &mut transparent, 32.0, 32.0, 40.0, 32.0);
    assert_eq!(pixel(&transparent, 40, 32), 0, "transparent edge fades out");

    let mut solid = session64();
    set_colors(&mut solid, "#ff0000", "#0000ff");
    let mut tool = GradientTool::new();
    assert!(tool.set_edge_mode("solid"));
    assert_eq!(tool.edge_mode_str(), "solid");
    drag(&mut tool, &mut solid, 32.0, 32.0, 40.0, 32.0);
    assert_eq!(
        pixel(&solid, 40, 32),
        pack_rgba(0.0, 0.0, 255.0, 255.0),
        "solid edge keeps the outer alpha"
    );
}

#[test]
fn drag_ignores_a_mid_drag_colour_change() {
    let mut session = session64();
    set_colors(&mut session, "#ff0000", "#0000ff");
    let mut tool = GradientTool::new();
    let mut render = false;
    let view = view1();
    {
        let mut cx = ToolCx {
            session: &mut session,
            view,
            need_render: &mut render,
        };
        tool.pointer_down(
            &build_tool_event(&RawPointer::new((32.0, 32.0)), &view),
            &mut cx,
        );
        cx.session
            .set_setting("primaryColor", SettingInput::Str("#00ff00"))
            .expect("recolour mid-drag");
        tool.pointer_move(
            &build_tool_event(&RawPointer::new((40.0, 32.0)), &view),
            &mut cx,
        );
        tool.pointer_up(
            &build_tool_event(&RawPointer::new((40.0, 32.0)), &view),
            &mut cx,
        );
    }
    assert_eq!(
        pixel(&session, 32, 32),
        pack_rgba(255.0, 0.0, 0.0, 255.0),
        "the centre keeps the down-time primary"
    );
}

#[test]
fn overlay_emits_a_blit_and_the_radius_ring() {
    let mut session = session64();
    set_colors(&mut session, "#ff0000", "#0000ff");
    let mut tool = GradientTool::new();
    let mut render = false;
    let view = view1();
    let info = OverlayInfo { view, dpr: 1.0 };
    let mut cx = ToolCx {
        session: &mut session,
        view,
        need_render: &mut render,
    };
    tool.pointer_down(
        &build_tool_event(&RawPointer::new((32.0, 32.0)), &view),
        &mut cx,
    );
    // Below the radius floor there is no preview at all.
    let mut ops = Vec::new();
    tool.overlay(&info, &mut ops);
    assert!(ops.is_empty(), "radius < 1 previews nothing");
    tool.pointer_move(
        &build_tool_event(&RawPointer::new((40.0, 32.0)), &view),
        &mut cx,
    );
    let mut ops = Vec::new();
    tool.overlay(&info, &mut ops);
    assert_eq!(ops.len(), 2, "blit + ring");
    match &ops[0] {
        OverlayOp::MaskBlit { w, h, data, .. } => {
            assert!(data.iter().any(|&p| p != 0), "the blit carries pixels");
            assert_eq!((*w as usize) * (*h as usize), data.len());
        }
        op => panic!("first op is the blit, got {op:?}"),
    }
    match &ops[1] {
        OverlayOp::Circle {
            cx,
            cy,
            r,
            rgba,
            width,
            dash,
        } => {
            assert_eq!((*cx, *cy), (32.0, 32.0));
            assert_eq!(*r, 8.0, "radius * zoom * dpr");
            assert_eq!(*rgba, [255, 255, 255, 230]);
            assert_eq!(*width, 1.0);
            assert_eq!(
                *dash,
                Some((4.0, 3.0)),
                "verbatim setLineDash, no dpr factor"
            );
        }
        op => panic!("second op is the ring, got {op:?}"),
    }
    // A repeat call with no movement reuses the cached blit.
    let mut ops2 = Vec::new();
    tool.overlay(&info, &mut ops2);
    assert_eq!(ops, ops2, "the cached preview is stable");
}

#[test]
fn option_setters_follow_the_panel_rules() {
    let mut tool = GradientTool::new();
    assert_eq!(tool.inner_radius(), 0.0);
    assert_eq!(tool.edge_mode_str(), "transparent");
    assert!(!tool.set_inner_radius(1.5), "non-integers are ignored");
    assert_eq!(tool.inner_radius(), 0.0);
    assert!(!tool.set_inner_radius(f64::NAN), "non-finite is ignored");
    assert!(tool.set_inner_radius(3000.0), "clamped into range");
    assert_eq!(tool.inner_radius(), 2048.0);
    assert!(tool.set_inner_radius(-4.0));
    assert_eq!(tool.inner_radius(), 0.0);
    assert!(!tool.set_edge_mode("bogus"), "unknown modes rejected");
    assert_eq!(tool.edge_mode_str(), "transparent");
    assert!(tool.set_edge_mode("solid"));
    assert!(
        !tool.set_edge_mode("solid"),
        "unchanged value reports false"
    );
}

#[test]
fn non_left_buttons_and_cancel_do_nothing() {
    let mut session = session64();
    let mut tool = GradientTool::new();
    let mut render = false;
    let view = view1();
    let mut cx = ToolCx {
        session: &mut session,
        view,
        need_render: &mut render,
    };
    let right = RawPointer {
        button: Button::Right,
        ..RawPointer::new((32.0, 32.0))
    };
    tool.pointer_down(&build_tool_event(&right, &view), &mut cx);
    assert!(!tool.is_drawing());
    assert!(!cx.session.has_stroke());
    tool.pointer_down(
        &build_tool_event(&RawPointer::new((32.0, 32.0)), &view),
        &mut cx,
    );
    assert!(tool.is_drawing());
    tool.cancel(&mut cx);
    assert!(!tool.is_drawing());
    assert!(!session.undo().expect("undo"), "cancel leaves no history");
}
