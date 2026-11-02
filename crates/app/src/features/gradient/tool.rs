//! Gradient tool state transitions: options, drag/resize/move, and commit.

use std::cell::RefCell;

use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::model::edit_session::EditOptions;
use pixel_engine::pixel::parse_hex;
use pixel_engine::raster::gradient::{RadialGradient, Stop, radial_gradient_mask};
use pixel_engine::raster::shape_raster::ShapeKind;

use crate::features::shape::{DragFlags, Pt, resize_geom};
use crate::input::ToolEvent;
use crate::tools::api::ToolCx;

use super::state::{Center, EdgeMode, GradientTool, Mode, Op, circle_of};

impl GradientTool {
    pub fn new() -> Self {
        Self {
            inner_radius: 0.0,
            edge_mode: EdgeMode::Transparent,
            center: None,
            pending: None,
            op: None,
            mode: Mode::Idle,
            canvas: None,
            preview: RefCell::new(None),
        }
    }

    pub fn inner_radius(&self) -> f64 {
        self.inner_radius
    }

    /// `panel_gradient_options.js`: integers clamp to 0..2048, the rest is
    /// ignored (no reset to a previous value).
    pub fn set_inner_radius(&mut self, v: f64) -> bool {
        if v.fract() != 0.0 || !v.is_finite() {
            return false;
        }
        let clamped = v.clamp(0.0, 2048.0);
        let changed = self.inner_radius != clamped;
        self.inner_radius = clamped;
        self.preview.borrow_mut().take();
        changed
    }

    pub fn edge_mode(&self) -> EdgeMode {
        self.edge_mode
    }

    pub fn edge_mode_str(&self) -> &'static str {
        self.edge_mode.as_str()
    }

    /// `panel_gradient_options.js`: only `'transparent'|'solid'`.
    pub fn set_edge_mode(&mut self, mode: &str) -> bool {
        match EdgeMode::parse(mode) {
            Some(m) => {
                let changed = self.edge_mode != m;
                self.edge_mode = m;
                self.preview.borrow_mut().take();
                changed
            }
            None => false,
        }
    }

    pub fn is_drawing(&self) -> bool {
        self.center.is_some()
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn drag_radius(center: &Center) -> f64 {
        f64::from(center.x1 - center.x).hypot(f64::from(center.y1 - center.y))
    }

    pub(super) fn up_radius(center: &Center, ev: &ToolEvent) -> f64 {
        f64::from(ev.x - center.x).hypot(f64::from(ev.y - center.y))
    }

    /// The engine gradient for a centre + radius, shared by commit/overlay.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn gradient_for(
        cx: f64,
        cy: f64,
        radius: f64,
        primary: &str,
        secondary: &str,
        inner_radius: f64,
        edge_mode: EdgeMode,
        w: usize,
        h: usize,
    ) -> EngineResult<RadialGradient> {
        let inner = parse_hex(primary)
            .ok_or_else(|| DrawToolError::out_of_range(format!("invalid color {primary}")))?;
        let outer = parse_hex(secondary)
            .ok_or_else(|| DrawToolError::out_of_range(format!("invalid color {secondary}")))?;
        let edge_a = if edge_mode == EdgeMode::Solid {
            f64::from(outer[3])
        } else {
            0.0
        };
        Ok(RadialGradient {
            cx,
            cy,
            r0: inner_radius.min(radius),
            r1: radius,
            stops: vec![
                Stop::new(
                    0.0,
                    f64::from(inner[0]),
                    f64::from(inner[1]),
                    f64::from(inner[2]),
                    f64::from(inner[3]),
                ),
                Stop::new(
                    1.0,
                    f64::from(outer[0]),
                    f64::from(outer[1]),
                    f64::from(outer[2]),
                    edge_a,
                ),
            ],
            w,
            h,
        })
    }

    pub(super) fn flags(ev: &ToolEvent, cx: &ToolCx) -> DragFlags {
        let s = cx.session.settings();
        DragFlags {
            shift: ev.shift,
            lock: ev.shift || s.shape_lock_aspect,
            center: ev.alt,
            snap: s.snap_unit,
        }
    }

    pub(super) fn resize_to(&mut self, p: Pt, flags: DragFlags, shift: bool, cx: &mut ToolCx) {
        let Some(Op::Resize { handle, orig }) = self.op.as_ref() else {
            return;
        };
        let geom = resize_geom(ShapeKind::Rect, &orig.geom, handle, None, p, flags, shift);
        if let Some(pending) = self.pending.as_mut() {
            pending.geom = geom;
        }
        self.preview.borrow_mut().take();
        cx.request_render();
    }

    /// Discard everything (document replacement, Delete, tool reset).
    pub fn discard(&mut self, cx: &mut ToolCx) {
        self.center = None;
        self.pending = None;
        self.op = None;
        self.mode = Mode::Idle;
        self.preview.borrow_mut().take();
        cx.request_render();
    }

    pub(super) fn cancel_operation(&mut self, cx: &mut ToolCx) {
        match self.mode {
            Mode::Drawing => {
                self.center = None;
                self.mode = if self.pending.is_some() {
                    Mode::Pending
                } else {
                    Mode::Idle
                };
            }
            Mode::Resizing | Mode::Moving => {
                if let Some(op) = self.op.take() {
                    let orig = match op {
                        Op::Resize { orig, .. } | Op::Move { orig, .. } => orig,
                    };
                    self.pending = Some(orig);
                    self.mode = Mode::Pending;
                }
            }
            _ => {}
        }
        self.preview.borrow_mut().take();
        cx.request_render();
    }

    /// Bake the pending gradient as one history entry.
    pub fn commit(&mut self, cx: &mut ToolCx) -> EngineResult<bool> {
        let Some(pending) = self.pending.clone() else {
            self.mode = Mode::Idle;
            return Ok(false);
        };
        let Some((cx_i, cy_i, radius)) = circle_of(&pending.geom) else {
            self.discard(cx);
            return Ok(false);
        };
        if radius < 1.0 {
            self.discard(cx);
            return Ok(false);
        }
        let Some((cw, ch)) = self.canvas.or_else(|| {
            cx.session
                .doc()
                .map(|d| (d.canvas.width_px, d.canvas.height_px))
        }) else {
            return Ok(false);
        };
        let (w, h) = (cw as usize, ch as usize);
        let label = "방사형 그라디언트";
        let gradient = Self::gradient_for(
            f64::from(cx_i),
            f64::from(cy_i),
            radius,
            &pending.primary,
            &pending.secondary,
            self.inner_radius,
            self.edge_mode,
            w,
            h,
        )?;
        let mask = radial_gradient_mask(&gradient)?;
        if let Err(e) = cx
            .session
            .stroke_begin(EditOptions::on_layer(&pending.layer_id).labeled(label))
        {
            if matches!(e.code, "LAYER_LOCKED" | "LAYER_HIDDEN" | "LAYER_NOT_FOUND") {
                self.discard(cx);
                return Ok(false);
            }
            return Err(e);
        }
        let result = cx.session.stroke_with(|wr| {
            for (i, p) in mask.data.iter().enumerate() {
                if *p != 0 {
                    wr.set((i % w) as u32, (i / w) as u32, *p);
                }
            }
        });
        if let Err(e) = result {
            cx.session.stroke_cancel();
            return Err(e);
        }
        cx.session.stroke_commit(Some(label))?;
        self.pending = None;
        self.op = None;
        self.mode = Mode::Idle;
        self.preview.borrow_mut().take();
        cx.request_render();
        Ok(true)
    }
}

impl Default for GradientTool {
    fn default() -> Self {
        Self::new()
    }
}
