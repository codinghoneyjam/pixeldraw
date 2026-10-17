//! Native UI layer — egui port of `src/ui/**`.
//
//! This module owns all UI widgets: menubar, toolbar, optionbar, statusbar,
//! dialogs, panels, and the shared components (toast, tooltip, collapsible).
//! The engine (`pixel-engine`) remains pure and knows nothing about egui.

pub mod actions;
pub mod autosave;
pub mod dialogs;
pub mod juice;
pub mod menubar;
pub mod optionbar;
pub mod panels;
pub mod settings_store;
pub mod shared;
pub mod statusbar;
pub mod theme;
pub mod toolbar;

// Re-export commonly used items
pub use actions::ActionMap;
pub use dialogs::{Dialog, DialogKind, DialogResult};
pub use menubar::Menubar;
pub use shared::{Toast, ToastKind, Tooltip, collapsible, strings};
pub use statusbar::Statusbar;
pub use toolbar::Toolbar;

/// Top-level UI state shared across all widgets.
#[derive(Default)]
pub struct UiState {
    pub menubar: MenubarState,
    pub toolbar: ToolbarState,
    pub optionbar: OptionbarState,
    pub statusbar: StatusbarState,
    pub dialogs: DialogsState,
    pub panels: panels::PanelsState,
    pub toasts: Vec<Toast>,
    pub tooltip: Option<Tooltip>,
    /// Widget key → click time, the clock behind `ui::juice`'s click flash and
    /// press pop. Shared because the clock has to outlive any one widget's
    /// `Ui`, and several widgets animate off the same table.
    pub juice_clicks: std::collections::HashMap<String, f64>,
}

/// Per-widget state structs (minimal for now, expand as needed).
#[derive(Default)]
pub struct MenubarState {
    pub file_menu_open: bool,
    pub edit_menu_open: bool,
    pub view_menu_open: bool,
    pub actions: ActionMap,
}

#[derive(Default)]
pub struct ToolbarState {
    pub active_tool: String,
    pub tool_hovered: Option<String>,
}

#[derive(Default)]
pub struct OptionbarState {
    pub current_tool_options: Option<ToolOptions>,
}

/// The only option value the bar still mirrors into UI state: the brush size,
/// which the pen block reads back as it writes the `penSize` setting. Every
/// other tool option lives on the tool struct itself.
#[derive(Default, Clone)]
pub struct ToolOptions {
    pub size: Option<u32>,
}

#[derive(Default)]
pub struct StatusbarState {
    pub cursor_pos: Option<(i32, i32)>,
    pub zoom: f32,
    pub document_size: Option<(u32, u32)>,
    pub layer_info: Option<String>,
    pub status_messages: Vec<String>,
}

#[derive(Default)]
pub struct DialogsState {
    pub active: Option<Dialog>,
    pub pending_result: Option<DialogResult>,
}

const LEFT_PANEL_MIN_WIDTH: f32 = 180.0;
const LEFT_PANEL_DEFAULT_WIDTH: f32 = 256.0;
const RIGHT_PANEL_MIN_WIDTH: f32 = 200.0;
const RIGHT_PANEL_DEFAULT_WIDTH: f32 = 282.0;
const SIDE_PANEL_MAX_WIDTH: f32 = 440.0;
const TOOL_RAIL_AND_GUTTERS_BUDGET: f32 = 84.0;
const MIN_CANVAS_WIDTH: f32 = 220.0;

fn side_panel_max_width(space_budget: f32, other_panel_width: f32, min_width: f32) -> f32 {
    (space_budget - other_panel_width).clamp(min_width, SIDE_PANEL_MAX_WIDTH)
}

