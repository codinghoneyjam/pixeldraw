//! Collapsible section widget.

use egui::{Response, RichText, Ui};

use super::CollapsibleState;

/// Render a collapsible section. Returns true if the section is open.
pub fn show(
    ui: &mut Ui,
    state: &mut CollapsibleState,
    title: &str,
    add_contents: impl FnOnce(&mut Ui),
) -> Response {
    let response = ui.horizontal(|ui| {
        let arrow = if state.open { "▼" } else { "▶" };
        ui.label(RichText::new(arrow).size(10.0).monospace());
        let btn_response = ui.button(RichText::new(title).size(13.0).strong());
        if btn_response.clicked() {
            state.toggle();
        }
        btn_response
    });

    if state.open {
        ui.separator();
        ui.indent("collapsible_content", |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            add_contents(ui);
        });
        ui.separator();
    }

    response.response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsible_toggles() {
        let mut s = CollapsibleState::new(false);
        assert!(!s.open);
        s.toggle();
        assert!(s.open);
        s.toggle();
        assert!(!s.open);
    }
}
