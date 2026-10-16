//! W6 acceptance — drag→pending→commit, placing, keys, handles, overlay.
//!
//! Facade: shared drag helpers here, one module per area beside it.

mod adjust;
mod emit;
mod pointer;
mod polygon;

use pixel_engine::raster::shape_raster::ShapeKind;

use crate::features::shape::ShapeTool;
use crate::input::{KeyEvent, build_tool_event};
use crate::tools::api::{Tool, ToolCx};

use crate::tools::tests::{down, view1};

pub(super) fn rect_tool() -> ShapeTool {
    ShapeTool::new(ShapeKind::Rect)
}

pub(super) fn key(name: &str, shift: bool) -> KeyEvent {
    KeyEvent {
        key: name.to_string(),
        code: name.to_string(),
        shift,
        ctrl: false,
        alt: false,
    }
}

pub(super) fn drag(
    tool: &mut ShapeTool,
    session: &mut pixel_engine::model::Session,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) {
    let mut render = false;
    let mut cx = ToolCx {
        session,
        view: view1(),
        need_render: &mut render,
    };
    tool.pointer_down(&build_tool_event(&down(x0, y0), &view1()), &mut cx);
    tool.pointer_move(&build_tool_event(&down(x1, y1), &view1()), &mut cx);
    tool.pointer_up(&build_tool_event(&down(x1, y1), &view1()), &mut cx);
}
