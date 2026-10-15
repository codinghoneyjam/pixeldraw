//! Panel chrome and the swatch primitives every panel section shares.
//!
//! Split out when `panels.rs` passed 1400 lines. The section card + header,
//! the collapsed-state-owning `PanelsState`, and the two cell painters are
//! used by three of the four left-panel sections, so they are the layer those
//! sections sit on. Everything here is `pub(crate)`: private to the facade.
use egui::{Color32, RichText, Ui};

#[derive(Default)]
pub struct PanelsState {
    /// Which slot palette/recent clicks write to. false = primary, true = secondary.
    pub secondary_active: bool,
    pub recent: Vec<String>,
    pub rename_buffer: String,
    pub renaming: Option<String>,
    /// Collapsed section ids (`color`, `palette`, `recent`, `brush`, `layers`).
    /// Mirrors JS `collapsedPanels` (`collapsible_sections.js`); in-memory for
    /// now — settings-file persistence is a Phase 3.9 follow-up.
    pub collapsed: std::collections::HashSet<String>,
}

/// JS `.dt-panel-head`: full-width header button, ▼/▶ marker, click toggles.
/// Returns true when the body should draw.
pub(crate) fn section_header(ui: &mut Ui, panels: &mut PanelsState, id: &str, title: &str) -> bool {
    let arrow = if panels.collapsed.contains(id) {
        "▶"
    } else {
        "▼"
    };
    let resp = ui.add_sized(
        [ui.available_width(), 22.0],
        egui::Button::new(RichText::new(format!("{arrow} {title}")).strong()).frame(false),
    );
    // Headers are stock buttons: hover glow only (no press scale on text).
    crate::ui::juice::hover_glow(ui, ui.id().with("sec").with(id), &resp, 4.0);
    if resp.clicked() {
        if !panels.collapsed.remove(id) {
            panels.collapsed.insert(id.to_string());
        }
        // JS `persistCollapsed`: the settings file keeps every other field.
        // Only on toggle — this header runs every frame.
        pixel_platform::save_collapsed_panel(id, panels.collapsed.contains(id));
    }
    !panels.collapsed.contains(id)
}

/// JS `#dt-palette, #dt-recent` (`style_base.css`): 8 columns, 2px gap, cells
/// stretch to fill (`width: 100%; aspect-ratio: 1`). Fixed-size cells leave a
/// trailing gutter and read as "spaced out", so the cell is derived from the
/// available width instead.
pub fn swatch_cell_width(available: f32) -> f32 {
    ((available - 7.0 * 2.0) / 8.0).floor().max(12.0)
}

pub(crate) fn swatch_cell(ui: &Ui) -> f32 {
    swatch_cell_width(ui.available_width())
}

/// rrect colour slot: generous corner, 1px edge, hover fade + click pop, and
/// — like JS `.dt-swatch[data-active="true"]` — corner-bracket arms in the
/// slot's color around a matching swatch, plus a fading white hover ring.
pub(crate) fn paint_swatch(
    ui: &mut Ui,
    clicks: &mut std::collections::HashMap<String, f64>,
    key: &str,
    c: Color32,
    cell: f32,
    mark: SlotMark,
) -> egui::Response {
    use crate::ui::juice::{click_pop, hover_fade, lerp_color};
    let radius = slot_radius(cell);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(cell, cell), egui::Sense::click());
    let hid = ui.id().with("sw").with(key);
    let f = hover_fade(ui.ctx(), hid, resp.hovered());
    let pop = click_pop(clicks, ui.ctx(), key, resp.clicked());
    let rect = rect.shrink((1.0 - pop) * cell * 0.2);
    let painter = ui.painter();
    // Hover lifts the fill slightly; the pop scale does the press.
    painter.rect_filled(rect, radius, lerp_color(c, Color32::WHITE, f * 0.12));
    painter.rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, EDGE),
        egui::StrokeKind::Inside,
    );
    if let Some(bracket) = mark.bracket_color() {
        // Corner arms (fixed 6px from each corner, 2px wide) over a faint
        // full ring — the native cut of the JS `::before` marker.
        let outer = rect.expand(2.0);
        painter.rect_stroke(
            outer,
            radius + 2.0,
            egui::Stroke::new(1.0, Color32::from_black_alpha(160)),
            egui::StrokeKind::Outside,
        );
        let arm = 6.0;
        let w = egui::Stroke::new(2.0, bracket);
        for (cx, cy, sx, sy) in [
            (outer.min.x, outer.min.y, 1.0, 1.0),
            (outer.max.x, outer.min.y, -1.0, 1.0),
            (outer.min.x, outer.max.y, 1.0, -1.0),
            (outer.max.x, outer.max.y, -1.0, -1.0),
        ] {
            let c = egui::pos2(cx, cy);
            painter.line_segment([c, c + egui::vec2(arm * sx, 0.0)], w);
            painter.line_segment([c, c + egui::vec2(0.0, arm * sy)], w);
        }
    } else if f > 0.0 {
        painter.rect_stroke(
            rect.expand(1.0),
            radius + 1.0,
            egui::Stroke::new(1.0, Color32::from_white_alpha((200.0 * f) as u8)),
            egui::StrokeKind::Outside,
        );
    }
    resp
}

