//! `features/shape/*` — five shape tools over one pending state machine.
//!
//! Facade: state machine in `state`, geometry in `geom`, placing + text in
//! `pending`, keys in `keys`, commit in `commit`, overlay in `overlay`, and
//! the option-bar block in `optionbar`.

mod commit;
mod geom;
mod keys;
pub(crate) mod overlay;
mod pending;
mod state;

pub mod optionbar;

#[cfg(test)]
mod tests;

pub use commit::{commit_spec, paint_preview};
pub use geom::{
    Bbox, DragFlags, PendingGeom, Pt, ShapePreset, drag_geom, hit_handle, inside_shape,
    layout_handles, move_geom, preset_points, resize_geom,
};
pub use pending::PendingValue;
pub use state::{PendingShape, ShapeTool};

use pixel_engine::raster::shape_raster::ShapeKind;

use crate::tools::api::Tool;

/// The five registered shape tools, in the JS `KINDS` order.
pub fn shape_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ShapeTool::new(ShapeKind::Line)),
        Box::new(ShapeTool::new(ShapeKind::Rect)),
        Box::new(ShapeTool::new(ShapeKind::Rrect)),
        Box::new(ShapeTool::new(ShapeKind::Ellipse)),
        Box::new(ShapeTool::new(ShapeKind::Polygon)),
    ]
}
