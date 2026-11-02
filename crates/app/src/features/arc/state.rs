//! Arc/chord kinds, drag/pending geometry, and the preview-state key.

use std::cell::RefCell;

use crate::features::shape::PendingGeom;
use crate::tools::overlay::OverlayOp;

use super::preview::PreviewKey;

/// Which registered tool this struct serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcKind {
    Arc,
    Chord,
}

/// Chord paint style — `chordMode` in the JS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordMode {
    Fill,
    Outline,
}

/// Frozen pointer-down state used while the drag is in flight.
#[derive(Debug, Clone)]
pub(super) struct Draw {
    pub(super) x0: i32,
    pub(super) y0: i32,
    pub(super) x1: i32,
    pub(super) y1: i32,
    pub(super) layer_id: String,
    pub(super) fill_color: String,
    pub(super) outline_color: String,
    pub(super) start: f64,
    pub(super) end: f64,
    pub(super) width: i32,
    pub(super) chord_mode: ChordMode,
    pub(super) shift: bool,
    pub(super) alt: bool,
}

impl Draw {
    pub(super) fn from_pending(p: &PendingArc) -> Option<Self> {
        let b = p.geom.bbox?;
        Some(Self {
            x0: b.x,
            y0: b.y,
            x1: b.x + b.w - 1,
            y1: b.y + b.h - 1,
            layer_id: p.layer_id.clone(),
            fill_color: p.fill_color.clone(),
            outline_color: p.outline_color.clone(),
            start: p.start,
            end: p.end,
            width: p.width,
            chord_mode: p.chord_mode,
            shift: false,
            alt: false,
        })
    }
}

/// A committed-on-confirm arc box.
#[derive(Debug, Clone)]
pub(super) struct PendingArc {
    pub(super) layer_id: String,
    pub(super) fill_color: String,
    pub(super) outline_color: String,
    pub(super) start: f64,
    pub(super) end: f64,
    pub(super) width: i32,
    pub(super) chord_mode: ChordMode,
    pub(super) geom: PendingGeom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Idle,
    Drawing,
    Pending,
    Resizing,
    Moving,
}

#[derive(Debug, Clone)]
pub(super) enum Op {
    Resize { handle: String, orig: PendingArc },
    Move { ox: i32, oy: i32, orig: PendingArc },
}

pub struct ArcTool {
    pub(super) kind: ArcKind,
    pub(super) start: f64,
    pub(super) end: f64,
    pub(super) width: i32,
    pub(super) chord_mode: ChordMode,
    pub(super) draw: Option<Draw>,
    pub(super) pending: Option<PendingArc>,
    pub(super) op: Option<Op>,
    pub(super) mode: Mode,
    /// Last-seen canvas size — `overlay()` has no session.
    pub(super) canvas: Option<(u32, u32)>,
    /// `(box, options, colour)`-keyed preview: the mask is canvas-sized.
    pub(super) preview: RefCell<Option<(PreviewKey, OverlayOp)>>,
}
