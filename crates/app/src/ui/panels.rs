//! Side panels — facade over the per-section modules.
//!
//! JS `index.html` left: color (slots/wheel/fields) + palette + recent + brush
//! (size/presets/preview). Right: layers (opacity/blend/list/buttons). Each
//! section is its own module; this file only orders them and owns nothing but
//! the module list.

mod brush;
mod chrome;
mod color;
mod layers;
mod palette;

pub use brush::SIZE_PRESETS;
pub use chrome::PanelsState;
pub use palette::DEFAULT_PALETTE;

use super::UiState;
use crate::session_glue::AppState;
use egui::Ui;

pub fn show_left(ui: &mut Ui, ui_state: &mut UiState, app: &mut AppState) {
    color::show_color(ui, ui_state, app);
    palette::show_palette(ui, ui_state, app);
    palette::show_recent(ui, ui_state, app);
    brush::show_brush(ui, ui_state, app);
}

pub fn show_right(ui: &mut Ui, ui_state: &mut UiState, app: &mut AppState) {
    layers::show_right(ui, ui_state, app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_cover_js_range() {
        assert_eq!(SIZE_PRESETS.len(), 16);
        assert_eq!(SIZE_PRESETS[0], 1);
        assert_eq!(SIZE_PRESETS[15], 64);
    }

    #[test]
    fn palette_has_40_colors() {
        assert_eq!(DEFAULT_PALETTE.len(), 40);
    }
}
