//! Hand-drawn per-tool vector icons.
//!
//! Split out of `toolbar.rs`: the panel lays out a grid of buttons, this
//! draws one glyph. Keeping them apart is what lets the "every glyph is
//! distinct and renderable" tests live next to the art they check.

pub(crate) fn draw_tool_vector_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    id: &str,
    color: egui::Color32,
) {
    let stroke = egui::Stroke::new(1.8, color);
    let c = rect.center();
    let pad = 7.0;
    let icon_rect = rect.shrink(pad);

    match id {
        "pen" => {
            let p0 = icon_rect.left_bottom() + egui::vec2(2.0, -2.0);
            let p1 = icon_rect.right_top() + egui::vec2(-2.0, 2.0);
            painter.line_segment([p0, p1], stroke);
            painter.line_segment([p0, p0 + egui::vec2(2.0, -5.0)], stroke);
            painter.line_segment([p0, p0 + egui::vec2(5.0, -2.0)], stroke);
            painter.circle_filled(p1, 2.0, color);
        }
        "eraser" => {
            let er = egui::Rect::from_center_size(c, egui::vec2(14.0, 10.0));
            painter.rect_stroke(er, 2.0, stroke, egui::StrokeKind::Inside);
            painter.line_segment(
                [
                    er.left_top() + egui::vec2(4.0, 0.0),
                    er.left_bottom() + egui::vec2(4.0, 0.0),
                ],
                stroke,
            );
        }
        "fill" => {
            let b0 = c + egui::vec2(-5.0, 4.0);
            let b1 = c + egui::vec2(4.0, -5.0);
            painter.line_segment([b0, b1], stroke);
            painter.rect_stroke(
                egui::Rect::from_center_size(c + egui::vec2(-1.0, -1.0), egui::vec2(9.0, 9.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.circle_filled(c + egui::vec2(5.0, 4.0), 2.0, color);
        }
        "eyedropper" => {
            let tip = icon_rect.left_bottom() + egui::vec2(2.0, -2.0);
            let top = icon_rect.right_top() + egui::vec2(-2.0, 2.0);
            painter.line_segment([tip, top], stroke);
            painter.circle_filled(top, 3.0, color);
            painter.circle_filled(tip, 1.5, color);
        }
        "line" => {
            painter.line_segment(
                [
                    icon_rect.left_bottom() + egui::vec2(2.0, -2.0),
                    icon_rect.right_top() + egui::vec2(-2.0, 2.0),
                ],
                stroke,
            );
        }
        "rect" => {
            let r = egui::Rect::from_center_size(c, egui::vec2(14.0, 11.0));
            painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Inside);
        }
        "rrect" => {
            let r = egui::Rect::from_center_size(c, egui::vec2(14.0, 11.0));
            painter.rect_stroke(r, 3.0, stroke, egui::StrokeKind::Inside);
        }
        "ellipse" => {
            let r = egui::Rect::from_center_size(c, egui::vec2(15.0, 10.0));
            painter.rect_stroke(r, 5.0, stroke, egui::StrokeKind::Inside);
        }
        "polygon" => {
            let p0 = c + egui::vec2(0.0, -6.0);
            let p1 = c + egui::vec2(6.0, 5.0);
            let p2 = c + egui::vec2(-6.0, 5.0);
            painter.line_segment([p0, p1], stroke);
            painter.line_segment([p1, p2], stroke);
            painter.line_segment([p2, p0], stroke);
        }
        "arc" => {
            let r = 6.5;
            for i in 0..12 {
                let a0 = (i as f32 / 12.0) * std::f32::consts::PI;
                let a1 = ((i + 1) as f32 / 12.0) * std::f32::consts::PI;
                painter.line_segment(
                    [
                        c + egui::vec2(a0.cos() * r, -a0.sin() * r + 2.0),
                        c + egui::vec2(a1.cos() * r, -a1.sin() * r + 2.0),
                    ],
                    stroke,
                );
            }
        }
        "chord" => {
            let r = 6.5;
            for i in 0..12 {
                let a0 = (i as f32 / 12.0) * std::f32::consts::PI;
                let a1 = ((i + 1) as f32 / 12.0) * std::f32::consts::PI;
                painter.line_segment(
                    [
                        c + egui::vec2(a0.cos() * r, -a0.sin() * r + 2.0),
                        c + egui::vec2(a1.cos() * r, -a1.sin() * r + 2.0),
                    ],
                    stroke,
                );
            }
            painter.line_segment([c + egui::vec2(-r, 2.0), c + egui::vec2(r, 2.0)], stroke);
        }
        "radial_gradient" => {
            painter.circle_stroke(c, 7.0, stroke);
            painter.circle_stroke(c, 3.0, stroke);
        }
        "text" => {
            painter.line_segment(
                [c + egui::vec2(-5.5, -5.0), c + egui::vec2(5.5, -5.0)],
                egui::Stroke::new(2.2, color),
            );
            painter.line_segment(
                [c + egui::vec2(0.0, -5.0), c + egui::vec2(0.0, 6.0)],
                egui::Stroke::new(2.2, color),
            );
        }
        "hand" => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c + egui::vec2(0.0, 2.0), egui::vec2(9.0, 8.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            for f in [-3.0, 0.0, 3.0] {
                painter.line_segment([c + egui::vec2(f, -2.0), c + egui::vec2(f, -6.0)], stroke);
            }
        }
        _ => {
            painter.circle_filled(c, 3.0, color);
        }
    }
}

/// Whether `id` has a hand-drawn arm of its own, as opposed to falling through
/// to the generic dot above. The rail has no other way to say "this tool has no
/// icon": it would just paint the dot, so the check lives next to the match.
#[cfg(test)]
pub(crate) fn has_icon(id: &str) -> bool {
    matches!(
        id,
        "pen"
            | "eraser"
            | "fill"
            | "eyedropper"
            | "line"
            | "rect"
            | "rrect"
            | "ellipse"
            | "polygon"
            | "arc"
            | "chord"
            | "radial_gradient"
            | "text"
            | "hand"
    )
}