/// Empty recent cell: dark base with a 4×4 checkerboard, the native cut of
/// the JS `.dt-swatch.is-empty` moiré placeholder.
pub(crate) fn paint_empty_swatch(ui: &mut Ui, cell: f32) {
    let radius = slot_radius(cell);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(cell, cell), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, radius, Color32::from_rgb(0x23, 0x24, 0x28));
    let n = 4.0;
    for gy in 0..4 {
        for gx in 0..4 {
            if (gx + gy) % 2 != 0 {
                continue;
            }
            let p0 = egui::pos2(
                rect.min.x + rect.width() * gx as f32 / n,
                rect.min.y + rect.height() * gy as f32 / n,
            );
            let p1 = egui::pos2(
                rect.min.x + rect.width() * (gx as f32 + 1.0) / n,
                rect.min.y + rect.height() * (gy as f32 + 1.0) / n,
            );
            painter.rect_filled(egui::Rect::from_min_max(p0, p1), 0.0, EDGE_HI);
        }
    }
    painter.rect_stroke(
        rect,
        radius,
        egui::Stroke::new(1.0, EDGE),
        egui::StrokeKind::Inside,
    );
}

/// JS theme accents, shared by the refined slot/window styling below.
pub(crate) const EDGE: Color32 = Color32::from_rgb(0x3a, 0x3d, 0x43);
pub(crate) const EDGE_HI: Color32 = Color32::from_rgb(0x4c, 0x51, 0x59);
const ACCENT: Color32 = Color32::from_rgb(0x00, 0x96, 0xff);
/// Secondary-slot marker: warm amber against the blue primary marker, so two
/// brackets on the grid read as "both slots", never as a doubled single.
const ACCENT2: Color32 = Color32::from_rgb(0xff, 0xb0, 0x20);

/// Which slot a swatch matches. Both slots may hold palette colors at once
/// (JS `syncActiveSwatches` marks either), so the two states need distinct
/// bracket colors; a cell holding *both* reads as primary.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotMark {
    None,
    Primary,
    Secondary,
}

impl SlotMark {
    fn bracket_color(self) -> Option<Color32> {
        match self {
            SlotMark::None => None,
            SlotMark::Primary => Some(ACCENT),
            SlotMark::Secondary => Some(ACCENT2),
        }
    }
}

/// Canonical-hex slot match for one cell (`syncActiveSwatches` rule).
pub(crate) fn slot_mark(primary_hex: &str, secondary_hex: &str, cell_hex: &str) -> SlotMark {
    if cell_hex.eq_ignore_ascii_case(primary_hex) {
        SlotMark::Primary
    } else if cell_hex.eq_ignore_ascii_case(secondary_hex) {
        SlotMark::Secondary
    } else {
        SlotMark::None
    }
}

/// rrect slot corner: JS `--radius-pct: 10%`, made slightly more generous so
/// the slot reads as a rounded card rather than a clipped square.
pub(crate) fn slot_radius(cell: f32) -> f32 {
    (cell * 0.28).clamp(3.0, 8.0)
}

/// Double-border section card: outer 8px box stroke + inner hairline, the
/// native stand-in for the layered `--border`/`--box-line-hi` window dressing.
pub(crate) fn section_card<R>(ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> R {
    let resp = egui::Frame::group(ui.style())
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, EDGE))
        .inner_margin(egui::Vec2::new(8.0, 6.0))
        .show(ui, |ui| body(ui));
    let inner = resp.response.rect.shrink(2.5);
    ui.painter().rect_stroke(
        inner,
        6.0,
        egui::Stroke::new(1.0, EDGE_HI),
        egui::StrokeKind::Inside,
    );
    resp.inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_radius_is_a_generous_rrect() {
        assert_eq!(slot_radius(22.0), 22.0 * 0.28);
        assert_eq!(slot_radius(4.0), 3.0);
        assert_eq!(slot_radius(200.0), 8.0);
    }

    #[test]
    fn slot_mark_distinguishes_the_two_slots() {
        assert_eq!(
            slot_mark("#000000", "#ffffff", "#000000"),
            SlotMark::Primary
        );
        assert_eq!(
            slot_mark("#000000", "#ffffff", "#FFFFFF"),
            SlotMark::Secondary
        );
        // case-insensitive; both match reads as primary
        assert_eq!(
            slot_mark("#FF0000", "#ff0000", "#ff0000"),
            SlotMark::Primary
        );
        assert_eq!(slot_mark("#000000", "#ffffff", "#123456"), SlotMark::None);
    }
}
