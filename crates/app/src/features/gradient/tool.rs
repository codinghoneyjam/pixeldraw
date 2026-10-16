//! The radial gradient tool: drag state, option setters, [`Tool`] impl.
//!
//! `inner_radius`/`edge_mode` are tool-local options (the Phase 3 option bar
//! owns them through the setters below), while the colours and centre freeze
//! at `pointer_down` like the JS `_center` snapshot.

use std::cell::RefCell;

use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::model::edit_session::EditOptions;
use pixel_engine::pixel::parse_hex;
use pixel_engine::raster::gradient::{RadialGradient, Stop, radial_gradient_mask};
use pixel_engine::view::canvas_to_screen;

use crate::input::{Button, ToolEvent};
use crate::tools::api::{CursorKind, OverlayInfo, Tool, ToolCx};
use crate::tools::overlay::OverlayOp;

use super::preview::blit_preview;

/// Gradient edge falloff — `edgeMode` in the JS (`'transparent'|'solid'`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeMode {
    Transparent,
    Solid,
}

impl EdgeMode {
    fn parse(s: &str) -> Option<Self> {
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

/// Frozen `pointer_down` state, exactly the JS `_center` fields.
#[derive(Debug, Clone)]
struct Center {
    x: i32,
    y: i32,
    x1: i32,
    y1: i32,
    layer_id: String,
    primary: String,
    secondary: String,
    inner_radius: f64,
    edge_mode: EdgeMode,
}

#[derive(Debug, Clone, PartialEq)]
struct PreviewKey {
    x: i32,
    y: i32,
    x1: i32,
    y1: i32,
    inner_radius: u64,
    edge_solid: bool,
    primary: String,
    secondary: String,
}

pub struct GradientTool {
    inner_radius: f64,
    edge_mode: EdgeMode,
    center: Option<Center>,
    /// Last-seen canvas size — `overlay()` has no session, like
    /// `ShapeTool.live`.
    canvas: Option<(u32, u32)>,
    /// Mask-keyed preview: `radial_gradient_mask` is canvas-sized.
    preview: RefCell<Option<(PreviewKey, OverlayOp)>>,
}

impl GradientTool {
    pub fn new() -> Self {
        Self {
            inner_radius: 0.0,
            edge_mode: EdgeMode::Transparent,
            center: None,
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
                changed
            }
            None => false,
        }
    }

    pub fn is_drawing(&self) -> bool {
        self.center.is_some()
    }

    fn drag_radius(center: &Center) -> f64 {
        f64::from(center.x1 - center.x).hypot(f64::from(center.y1 - center.y))
    }

    fn up_radius(center: &Center, ev: &ToolEvent) -> f64 {
        f64::from(ev.x - center.x).hypot(f64::from(ev.y - center.y))
    }

    /// The engine gradient for a centre + radius, shared by commit/overlay.
    fn gradient_for(
        center: &Center,
        radius: f64,
        w: usize,
        h: usize,
    ) -> EngineResult<RadialGradient> {
        let inner = parse_hex(&center.primary).ok_or_else(|| {
            DrawToolError::out_of_range(format!("invalid color {}", center.primary))
        })?;
        let outer = parse_hex(&center.secondary).ok_or_else(|| {
            DrawToolError::out_of_range(format!("invalid color {}", center.secondary))
        })?;
        let edge_a = if center.edge_mode == EdgeMode::Solid {
            f64::from(outer[3])
        } else {
            0.0
        };
        Ok(RadialGradient {
            cx: f64::from(center.x),
            cy: f64::from(center.y),
            r0: center.inner_radius.min(radius),
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
}

impl Default for GradientTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for GradientTool {
    fn gradient_tool_mut(&mut self) -> Option<&mut crate::features::gradient::GradientTool> {
        Some(self)
    }

    fn id(&self) -> &'static str {
        "radial_gradient"
    }

    fn cursor(&self) -> CursorKind {
        CursorKind::Crosshair
    }

    fn pointer_down(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        if ev.button != Button::Left {
            return;
        }
        let Some(doc) = cx.session.doc() else {
            return;
        };
        let settings = cx.session.settings();
        self.center = Some(Center {
            x: ev.x,
            y: ev.y,
            x1: ev.x,
            y1: ev.y,
            layer_id: doc.active_layer_id.clone(),
            primary: settings.primary_color,
            secondary: settings.secondary_color,
            inner_radius: self.inner_radius,
            edge_mode: self.edge_mode,
        });
        self.canvas = Some((doc.canvas.width_px, doc.canvas.height_px));
        cx.request_render();
    }

    fn pointer_move(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        let Some(center) = self.center.as_mut() else {
            return;
        };
        center.x1 = ev.x;
        center.y1 = ev.y;
        cx.request_render();
    }

    fn pointer_up(&mut self, ev: &ToolEvent, cx: &mut ToolCx) {
        let Some(center) = self.center.take() else {
            return;
        };
        self.preview.borrow_mut().take();
        cx.request_render();
        let radius = Self::up_radius(&center, ev);
        if radius < 1.0 {
            return;
        }
        let Some(doc) = cx.session.doc() else {
            return;
        };
        let (w, h) = (doc.canvas.width_px as usize, doc.canvas.height_px as usize);
        let layer_id = center.layer_id.clone();
        let gradient = match Self::gradient_for(&center, radius, w, h) {
            Ok(g) => g,
            Err(e) => {
                cx.report(e);
                return;
            }
        };
        let mask = match radial_gradient_mask(&gradient) {
            Ok(m) => m,
            Err(e) => {
                cx.report(e);
                return;
            }
        };
        if cx
            .session
            .stroke_begin(EditOptions::on_layer(&layer_id).labeled("방사형 그라디언트"))
            .is_err()
        {
            // Locked/hidden/gone: the engine already emitted the status, and
            // the JS threw here instead of painting.
            return;
        }
        let _ = cx.session.stroke_with(|wr| {
            for (i, p) in mask.data.iter().enumerate() {
                if *p != 0 {
                    wr.set((i % w) as u32, (i / w) as u32, *p);
                }
            }
        });
        // Commit failure (layer deleted mid-drag) must not break input.
        let _ = cx.session.stroke_commit(Some("방사형 그라디언트"));
    }

    fn cancel(&mut self, cx: &mut ToolCx) {
        self.center = None;
        self.preview.borrow_mut().take();
        cx.request_render();
    }

    fn deactivate(&mut self, cx: &mut ToolCx) {
        self.cancel(cx);
    }

    fn overlay(&self, info: &OverlayInfo, out: &mut Vec<OverlayOp>) {
        let Some(center) = self.center.as_ref() else {
            return;
        };
        let radius = Self::drag_radius(center);
        if radius < 1.0 {
            return;
        }
        let Some((cw, ch)) = self.canvas else {
            return;
        };
        let key = PreviewKey {
            x: center.x,
            y: center.y,
            x1: center.x1,
            y1: center.y1,
            inner_radius: center.inner_radius.to_bits(),
            edge_solid: center.edge_mode == EdgeMode::Solid,
            primary: center.primary.clone(),
            secondary: center.secondary.clone(),
        };
        let cached = self
            .preview
            .borrow()
            .as_ref()
            .and_then(|(old_key, op)| (*old_key == key).then(|| op.clone()));
        if let Some(op) = cached {
            out.push(op);
        } else {
            let op = Self::gradient_for(center, radius, cw as usize, ch as usize)
                .ok()
                .and_then(|g| radial_gradient_mask(&g).ok())
                .and_then(|m| blit_preview(m.w, m.h, &m.data));
            if let Some(op) = op {
                out.push(op.clone());
                *self.preview.borrow_mut() = Some((key, op));
            }
        }
        // The dashed radius ring: `setLineDash([4,3])` verbatim (no dpr
        // factor) and `rgba(255,255,255,0.9)` ≈ 230, like the JS.
        let dev = canvas_to_screen(info.view, f64::from(center.x), f64::from(center.y));
        out.push(OverlayOp::Circle {
            cx: dev.x * info.dpr,
            cy: dev.y * info.dpr,
            r: radius * info.view.zoom() * info.dpr,
            rgba: [255, 255, 255, 230],
            width: 1.0,
            dash: Some((4.0, 3.0)),
        });
    }
}
