fn mid_round_half_up(a: i32, b: i32) -> i32 {
    ((a as i64 + b as i64) as f64 / 2.0 + 0.5).floor() as i32
}

// Shape option block: mode toggle, radius, lock/snap, bbox editing, polygon
// vertices, commit/cancel.
//
// The enable rules are a function of the active tool AND its pending geometry,
// so they live here rather than in the shell - the JS file did the same.
use crate::features::geom_options::{
    center_geometry_group, clamp_coord, clamp_extent, confirm_commit_now, coord_range,
    extent_range, geometry_field,
};
use crate::features::shape::PendingValue;
use crate::session_glue::AppState;
use crate::tools::api::ToolCx;
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

    if ui
        .button(if is_filled { "윤곽선" } else { "채우기" })
        .clicked()
    {
        let _ = app.session.set_setting(
            "shapeFill",
            pixel_engine::model::SettingInput::Str(if is_filled { "outline" } else { "fill" }),
        );
    }
    // Stroke width comes from the brush panel's pen size (JS has no width
    // control in the shape block).

    if tool == "rrect" {
        let mut radius = settings.shape_radius;
        ui.label(RichText::new("반경").size(12.0));
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
    if ui.checkbox(&mut lock_aspect, "비율 고정").changed() {
        let _ = app.session.set_setting(
            "shapeLockAspect",
            pixel_engine::model::SettingInput::Bool(lock_aspect),
        );
    }

    let mut snap_unit = settings.snap_unit;
    if ui.checkbox(&mut snap_unit, "32px 스냅").changed() {
        let _ = app.session.set_setting(
            "snapUnit",
            pixel_engine::model::SettingInput::Bool(snap_unit),
        );
    }

    // Pending snapshot: read once, then drive the numeric controls.
    let snap = app
        .tools
        .get_mut(tool)
        .and_then(|t| t.shape_tool_mut())
        .map(|s| (s.pending_value(), s.vertices_value(), s.is_placing()));
    let has_pending = snap.as_ref().is_some_and(|(p, _, _)| p.is_some());

    if !is_polygon {
        center_geometry_group(ui);
        // Bbox fields. Line pendings are end-pairs, so W/H show x1/y1
        // exactly like JS `fill()`; edits write back the same shape.
        let mut b = match snap.as_ref().and_then(|(p, _, _)| p.clone()) {
            Some(PendingValue::LineEnds { x0, y0, x1, y1 }) => [x0, y0, x1, y1],
            Some(PendingValue::Box { x, y, w, h, .. }) => [x, y, w, h],
            _ if is_line => [0, 0, 0, 0],
            _ => [0, 0, 1, 1],
        };
        let mut edited = false;
        for (i, label) in ["X", "Y", "W", "H"].iter().enumerate() {
            // A line spells its fields `x0 y0 x1 y1`, so every field is a
            // coordinate. Boxes use W/H as extents, which can never be zero.
            let range = if is_line || i < 2 {
                coord_range()
            } else {
                extent_range()
            };
            if geometry_field(ui, label, &mut b[i], range, true) {
                edited = true;
            }
        }
        if edited {
            b[0] = clamp_coord(b[0]);
            b[1] = clamp_coord(b[1]);
            if is_line {
                b[2] = clamp_coord(b[2]);
                b[3] = clamp_coord(b[3]);
            } else {
                b[2] = clamp_extent(b[2]);
                b[3] = clamp_extent(b[3]);
            }
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
            let mut x = clamp_coord(p.0);
            let mut y = clamp_coord(p.1);
            if geometry_field(ui, "X", &mut x, coord_range(), editable) {
                points[i].0 = x;
                edited = true;
            }
            if geometry_field(ui, "Y", &mut y, coord_range(), editable) {
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
                // JS `Math.round((last + first) / 2)` rounds halves toward +∞,
                // unlike Rust integer division (toward zero).
                points.push((
                    mid_round_half_up(last.0, first.0),
                    mid_round_half_up(last.1, first.1),
                ));
                set_shape_pending(app, tool, Some(PendingValue::Polygon { points }));
            }
        }
    }

    // A first edit can create the pending shape above; keep confirm enabled
    // on that same frame.
    let has_pending = app.tools.get(tool).is_some_and(|t| t.has_pending());
    let commit_clicked = crate::ui::juice::button(
        ui,
        juice_clicks,
        "shape:commit",
        has_pending,
        egui::Button::new("확정"),
    )
    .clicked();
    if confirm_commit_now(ui, tool, commit_clicked) {
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
        if let Some(shape) = tools.get_mut(tool).and_then(|t| t.shape_tool_mut())
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
    if let Some(shape) = tools.get_mut(tool).and_then(|t| t.shape_tool_mut())
        && let Err(e) = shape.set_pending(value, &mut cx)
    {
        cx.report(e);
    }
}
