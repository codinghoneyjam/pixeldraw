//! `features/shape/shape_geom.js` — pure shape geometry.
//!
//! Facade: shared types in `types`, handle math in `handles`, drag/resize/move
//! in `ops`, option-bar parsing and spec building in `spec`.

mod handles;
mod ops;
mod spec;
mod types;

pub use handles::{hit_handle, inside_shape, layout_handles};
pub use ops::{drag_geom, move_geom, resize_geom, snap_place_point};
pub use spec::{build_spec, draw_spec, parse_box_geom, parse_line_geom, parse_polygon_geom};
pub use types::{Bbox, DragFlags, PendingGeom, Pt};
