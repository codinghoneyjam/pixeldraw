//! Polygon placing, closing, and committing.

use pixel_engine::raster::shape_raster::ShapeKind;

use crate::input::build_tool_event;
use crate::tools::api::{Tool, ToolCx};
use crate::tools::shape::ShapeTool;

use super::super::super::tests::{down, pixel, session64, view1};
use super::key;

/// Polygon: three clicks place, Enter makes pending, Enter commits.
#[test]
fn polygon_place_enter_enter() {
    let (mut session, _queue) = session64();
    let mut tool = ShapeTool::new(ShapeKind::Polygon);
    let mut render = false;
    let mut cx = ToolCx {
        session: &mut session,
        view: view1(),
        need_render: &mut render,
    };
    for (x, y) in [(10.0, 10.0), (20.0, 10.0), (15.0, 20.0)] {
        tool.pointer_down(&build_tool_event(&down(x, y), &view1()), &mut cx);
    }
    assert!(tool.is_placing());
    assert!(!tool.has_pending());
    assert!(
        tool.key_down(&key("Enter", false), &mut cx),
        "Enter finishes placing"
    );
    assert!(tool.has_pending(), "three vertices become pending");
    assert!(
        tool.key_down(&key("Enter", false), &mut cx),
        "Enter commits pending"
    );
    assert!(!tool.has_pending());
    // Default fill is outline: the top edge paints, the middle stays empty.
    assert_ne!(pixel(&session, 15, 10), 0, "triangle top edge painted");
    assert_eq!(
        pixel(&session, 15, 12),
        0,
        "outline leaves the middle empty"
    );
    assert!(session.undo().expect("undo"));
    assert_eq!(pixel(&session, 15, 10), 0);
}

/// Clicking the first vertex closes the polygon without Enter.
#[test]
fn polygon_closes_on_first_vertex_click() {
    let (mut session, _queue) = session64();
    let mut tool = ShapeTool::new(ShapeKind::Polygon);
    let mut render = false;
    let mut cx = ToolCx {
        session: &mut session,
        view: view1(),
        need_render: &mut render,
    };
    for (x, y) in [(10.0, 10.0), (20.0, 10.0), (15.0, 20.0), (10.0, 10.0)] {
        tool.pointer_down(&build_tool_event(&down(x, y), &view1()), &mut cx);
    }
    assert!(tool.has_pending(), "closing click makes pending");
}
