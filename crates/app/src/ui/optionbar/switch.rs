//! The single-button segmented switch (?뚮몢由?| 梨꾩? as one pill).
//!
//! Also home to the fixed slot geometry, because the pill IS the one control
//! whose size may never change: the bar being a stable pair of cells is what
//! keeps it from feeling like a deck of swapped cards per tool.
use egui::{Color32, Ui, Vec2};

/// Fixed-width right module slot: the viewport quick actions (grid / 100% /
/// fit). Constant, so a wide tool block can never push it sideways.
pub(crate) const VIEWPORT_SLOT_W: f32 = 240.0;
/// Floor for the tool module slot. A narrower tool block still reserves this
/// much, so the viewport module stays where the eye last left it.
pub(crate) const TOOL_SLOT_MIN_W: f32 = 320.0;
/// Gap between the two modules.
pub(crate) const MODULE_GAP: f32 = 10.0;
/// Height every module slot reserves. Together with the two widths this makes
/// the bar a fixed row of two cells instead of a reflowing strip.
pub(crate) const MODULE_H: f32 = 30.0;
/// The segmented switch's fixed footprint; `MODULE_H` must stay >= this so the
/// pill always fits its slot whole.
pub(crate) const SEGMENTED_W: f32 = 150.0;
pub(crate) const SEGMENTED_H: f32 = 22.0;

/// The tool module's fixed-width slot.
///
/// The slot is allocated in the parent by exact size, so it reserves its width
/// whatever the tool draws. The content goes into a scroll area that shrinks to
/// its content (egui's default `auto_shrink`), which is what keeps the bar a
/// fixed row of two cells: a narrow block leaves the tail of the slot blank
/// instead of stretching, and an over-wide block (polygon vertices) scrolls
/// inside the slot instead of pushing module 2 sideways.
///
/// The slot ui inherits the parent's `left_to_right` layout, so the tool block
/// lays out as ONE row, centred vertically - exactly as it did when it lived
/// directly inside the bar. (`vertical_centered` must NOT be used here: it
/// switches the layout to top_down and stacks every control on its own line.)
pub(crate) fn tool_slot(ui: &mut Ui, rect: egui::Rect, add: impl FnOnce(&mut Ui)) -> f32 {
    let mut slot = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("optionbar_tool")
            .max_rect(rect)
            .sense(egui::Sense::empty()),
    );
    let mut content_w = 0.0;
    egui::ScrollArea::horizontal()
        .id_salt("optionbar_tool_scroll")
        .show(&mut slot, |slot| {
            slot.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            add(slot);
            content_w = slot.min_rect().width();
        });
    content_w
}
/// Single-button segmented switch: one pill, click toggles (JS
/// `.dt-option-switch` — 테두리|채움 in one control).
///
/// The active half is marked **three** ways, only one of which is colour:
///   1. a solid thumb chip that physically covers that half,
///   2. a 2px underline bar under the label,
///   3. the always-visible centre divider that splits the pill in two.
///
/// A colour wash alone was unreadable at a glance (and unreadable at all for a
/// red/green colour-blind user, which is exactly where this control lives), so
/// the state is carried by geometry first.
///
/// Size is fixed so swapping tools never reflows the bar.
pub(crate) fn segmented_switch(
    ui: &mut Ui,
    clicks: &mut std::collections::HashMap<String, f64>,
    key: &str,
    enabled: bool,
    left: &str,
    right: &str,
    is_right: bool,
) -> bool {
    use crate::ui::juice::{click_pop, hover_fade, lerp_color};
    let size = egui::vec2(SEGMENTED_W, SEGMENTED_H);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let hid = ui.id().with("sw").with(key);
    let f = hover_fade(ui.ctx(), hid, enabled && resp.hovered());
    let pop = click_pop(clicks, ui.ctx(), key, enabled && resp.clicked());
    let rect = rect.shrink((1.0 - pop) * 2.0);
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        5.0,
        lerp_color(
            Color32::from_rgb(0x23, 0x24, 0x28),
            Color32::from_rgb(0x35, 0x37, 0x3c),
            f,
        ),
    );
    painter.rect_stroke(
        rect,
        5.0,
        egui::Stroke::new(1.0, Color32::from_rgb(0x3a, 0x3d, 0x43)),
        egui::StrokeKind::Inside,
    );
    // Centre divider: the clear internal split, drawn whatever the state.
    painter.line_segment(
        [
            rect.center_top() + egui::vec2(0.0, 4.0),
            rect.center_bottom() + egui::vec2(0.0, -4.0),
        ],
        egui::Stroke::new(1.0, Color32::from_rgb(0x4c, 0x51, 0x59)),
    );
    for (label, half_right) in [(left, false), (right, true)] {
        let active = is_right == half_right;
        let center = if half_right {
            rect.right_center() + egui::vec2(-rect.width() / 4.0, 0.0)
        } else {
            rect.left_center() + egui::vec2(rect.width() / 4.0, 0.0)
        };
        if active {
            // 1. Thumb: a solid chip covering the whole active half.
            let thumb = egui::Rect::from_center_size(
                center,
                egui::vec2(rect.width() / 2.0 - 4.0, rect.height() - 6.0),
            );
            painter.rect_filled(
                thumb,
                3.0,
                if enabled {
                    Color32::from_rgb(0x1e, 0x6f, 0xa8)
                } else {
                    Color32::from_rgb(0x2d, 0x30, 0x36)
                },
            );
            painter.rect_stroke(
                thumb,
                3.0,
                egui::Stroke::new(
                    1.0,
                    if enabled {
                        Color32::from_rgb(0x4c, 0xc2, 0xff)
                    } else {
                        Color32::from_rgb(0x3a, 0x3d, 0x43)
                    },
                ),
                egui::StrokeKind::Inside,
            );
            // 2. Underline marker under the label, so the state survives even
            //    where the chip's blue and the track's grey are hard to tell.
            let text_w = label.chars().count() as f32 * 6.5;
            painter.line_segment(
                [
                    center + egui::vec2(-text_w / 2.0, 6.0),
                    center + egui::vec2(text_w / 2.0, 6.0),
                ],
                egui::Stroke::new(2.0, Color32::WHITE),
            );
        }
        painter.text(
            center + egui::vec2(0.0, -1.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(11.0),
            if !enabled {
                Color32::from_rgb(0x5a, 0x5e, 0x65)
            } else if active {
                Color32::WHITE
            } else {
                Color32::from_rgb(0x9a, 0x9e, 0xa5)
            },
        );
    }
    resp.clone()
        .on_hover_text(format!("{left} / {right}: 클릭하면 바뀝니다"));
    enabled && resp.clicked()
}

/// The two module slots for a bar of width `bar_w`: (tool width, viewport width).
/// Pure on purpose - it is the invariant the layout test can pin without a UI.
#[must_use]
pub fn module_slots(bar_w: f32) -> (f32, f32) {
    let view_w = VIEWPORT_SLOT_W.min((bar_w * 0.45).max(140.0));
    let tool_w = (bar_w - view_w - MODULE_GAP).max(TOOL_SLOT_MIN_W);
    (tool_w, view_w)
}
