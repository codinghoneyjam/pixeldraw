//! Arc/chord acceptance: commit labels, degenerate drags, colour snapshots,
//! option setters, overlay-preview parity.

use std::collections::HashSet;

use pixel_engine::model::{NewDocumentOptions, Session, SettingInput};
use pixel_engine::pixel::pack_rgba;
use pixel_engine::view::View;

use crate::input::{Button, RawPointer, build_tool_event};
use crate::tools::api::{CursorKind, OverlayInfo, Tool, ToolCx};
use crate::tools::overlay::OverlayOp;

use super::{ArcTool, ChordMode};

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

fn painted(session: &Session) -> HashSet<(i32, i32)> {
    let doc = session.doc().expect("doc");
    let layer = doc.get_layer(&doc.active_layer_id).expect("active");
    let mut out = HashSet::new();
    for y in 0..H {
        for x in 0..W {
            if layer.store.get_pixel(x, y) != 0 {
                out.insert((x as i32, y as i32));
            }
        }
    }
    out
}

fn drag(tool: &mut ArcTool, session: &mut Session, x0: f64, y0: f64, x1: f64, y1: f64) {
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

fn red() -> u32 {
    pack_rgba(255.0, 0.0, 0.0, 255.0)
}

fn green() -> u32 {
    pack_rgba(0.0, 255.0, 0.0, 255.0)
}

fn blue() -> u32 {
    pack_rgba(0.0, 0.0, 255.0, 255.0)
}

#[test]
fn arc_drag_commits_one_undo_step_labelled_ho() {
    let mut session = session64();
    set_colors(&mut session, "#ff0000", "#0000ff");
    let mut tool = ArcTool::arc();
    assert_eq!(tool.id(), "arc");
    assert_eq!(tool.cursor(), CursorKind::Crosshair);
    drag(&mut tool, &mut session, 10.0, 10.0, 30.0, 26.0);
    assert!(!session.has_stroke());
    let cells = painted(&session);
    assert!(!cells.is_empty(), "the arc stroke painted");
    assert_eq!(
        session.history().undo_label().as_deref(),
        Some("호"),
        "the JS history label, verbatim"
    );
    assert!(session.undo().expect("undo"));
    assert!(painted(&session).is_empty(), "undo restores transparency");
}

#[test]
fn chord_drag_commits_labelled_hyeon() {
    let mut session = session64();
    set_colors(&mut session, "#ff0000", "#0000ff");
    let mut tool = ArcTool::chord();
    assert_eq!(tool.id(), "chord");
    drag(&mut tool, &mut session, 8.0, 8.0, 28.0, 24.0);
    assert!(!painted(&session).is_empty());
    assert_eq!(
        session.history().undo_label().as_deref(),
        Some("현"),
        "the JS history label, verbatim"
    );
    assert!(session.undo().expect("undo"));
    assert!(painted(&session).is_empty());
}

#[test]
fn degenerate_axis_drag_paints_nothing() {
    // Either axis equal means no box — the JS checks `||`, not `&&`.
    for (x0, y0, x1, y1) in [(10.0, 10.0, 10.0, 20.0), (10.0, 10.0, 20.0, 10.0)] {
        let mut session = session64();
        let mut tool = ArcTool::arc();
        drag(&mut tool, &mut session, x0, y0, x1, y1);
        assert!(
            painted(&session).is_empty(),
            "no pixels for {x0},{y0}→{x1},{y1}"
        );
        assert!(
            !session.undo().expect("undo"),
            "no history entry for {x0},{y0}→{x1},{y1}"
        );
    }
}

#[test]
fn chord_outline_paints_the_secondary_colour() {
    let mut session = session64();
    set_colors(&mut session, "#ff0000", "#0000ff");
    let mut tool = ArcTool::chord();
    tool.set_chord_mode(ChordMode::Outline);
    assert!(tool.outline());
    drag(&mut tool, &mut session, 8.0, 8.0, 28.0, 24.0);
    let doc = session.doc().expect("doc");
    let layer = doc.get_layer(&doc.active_layer_id).expect("active");
    let (mut blues, mut reds) = (0, 0);
    for y in 0..H {
        for x in 0..W {
            match layer.store.get_pixel(x, y) {
                p if p == blue() => blues += 1,
                p if p == red() => reds += 1,
                _ => {}
            }
        }
    }
    assert!(blues > 0, "outline chord paints secondary");
    assert_eq!(reds, 0, "no primary pixels in an outline chord");
}

#[test]
fn drag_ignores_a_mid_drag_colour_change() {
    let mut session = session64();
    set_colors(&mut session, "#ff0000", "#0000ff");
    let mut tool = ArcTool::arc();
    let mut render = false;
    let view = view1();
    {
        let mut cx = ToolCx {
            session: &mut session,
            view,
            need_render: &mut render,
        };
        tool.pointer_down(
            &build_tool_event(&RawPointer::new((10.0, 10.0)), &view),
            &mut cx,
        );
        cx.session
            .set_setting("primaryColor", SettingInput::Str("#00ff00"))
            .expect("recolour mid-drag");
        tool.pointer_move(
            &build_tool_event(&RawPointer::new((30.0, 26.0)), &view),
            &mut cx,
        );
        tool.pointer_up(
            &build_tool_event(&RawPointer::new((30.0, 26.0)), &view),
            &mut cx,
        );
    }
    let doc = session.doc().expect("doc");
    let layer = doc.get_layer(&doc.active_layer_id).expect("active");
    let (mut reds, mut greens) = (0, 0);
    for y in 0..H {
        for x in 0..W {
            match layer.store.get_pixel(x, y) {
                p if p == red() => reds += 1,
                p if p == green() => greens += 1,
                _ => {}
            }
        }
    }
    assert!(reds > 0, "the down-time snapshot colour painted");
    assert_eq!(greens, 0, "the mid-drag colour never painted");
}

#[test]
fn overlay_preview_matches_the_committed_pixels() {
    let mut session = session64();
    set_colors(&mut session, "#000000", "#0000ff");
    let mut tool = ArcTool::arc();
    let mut render = false;
    let view = view1();
    let info = OverlayInfo { view, dpr: 1.0 };
    {
        let mut cx = ToolCx {
            session: &mut session,
            view,
            need_render: &mut render,
        };
        tool.pointer_down(
            &build_tool_event(&RawPointer::new((10.0, 10.0)), &view),
            &mut cx,
        );
        tool.pointer_move(
            &build_tool_event(&RawPointer::new((30.0, 26.0)), &view),
            &mut cx,
        );
        let mut ops = Vec::new();
        tool.overlay(&info, &mut ops);
        let mut preview = HashSet::new();
        for op in &ops {
            if let OverlayOp::MaskBlit { x, y, w, h, data } = op {
                for dy in 0..*h {
                    for dx in 0..*w {
                        if data[dy as usize * *w as usize + dx as usize] != 0 {
                            preview.insert((x + dx as i32, y + dy as i32));
                        }
                    }
                }
            }
        }
        assert!(!preview.is_empty(), "the drag previews pixels");
        // A second overlay call with no movement reuses the cache.
        let mut ops2 = Vec::new();
        tool.overlay(&info, &mut ops2);
        assert_eq!(ops, ops2, "the cached preview is stable");
        tool.pointer_up(
            &build_tool_event(&RawPointer::new((30.0, 26.0)), &view),
            &mut cx,
        );
        assert_eq!(painted(cx.session), preview, "preview == commit (WYSIWYG)");
    }
}

#[test]
fn option_setters_follow_the_panel_rules() {
    let mut tool = ArcTool::arc();
    assert_eq!((tool.start(), tool.end(), tool.width()), (0.0, 180.0, 2));
    assert!(tool.set_start(45.0));
    assert!(!tool.set_start(45.0), "unchanged value reports false");
    assert!(!tool.set_start(f64::NAN), "non-finite start rejected");
    assert!(!tool.set_end(f64::INFINITY), "non-finite end rejected");
    assert!(tool.set_end(-30.0), "angles are unbounded");
    assert!(tool.set_width(100), "clamped into range");
    assert_eq!(tool.width(), 64);
    assert!(tool.set_width(-5));
    assert_eq!(tool.width(), 1);
    assert!(!tool.set_width(1), "unchanged value reports false");
    // The JS fill/outline toggle pair.
    assert!(tool.fill());
    tool.set_fill(false);
    assert!(tool.outline());
    tool.set_fill(false);
    assert!(tool.outline(), "re-clearing keeps outline");
    tool.set_outline(false);
    assert!(tool.fill(), "clearing outline restores fill");
}

#[test]
fn non_left_buttons_and_cancel_do_nothing() {
    let mut session = session64();
    let mut tool = ArcTool::arc();
    let mut render = false;
    let view = view1();
    let mut cx = ToolCx {
        session: &mut session,
        view,
        need_render: &mut render,
    };
    let right = RawPointer {
        button: Button::Right,
        ..RawPointer::new((10.0, 10.0))
    };
    tool.pointer_down(&build_tool_event(&right, &view), &mut cx);
    assert!(!tool.is_drawing());
    assert!(!cx.session.has_stroke());
    tool.pointer_down(
        &build_tool_event(&RawPointer::new((10.0, 10.0)), &view),
        &mut cx,
    );
    assert!(tool.is_drawing());
    tool.cancel(&mut cx);
    assert!(!tool.is_drawing());
    let mut ops = Vec::new();
    tool.overlay(&OverlayInfo { view, dpr: 1.0 }, &mut ops);
    assert!(ops.is_empty(), "no preview after cancel");
    assert!(!session.undo().expect("undo"), "cancel leaves no history");
}
