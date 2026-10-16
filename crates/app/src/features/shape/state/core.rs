//! Shape tool state — modes, pending shape, and the small accessors.
//!
//! Drags become `pending`, pending survives pointer-up for resize/move/nudge,
//! and only `commit` writes pixels. Colours freeze at draw time
//! (`pending.color`); size/fill/radius stay live settings.

use pixel_engine::model::Settings;
use pixel_engine::raster::shape_raster::ShapeKind;

use crate::tools::api::ToolCx;

use super::super::geom::{PendingGeom, Pt};
use super::super::pending::{PendingValue, emit_text, pending_json, read_value};

/// Live settings keys — a change re-emits and re-renders when a shape is open.
pub(super) const LIVE_KEYS: [pixel_engine::model::SettingKey; 5] = [
    pixel_engine::model::SettingKey::PenSize,
    pixel_engine::model::SettingKey::ShapeFill,
    pixel_engine::model::SettingKey::ShapeRadius,
    pixel_engine::model::SettingKey::SnapUnit,
    pixel_engine::model::SettingKey::ShapeLockAspect,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Idle,
    Drawing,
    Placing,
    Pending,
    Resizing,
    Moving,
}

#[derive(Debug, Clone)]
pub(crate) struct Draw {
    pub(crate) p0: Pt,
    pub(crate) cur: Option<Pt>,
    pub(crate) layer_id: String,
    pub(crate) shift: bool,
    pub(crate) alt: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum Op {
    Resize {
        handle: String,
        end: Option<u8>,
        orig: PendingShape,
    },
    Move {
        ox: i32,
        oy: i32,
        orig: PendingShape,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Place {
    pub(crate) points: Vec<Pt>,
    pub(crate) layer_id: String,
}

/// A pending shape: captured layer + captured colour + geometry.
#[derive(Debug, Clone)]
pub struct PendingShape {
    pub layer_id: String,
    pub color: String,
    pub geom: PendingGeom,
}

pub struct ShapeTool {
    pub(crate) kind: ShapeKind,
    pub(crate) mode: Mode,
    pub(crate) pending: Option<PendingShape>,
    pub(crate) draw: Option<Draw>,
    pub(crate) op: Option<Op>,
    pub(crate) place: Option<Place>,
    pub(crate) hover_pt: Option<Pt>,
    pub(crate) emitted: Option<String>,
    /// Last-seen settings for the session-less `overlay()` preview.
    pub(crate) live: Settings,
}

impl ShapeTool {
    pub fn new(kind: ShapeKind) -> Self {
        Self {
            kind,
            mode: Mode::Idle,
            pending: None,
            draw: None,
            op: None,
            place: None,
            hover_pt: None,
            emitted: None,
            live: Settings::default(),
        }
    }

    pub fn kind(&self) -> ShapeKind {
        self.kind
    }

    pub(super) fn label(&self) -> &'static str {
        match self.kind {
            ShapeKind::Line => "직선",
            ShapeKind::Rect => "사각형",
            ShapeKind::Rrect => "둥근 사각형",
            ShapeKind::Ellipse => "타원",
            ShapeKind::Polygon => "다각형",
        }
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn is_placing(&self) -> bool {
        self.mode == Mode::Placing
    }

    pub(super) fn zoom(&self, cx: &ToolCx) -> f64 {
        let z = cx.view.zoom();
        if z.is_finite() && z > 0.0 { z } else { 1.0 }
    }

    /// Snapshot settings for `overlay()`, which has no session.
    pub(crate) fn refresh_live(&mut self, cx: &ToolCx) {
        self.live = cx.session.settings();
    }

    /// `_emitChanged` — `JSON.stringify({pending, vertices})` comparison, then
    /// `emitToolState(kind, pending)`. Text is the contract (roadmap W6).
    pub(crate) fn emit_changed(&mut self, cx: &mut ToolCx) {
        let cur = emit_text(self.kind, self.pending.as_ref(), self.vertices());
        if Some(&cur) == self.emitted.as_ref() {
            return;
        }
        self.emitted = Some(cur);
        cx.session
            .emit_tool_state(self.kind.as_str(), pending_json(self.pending.as_ref()));
    }

    fn vertices(&self) -> Option<Vec<Pt>> {
        if self.kind != ShapeKind::Polygon {
            return None;
        }
        let points = self
            .place
            .as_ref()
            .map(|p| p.points.clone())
            .or_else(|| self.pending.as_ref().map(|p| p.geom.points.clone()))?;
        Some(points)
    }

    /// `getPending` for the option bar.
    pub fn pending_value(&self) -> Option<PendingValue> {
        read_value(self.kind, self.pending.as_ref())
    }

    /// `getVertices` for the option bar — cloned polygon points or `None`.
    pub fn vertices_value(&self) -> Option<Vec<Pt>> {
        self.vertices()
    }
}
