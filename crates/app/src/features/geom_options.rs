//! Shared X/Y/W/H + commit/cancel block for the pending-geometry tools.
//!
//! Arc, gradient and text all expose their pending box through
//! [`Tool::pending_box`](crate::tools::api::Tool::pending_box), so the option
//! bar can edit them with one set of controls in one order. Shapes keep their
//! richer line/polygon block but use the same action labels.

use crate::session_glue::AppState;
use crate::tools::api::ToolCx;
use egui::{DragValue, RichText, Ui};
use std::ops::RangeInclusive;

/// Geometry fields are intentionally limited to four digits so a typo cannot
/// create an absurd pending box. Negative coordinates stay legal: a shape may
/// start off-canvas and clip in on commit.
pub const GEOM_MIN: i32 = -9999;
pub const GEOM_MAX: i32 = 9999;

/// Wide enough for a sign and four digits in both drag and edit mode. egui's
/// `DragValue` sizes its text editor from `interact_size.x`, so setting that is
/// what makes the whole number visible while typing.
const GEOM_FIELD_WIDTH: f32 = 64.0;

/// Fixed label width keeps the X/Y/W/H group the same size no matter which
/// tool is active, so the centred position does not jitter between tools.
const GEOM_LABEL_WIDTH: f32 = 12.0;

/// The option bar sets 10px item spacing. Four label/field pairs therefore
/// occupy a deterministic 374px, which is the width used for centring.
const GEOM_GROUP_WIDTH: f32 = 4.0 * (GEOM_LABEL_WIDTH + GEOM_FIELD_WIDTH) + 7.0 * 10.0;

/// Never shove the geometry group into the tool-specific controls on a narrow
/// window; the horizontal scroll area can still reach it.
const GEOM_MIN_LEADING_GAP: f32 = 12.0;

/// Which geometry field currently owns keyboard focus. Kept in egui data so
/// the confirm button can apply a pending text edit before committing.
const GEOM_FOCUS_KEY: &str = "pixeldraw.geom.focus";

fn deferred_commit_key(tool_id: &str) -> egui::Id {
    egui::Id::new(("pixeldraw.geom.deferred_commit", tool_id))
}

pub fn coord_range() -> RangeInclusive<i32> {
    GEOM_MIN..=GEOM_MAX
}

pub fn extent_range() -> RangeInclusive<i32> {
    1..=GEOM_MAX
}

/// Clamp a position typed into X/Y (or a line endpoint).
pub fn clamp_coord(value: i32) -> i32 {
    value.clamp(GEOM_MIN, GEOM_MAX)
}

/// Clamp a box extent: W/H are always at least one pixel.
pub fn clamp_extent(value: i32) -> i32 {
    value.clamp(1, GEOM_MAX)
}

/// One labelled numeric field: four-digit range, integer-only display, and a
/// fixed width so the actual value is never clipped to `0`/`1`.
///
/// `clamp_existing_to_range(false)` keeps a pending shape's real geometry on
/// screen; only user edits are clamped. That matters for boxes that legitimately
/// extend outside the canvas.
pub fn geometry_field(
    ui: &mut Ui,
    label: &str,
    value: &mut i32,
    range: RangeInclusive<i32>,
    enabled: bool,
) -> bool {
    ui.add_sized(
        [GEOM_LABEL_WIDTH, ui.spacing().interact_size.y],
        egui::Label::new(RichText::new(label).size(12.0)),
    );
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().interact_size.x = GEOM_FIELD_WIDTH;
            ui.add_enabled(
                enabled,
                DragValue::new(value)
                    .range(range)
                    .speed(1.0)
                    .max_decimals(0)
                    // Typed values are applied once, on Enter/focus loss,
                    // instead of rebuilding the pending preview per keystroke.
                    // Dragging still updates live.
                    .update_while_editing(false)
                    .clamp_existing_to_range(false),
            )
        })
        .inner;
    if response.has_focus() {
        ui.data_mut(|data| data.insert_temp(egui::Id::new(GEOM_FOCUS_KEY), response.id));
    }
    response.changed()
}

fn take_deferred_commit(ui: &mut Ui, tool_id: &str) -> bool {
    ui.data_mut(|data| {
        data.remove_temp::<bool>(deferred_commit_key(tool_id))
            .unwrap_or(false)
    })
}

fn commit_is_ready(ui: &mut Ui, tool_id: &str) -> bool {
    let focused = ui.data(|data| data.get_temp::<egui::Id>(egui::Id::new(GEOM_FOCUS_KEY)));
    if let Some(id) = focused
        && ui.memory(|memory| memory.has_focus(id))
    {
        // The click moved focus away from the field, but DragValue only writes
        // its text value on the *next* frame. Defer the commit by one frame so
        // the freshly typed number is part of it.
        ui.memory_mut(|memory| memory.surrender_focus(id));
        ui.data_mut(|data| data.insert_temp(deferred_commit_key(tool_id), true));
        ui.ctx().request_repaint();
        return false;
    }
    true
}

