//! Shape option block: mode toggle, radius, lock/snap, bbox editing, polygon
//! vertices, commit/cancel.
//!
//! The enable rules are a function of the active tool AND its pending geometry,
//! so they live here rather than in the shell - the JS file did the same.
use super::tools::mid_round_half_up;
use crate::session_glue::AppState;
use crate::tools::api::ToolCx;
use crate::tools::shape::{PendingValue, ShapeTool};
use crate::ui::optionbar::switch::segmented_switch;
use egui::{DragValue, RichText, Ui};

pub fn shape_options(
    ui: &mut Ui,
    juice_clicks: &mut std::collections::HashMap<String, f64>,
    app: &mut AppState,
    tool: &str,
) {
    let is_line = tool == "line";
    let is_polygon = tool == "polygon";
    let settings = app.session.settings();
    let is_filled = settings.shape_fill == "fill";

    ui.label(RichText::new("모드:").size(12.0));
    if segmented_switch(
        ui,
        juice_clicks,
        "shape:mode",
        !is_line,
        "테두리",
        "채움",
        is_filled,
    ) {
        let _ = app.session.set_setting(
            "shapeFill",
            pixel_engine::model::SettingInput::Str(if is_filled { "outline" } else { "fill" }),
        );
    }

    // Stroke width comes from the brush panel's pen size (JS has no width
    // control in the shape block).

    if tool == "rrect" {
        let mut radius = settings.shape_radius;
        ui.label(RichText::new("반경:").size(12.0));
        if ui
            .add(DragValue::new(&mut radius).speed(1.0).suffix(" px"))
            .changed()
        {
            let _ = app.session.set_setting(
                "shapeRadius",
                pixel_engine::model::SettingInput::Num(radius as f64),
            );
        }
    }

    let mut lock_aspect = settings.shape_lock_aspect;
    if ui.checkbox(&mut lock_aspect, "정비율 고정").changed() {
        let _ = app.session.set_setting(
            "shapeLockAspect",
            pixel_engine::model::SettingInput::Bool(lock_aspect),
        );
    }

    let mut snap_unit = settings.snap_unit;
    if ui.checkbox(&mut snap_unit, "단위 스냅").changed() {
        let _ = app.session.set_setting(
            "snapUnit",
            pixel_engine::model::SettingInput::Bool(snap_unit),
        );
    }

    // Pending snapshot: read once, then drive the numeric controls.
    let snap = app
        .tools
        .get(tool)
        .and_then(|t| t.as_any().downcast_ref::<ShapeTool>())
        .map(|s| (s.pending_value(), s.vertices_value(), s.is_placing()));
    let has_pending = snap.as_ref().is_some_and(|(p, _, _)| p.is_some());

    if !is_polygon {
        // Bbox fields. Line pendings are end-pairs, so W/H show x1/y1
        // exactly like JS `fill()`; edits write back the same shape.
        let mut b = match &snap.as_ref().and_then(|(p, _, _)| p.clone()) {
            Some(PendingValue::LineEnds { x0, y0, x1, y1 }) => [*x0, *y0, *x1, *y1],
            Some(PendingValue::Box { x, y, w, h, .. }) => [*x, *y, *w, *h],
            _ => [0, 0, 0, 0],
        };
        let mut edited = false;
        for (i, label) in ["X", "Y", "W", "H"].iter().enumerate() {
            ui.label(RichText::new(*label).size(12.0));
            if ui
                .add_enabled(has_pending, DragValue::new(&mut b[i]).speed(1.0))
                .changed()
            {
                edited = true;
            }
        }
        if edited {
            let value = if is_line {
                PendingValue::LineEnds {
                    x0: b[0],
                    y0: b[1],
                    x1: b[2],
                    y1: b[3],
                }
            } else {
                PendingValue::Box {
                    x: b[0],
                    y: b[1],
                    w: b[2],
                    h: b[3],
                    radius: None,
                }
            };
            set_shape_pending(app, tool, Some(value));
        }
    } else if let Some(points) = snap.as_ref().and_then(|(_, v, _)| v.clone()) {
        // Polygon vertex rows + midpoint append (JS `paintVertices`).
        let placing = snap.as_ref().is_some_and(|(_, _, placing)| *placing);
        let editable = has_pending && !placing;
        let mut points = points;
        let mut edited = false;
        for (i, p) in points.clone().iter().enumerate() {
            ui.label(RichText::new(format!("{}", i + 1)).size(11.0));
            let mut x = p.0;
            let mut y = p.1;
            if ui
                .add_enabled(editable, DragValue::new(&mut x).speed(1.0))
                .changed()
            {
                points[i].0 = x;
                edited = true;
            }
            if ui
                .add_enabled(editable, DragValue::new(&mut y).speed(1.0))
                .changed()
            {
                points[i].1 = y;
                edited = true;
            }
            // JS drops the row only above 3 vertices.
            if ui
                .add_enabled(editable && points.len() > 3, egui::Button::new("−"))
                .on_hover_text(format!("정점 {} 삭제", i + 1))
                .clicked()
            {
                points.remove(i);
                edited = true;
                break;
            }
        }
        if edited {
            set_shape_pending(app, tool, Some(PendingValue::Polygon { points }));
        }
        if ui
            .add_enabled(editable, egui::Button::new("정점 추가"))
            .clicked()
        {
            let mut points = snap
                .as_ref()
                .and_then(|(_, v, _)| v.clone())
                .unwrap_or_default();
            if let (Some(last), Some(first)) = (points.last(), points.first()) {
                // JS `Math.round((last + first) / 2)` — halves toward +∞
                // unlike Rust integer division (toward zero).
                points.push((
                    mid_round_half_up(last.0, first.0),
                    mid_round_half_up(last.1, first.1),
                ));
                set_shape_pending(app, tool, Some(PendingValue::Polygon { points }));
            }
        }
    }

    if crate::ui::juice::button(
        ui,
        juice_clicks,
        "shape:commit",
        has_pending,
        egui::Button::new("확정"),
    )
    .clicked()
    {
        let mut render = false;
        let AppState {
            session,
            tools,
            view_store,
            ..
        } = app;
        let mut cx = ToolCx {
            session,
            view: view_store.view(),
            need_render: &mut render,
        };
        if let Some(shape) = tools
            .get_mut(tool)
            .and_then(|t| t.as_any_mut().downcast_mut::<ShapeTool>())
            && let Err(e) = shape.commit(&mut cx)
        {
            cx.report(e);
        }
    }
    if crate::ui::juice::button(
        ui,
        juice_clicks,
        "shape:cancel",
        has_pending,
        egui::Button::new("취소"),
    )
    .clicked()
    {
        set_shape_pending(app, tool, None);
    }
}

/// `setPending` through a fresh [`ToolCx`], routing [`DrawToolError`] to
/// the session notifier like the JS `safe()` wrapper.
pub fn set_shape_pending(app: &mut AppState, tool: &str, value: Option<PendingValue>) {
    let mut render = false;
    let AppState {
        session,
        tools,
        view_store,
        ..
    } = app;
    let mut cx = ToolCx {
        session,
        view: view_store.view(),
        need_render: &mut render,
    };
    if let Some(shape) = tools
        .get_mut(tool)
        .and_then(|t| t.as_any_mut().downcast_mut::<ShapeTool>())
        && let Err(e) = shape.set_pending(value, &mut cx)
    {
        cx.report(e);
    }
}
