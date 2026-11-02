//! Gradient edge mode, drag/pending state, and the pending-box geometry helpers.

use std::cell::RefCell;

use crate::features::shape::{Bbox, PendingGeom};
use crate::tools::overlay::OverlayOp;

/// Gradient edge falloff — `edgeMode` in the JS (`'transparent'|'solid'`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeMode {
    Transparent,
    Solid,
}

impl EdgeMode {
    pub(super) fn parse(s: &str) -> Option<Self> {
        match s {
            "transparent" => Some(EdgeMode::Transparent),
            "solid" => Some(EdgeMode::Solid),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            EdgeMode::Transparent => "transparent",
            EdgeMode::Solid => "solid",
        }
    }
}

/// Frozen pointer-down colours and centre used while dragging.
#[derive(Debug, Clone)]
pub(super) struct Center {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) x1: i32,
    pub(super) y1: i32,
    pub(super) layer_id: String,
    pub(super) primary: String,
    pub(super) secondary: String,
}

#[derive(Debug, Clone)]
pub(super) struct PendingGradient {
    pub(super) layer_id: String,
    pub(super) primary: String,
    pub(super) secondary: String,
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
    Resize {
        handle: String,
        orig: PendingGradient,
    },
    Move {
        ox: i32,
        oy: i32,
        orig: PendingGradient,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PreviewKey {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) x1: i32,
    pub(super) y1: i32,
    pub(super) inner_radius: u64,
    pub(super) edge_solid: bool,
    pub(super) primary: String,
    pub(super) secondary: String,
}

pub struct GradientTool {
    pub(super) inner_radius: f64,
    pub(super) edge_mode: EdgeMode,
    pub(super) center: Option<Center>,
    pub(super) pending: Option<PendingGradient>,
    pub(super) op: Option<Op>,
    pub(super) mode: Mode,
    /// Last-seen canvas size — `overlay()` has no session.
    pub(super) canvas: Option<(u32, u32)>,
    pub(super) preview: RefCell<Option<(PreviewKey, OverlayOp)>>,
}

/// `[cx, cy, radius]` from a pending box; the box is the gradient's bounds.
pub(super) fn circle_of(geom: &PendingGeom) -> Option<(i32, i32, f64)> {
    let b = geom.bbox?;
    let side = b.w.min(b.h).max(1);
    let radius = (side - 1) as f64 / 2.0;
    Some((b.x + (b.w - 1) / 2, b.y + (b.h - 1) / 2, radius))
}

pub(super) fn square_geom(cx: i32, cy: i32, radius: i32) -> PendingGeom {
    let r = radius.max(1);
    PendingGeom {
        bbox: Some(Bbox {
            x: cx - r,
            y: cy - r,
            w: r * 2 + 1,
            h: r * 2 + 1,
        }),
        p0: None,
        p1: None,
        radius: 0,
        points: vec![],
        preset: None,
    }
}
