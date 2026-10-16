//! Shared shape geometry types.

/// Canvas pixel point.
pub type Pt = (i32, i32);

/// `[x, y, w, h]` with `w,h >= 1`, like every JS bbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bbox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Pending geometry: exactly one variant is live, implied by the tool kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingGeom {
    pub bbox: Option<Bbox>,
    pub p0: Option<Pt>,
    pub p1: Option<Pt>,
    pub radius: i32,
    pub points: Vec<Pt>,
}

/// Drag flags — `_flags(ev)` in the JS: shift locks, alt centers.
#[derive(Debug, Clone, Copy)]
pub struct DragFlags {
    pub shift: bool,
    pub lock: bool,
    pub center: bool,
    pub snap: bool,
}

/// Handle ids in the JS fixed order.
pub const HANDLE_IDS: [&str; 8] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];

#[derive(Debug, Clone, PartialEq)]
pub struct Handle {
    pub id: String,
    /// Line endpoint index (`p0`/`p1`); `None` for box and polygon handles.
    pub end: Option<u8>,
    pub x: f64,
    pub y: f64,
}
