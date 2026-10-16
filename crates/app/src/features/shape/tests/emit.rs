//! Emission text, overlay ops, and the tool factory.

use pixel_engine::model::event::EventDetail;

use crate::features::shape::shape_tools;
use crate::tools::api::OverlayInfo;
use crate::tools::overlay::OverlayOp;

use super::{drag, rect_tool};
use crate::tools::tests::{session64, view1};

/// The emission text is byte-exact `JSON.stringify({pending, vertices})`.
#[test]
fn pending_text_matches_the_js_serialization() {
    let (mut session, queue) = session64();
    let mut tool = rect_tool();
    drag(&mut tool, &mut session, 10.0, 10.0, 20.0, 12.0);
    let states: Vec<(String, String)> = queue
        .borrow()
        .iter()
        .filter(|ev| matches!(&ev.detail, EventDetail::ToolState { .. }))
        .map(|ev| match &ev.detail {
            EventDetail::ToolState { tool, pending } => (tool.clone(), pending.clone()),
            _ => unreachable!("filtered to tool states"),
        })
        .collect();
    let (_, last) = states.last().expect("tool state emitted");
    // readPending box literal order {x,y,w,h,radius}, default radius 8.
    assert_eq!(last, "{\"x\":10,\"y\":10,\"w\":11,\"h\":3,\"radius\":8}");
}

/// Overlay emits mask layers, a dashed frame and handles.
#[test]
fn overlay_produces_layers_frame_and_handles() {
    use crate::tools::api::Tool;
    let (mut session, _queue) = session64();
    let mut tool = rect_tool();
    drag(&mut tool, &mut session, 10.0, 10.0, 20.0, 12.0);
    let info = OverlayInfo {
        view: view1(),
        dpr: 1.0,
    };
    let mut ops = vec![];
    tool.overlay(&info, &mut ops);
    assert!(
        ops.iter()
            .any(|op| matches!(op, OverlayOp::MaskBlit { .. })),
        "WYSIWYG mask layer, got {ops:?}"
    );
    let frames: Vec<_> = ops
        .iter()
        .filter(|op| matches!(op, OverlayOp::StrokeRect { dash: Some(_), .. }))
        .collect();
    assert_eq!(frames.len(), 1, "one dashed frame, got {ops:?}");
    // Eight handles × (white fill + black outline).
    let fills = ops
        .iter()
        .filter(|op| matches!(op, OverlayOp::FillRect { .. }))
        .count();
    assert!(fills >= 8, "handles drawn, got {fills}");
}

/// The factory registers all five kinds in `KINDS` order.
#[test]
fn shape_tools_registers_five_kinds() {
    let tools = shape_tools();
    let ids: Vec<&str> = tools.iter().map(|t| t.id()).collect();
    assert_eq!(ids, ["line", "rect", "rrect", "ellipse", "polygon"]);
}