/// Main UI render function — called from the shell each frame.
/// Takes `&mut egui::Ui` as per eframe 0.36 App::ui signature.
pub fn render_ui(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    app_state: &mut crate::session_glue::AppState,
) {
    // Panels are declared here and only here. Each module's `show()` lays out
    // into the `Ui` handed to it; it must not open a panel of its own. Panels
    use egui::Vec2;
    let pad = |x: f32, y: f32| egui::Frame::new().inner_margin(Vec2::new(x, y));

    // 1. Menubar (top)
    egui::Panel::top("menubar_panel")
        .frame(pad(8.0, 2.0))
        .show(ui, |ui| {
            menubar::show(ui, ui_state, app_state);
        });

    // Full-width bars must be declared before side panels consume horizontal space.
    egui::Panel::top("optionbar_panel")
        .frame(pad(8.0, 4.0))
        .show(ui, |ui| {
            optionbar::show(ui, ui_state, app_state);
        });

    egui::Panel::bottom("statusbar_panel")
        .frame(pad(12.0, 4.0))
        .show(ui, |ui| {
            statusbar::show(ui, ui_state, app_state);
        });

    // The tool rail owns the leftmost strip of the remaining workspace.
    egui::Panel::left("toolbar_panel")
        .frame(pad(6.0, 6.0))
        .resizable(false)
        .show(ui, |ui| {
            ui.set_max_width(52.0);
            toolbar::show(ui, ui_state, app_state);
        });

    // Left side panels (colour / palette / recent / brush).
    let side_panel_budget =
        (ui.available_width() - TOOL_RAIL_AND_GUTTERS_BUDGET - MIN_CANVAS_WIDTH).max(0.0);
    let right_panel_width = egui::PanelState::load(ui.ctx(), egui::Id::new("right_panels"))
        .map(|state| state.size().x)
        .unwrap_or(RIGHT_PANEL_DEFAULT_WIDTH);
    let left_panel_max =
        side_panel_max_width(side_panel_budget, right_panel_width, LEFT_PANEL_MIN_WIDTH);
    egui::Panel::left("left_panels")
        .default_size(LEFT_PANEL_DEFAULT_WIDTH)
        .size_range(LEFT_PANEL_MIN_WIDTH..=left_panel_max)
        .frame(pad(8.0, 6.0))
        .resizable(true)
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                panels::show_left(ui, ui_state, app_state);
            });
        });

    let left_panel_width = egui::PanelState::load(ui.ctx(), egui::Id::new("left_panels"))
        .map(|state| state.size().x)
        .unwrap_or(LEFT_PANEL_DEFAULT_WIDTH);
    let right_panel_max =
        side_panel_max_width(side_panel_budget, left_panel_width, RIGHT_PANEL_MIN_WIDTH);
    egui::Panel::right("right_panels")
        .default_size(RIGHT_PANEL_DEFAULT_WIDTH)
        .size_range(RIGHT_PANEL_MIN_WIDTH..=right_panel_max)
        .resizable(true)
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                panels::show_right(ui, ui_state, app_state);
            });
        });

    // Dialogs (modal, on top)
    dialogs::show(ui, ui_state, app_state);

    // Toasts (top-right, non-blocking)
    shared::toasts::show(ui, ui_state);

    // Tooltip (follows cursor)
    shared::tooltip::show(ui, ui_state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_state_defaults_are_empty() {
        let state = UiState::default();
        assert!(state.toasts.is_empty());
        assert!(state.tooltip.is_none());
        assert!(!state.menubar.file_menu_open);
    }

    #[test]
    fn tool_options_default_is_none() {
        let opts = ToolOptions::default();
        assert!(opts.size.is_none());
    }

    #[test]
    fn side_panel_maxima_preserve_canvas_width_at_minimum_window_size() {
        let space_budget =
            crate::shell::WINDOW_MIN_SIZE[0] - TOOL_RAIL_AND_GUTTERS_BUDGET - MIN_CANVAS_WIDTH;
        assert_eq!(space_budget, 596.0);
        assert_eq!(
            side_panel_max_width(
                space_budget,
                RIGHT_PANEL_DEFAULT_WIDTH,
                LEFT_PANEL_MIN_WIDTH
            ),
            314.0
        );
        assert_eq!(
            side_panel_max_width(
                space_budget,
                LEFT_PANEL_DEFAULT_WIDTH,
                RIGHT_PANEL_MIN_WIDTH
            ),
            340.0
        );
        assert_eq!(
            side_panel_max_width(space_budget, 440.0, RIGHT_PANEL_MIN_WIDTH),
            RIGHT_PANEL_MIN_WIDTH
        );
        assert_eq!(
            side_panel_max_width(space_budget, 200.0, LEFT_PANEL_MIN_WIDTH),
            396.0
        );
        assert_eq!(
            side_panel_max_width(space_budget, LEFT_PANEL_MIN_WIDTH, RIGHT_PANEL_MIN_WIDTH),
            416.0
        );
    }
}
