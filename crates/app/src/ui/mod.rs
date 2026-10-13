//! Native UI layer — egui port of `src/ui/**`.
//
//! This module owns all UI widgets: menubar, toolbar, optionbar, statusbar,
//! dialogs, panels, and the shared components (toast, tooltip, collapsible).
//! The engine (`pixel-engine`) remains pure and knows nothing about egui.

pub mod actions;
pub mod dialogs;
pub mod menubar;
pub mod optionbar;
pub mod shared;
pub mod statusbar;
pub mod toolbar;

// Re-export commonly used items
pub use actions::ActionMap;
pub use dialogs::{Dialog, DialogKind, DialogResult};
pub use menubar::Menubar;
pub use optionbar::Optionbar;
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
    pub toasts: Vec<Toast>,
    pub tooltip: Option<Tooltip>,
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
    pub expanded_sections: std::collections::HashSet<String>,
    pub current_tool_options: Option<ToolOptions>,
}

#[derive(Default, Clone)]
pub struct ToolOptions {
    pub size: Option<u32>,
    pub opacity: Option<f32>,
    pub hardness: Option<f32>,
    pub fill_color: Option<[u8; 4]>,
    pub stroke_color: Option<[u8; 4]>,
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

/// Main UI render function — called from the shell each frame.
/// Takes `&mut egui::Ui` as per eframe 0.36 App::ui signature.
pub fn render_ui(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    app_state: &mut crate::session_glue::AppState,
) {
    // Panels are declared here and only here. Each module's `show()` lays out
    // into the `Ui` handed to it; it must not open a panel of its own. Panels
    // are siblings in egui, and re-opening one inside itself is what clipped
    // the toolbar's buttons to their top ~40%.
    use egui::Vec2;
    let pad = |x: f32, y: f32| egui::Frame::new().inner_margin(Vec2::new(x, y));

    // 1. Menubar (top)
    egui::Panel::top("menubar_panel")
        .frame(pad(8.0, 2.0))
        .show(ui, |ui| {
            menubar::show(ui, ui_state, app_state);
        });

    // 2. Toolbar (below menubar)
    egui::Panel::top("toolbar_panel")
        .frame(pad(8.0, 4.0))
        .show(ui, |ui| {
            toolbar::show(ui, ui_state, app_state);
        });

    // 3. Optionbar (context-sensitive, below toolbar)
    egui::Panel::top("optionbar_panel").show(ui, |ui| {
        optionbar::show(ui, ui_state, app_state);
    });

    // 4. Central canvas area is handled by shell.rs show_canvas()

    // 5. Statusbar (bottom)
    egui::Panel::bottom("statusbar_panel")
        .frame(pad(12.0, 4.0).fill(egui::Color32::from_rgb(255, 0, 0)))
        .show(ui, |ui| {
            statusbar::show(ui, ui_state, app_state);
        });

    // 6. Dialogs (modal, on top)
    dialogs::show(ui, ui_state, app_state);

    // 7. Toasts (top-right, non-blocking)
    shared::toasts::show(ui, ui_state);

    // 8. Tooltip (follows cursor)
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
        assert!(opts.opacity.is_none());
    }
}