/// Whether a confirm-button press should run now. A press while a geometry
/// text field is focused is deferred one frame; the returned value on that
/// frame is `false`, and the next frame returns `true` automatically.
pub fn confirm_commit_now(ui: &mut Ui, tool_id: &str, clicked: bool) -> bool {
    let deferred = take_deferred_commit(ui, tool_id);
    if clicked {
        deferred || commit_is_ready(ui, tool_id)
    } else {
        deferred
    }
}

/// How much space to insert before the geometry group so it lands on the
/// bar's centre line. Pure for testability; the caller adds the result with
/// `ui.add_space`.
pub fn geometry_group_lead(bar_center_x: f32, current_left_x: f32, group_width: f32) -> f32 {
    let target_left = bar_center_x - group_width * 0.5;
    (target_left - current_left_x).max(GEOM_MIN_LEADING_GAP)
}

/// Centre the shared X/Y/W/H block in the option bar. The bar's `max_rect`
/// spans the visible viewport, so this is independent of how wide the
/// tool-specific controls on the left happen to be.
pub fn center_geometry_group(ui: &mut Ui) {
    let lead = geometry_group_lead(
        ui.max_rect().center().x,
        ui.cursor().min.x,
        GEOM_GROUP_WIDTH,
    );
    ui.add_space(lead);
}

/// Render the numeric box and its confirm/cancel pair.
///
/// The fields are always enabled: when no pending exists, editing one creates
/// a pending box at the field values. `w`/`h` are clamped to at least one so a
/// zero-sized pending can never be committed.
pub fn geometry_and_actions(
    ui: &mut Ui,
    juice_clicks: &mut std::collections::HashMap<String, f64>,
    app: &mut AppState,
    tool_id: &str,
) {
    let (snap, resizable) = app
        .tools
        .get(tool_id)
        .map(|tool| (tool.pending_box(), tool.pending_box_resizable()))
        .unwrap_or((None, true));

    center_geometry_group(ui);
    let mut b = snap.unwrap_or([0, 0, 1, 1]);
    let mut edited = false;
    for (i, label) in ["X", "Y", "W", "H"].iter().enumerate() {
        let enabled = i < 2 || resizable;
        let range = if i < 2 { coord_range() } else { extent_range() };
        if geometry_field(ui, label, &mut b[i], range, enabled) {
            edited = true;
        }
    }
    if edited {
        b[0] = clamp_coord(b[0]);
        b[1] = clamp_coord(b[1]);
        b[2] = clamp_extent(b[2]);
        b[3] = clamp_extent(b[3]);
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
        if let Some(tool) = tools.get_mut(tool_id)
            && let Err(e) = tool.set_pending_box(b, &mut cx)
        {
            cx.report(e);
        }
    }
    // The first edit can create the pending box above, so re-read this before
    // deciding whether the confirm button is enabled.
    let has_pending = app
        .tools
        .get(tool_id)
        .is_some_and(|tool| tool.has_pending());

    let commit_clicked = crate::ui::juice::button(
        ui,
        juice_clicks,
        &format!("{tool_id}:geometry:commit"),
        has_pending,
        egui::Button::new("확정"),
    )
    .clicked();
    if confirm_commit_now(ui, tool_id, commit_clicked) {
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
        if let Some(tool) = tools.get_mut(tool_id)
            && let Err(e) = tool.commit_pending(&mut cx)
        {
            cx.report(e);
        }
    }
    if crate::ui::juice::button(
        ui,
        juice_clicks,
        &format!("{tool_id}:geometry:cancel"),
        has_pending,
        egui::Button::new("취소"),
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
        if let Some(tool) = tools.get_mut(tool_id) {
            tool.discard_pending(&mut cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_values_stay_within_four_digits() {
        assert_eq!(clamp_coord(12_345), 9_999);
        assert_eq!(clamp_coord(-12_345), -9_999);
        assert_eq!(clamp_coord(0), 0);
        assert_eq!(clamp_coord(-9_999), -9_999);
    }

    #[test]
    fn extents_never_collapse_to_zero() {
        assert_eq!(clamp_extent(0), 1);
        assert_eq!(clamp_extent(-40), 1);
        assert_eq!(clamp_extent(12_345), 9_999);
        assert_eq!(clamp_extent(9_999), 9_999);
    }

    #[test]
    fn geometry_group_centres_and_keeps_a_minimum_gap() {
        // 500px bar centre, group 374px wide -> group starts at 313.
        assert_eq!(geometry_group_lead(500.0, 100.0, 374.0), 213.0);
        // Once the tool controls reach the centre, stop overlapping them.
        assert_eq!(geometry_group_lead(500.0, 400.0, 374.0), 12.0);
    }

    #[test]
    fn geometry_group_width_matches_the_laid_out_fields() {
        // 4 * (12 label + 64 field) + 7 * 10 spacing.
        assert_eq!(GEOM_GROUP_WIDTH, 374.0);
    }
}
