//! Menubar — port of `src/ui/menubar/**`.

use super::{
    UiState,
    actions::{Action, ActionMap},
};
use crate::session_glue::AppState;
use egui::Ui;

#[derive(Default)]
pub struct MenubarState {
    pub file_menu_open: bool,
    pub edit_menu_open: bool,
    pub view_menu_open: bool,
}

pub struct Menubar {
    actions: ActionMap,
}

impl Menubar {
    pub fn new() -> Self {
        Self {
            actions: ActionMap::new(),
        }
    }

    /// Render the menubar. Returns any triggered action.
    pub fn show(&mut self, ui: &mut Ui, state: &mut UiState, app: &mut AppState) -> Option<Action> {
        let mut triggered = None;
        ui.horizontal(|ui| {
            // File menu
            triggered = self.file_menu(ui, state, app);
            // Edit menu
            let r = self.edit_menu(ui, state, app);
            if triggered.is_none() {
                triggered = r;
            }
            // View menu
            let r = self.view_menu(ui, state, app);
            if triggered.is_none() {
                triggered = r;
            }
            // Layer menu
            let r = self.layer_menu(ui, state, app);
            if triggered.is_none() {
                triggered = r;
            }
            // Help menu
            let r = self.help_menu(ui, state);
            if triggered.is_none() {
                triggered = r;
            }
        });
        triggered
    }

    fn file_menu(
        &mut self,
        ui: &mut Ui,
        _state: &mut UiState,
        _app: &mut AppState,
    ) -> Option<Action> {
        let mut triggered = None;
        ui.menu_button(self.actions.label(&Action::NewDocument), |ui| {
            if ui
                .button(self.actions.label(&Action::NewDocument))
                .clicked()
            {
                triggered = Some(Action::NewDocument);
                ui.close();
            }
            if ui.button(self.actions.label(&Action::OpenFile)).clicked() {
                triggered = Some(Action::OpenFile);
                ui.close();
            }
            ui.separator();
            if ui.button(self.actions.label(&Action::Save)).clicked() {
                triggered = Some(Action::Save);
                ui.close();
            }
            if ui.button(self.actions.label(&Action::SaveAs)).clicked() {
                triggered = Some(Action::SaveAs);
                ui.close();
            }
            if ui.button(self.actions.label(&Action::ExportPng)).clicked() {
                triggered = Some(Action::ExportPng);
                ui.close();
            }
            ui.separator();
            if ui
                .button(self.actions.label(&Action::CloseDocument))
                .clicked()
            {
                triggered = Some(Action::CloseDocument);
                ui.close();
            }
        });
        triggered
    }

    fn edit_menu(
        &mut self,
        ui: &mut Ui,
        _state: &mut UiState,
        app: &mut AppState,
    ) -> Option<Action> {
        let mut triggered = None;
        ui.menu_button("편집", |ui| {
            let (can_undo, can_redo) = app.history_state();
            if ui
                .add_enabled(
                    can_undo,
                    egui::Button::new(self.actions.label(&Action::Undo)),
                )
                .clicked()
            {
                triggered = Some(Action::Undo);
                ui.close();
            }
            if ui
                .add_enabled(
                    can_redo,
                    egui::Button::new(self.actions.label(&Action::Redo)),
                )
                .clicked()
            {
                triggered = Some(Action::Redo);
                ui.close();
            }
            ui.separator();
            if ui.button(self.actions.label(&Action::Cut)).clicked() {
                triggered = Some(Action::Cut);
                ui.close();
            }
            if ui.button(self.actions.label(&Action::Copy)).clicked() {
                triggered = Some(Action::Copy);
                ui.close();
            }
            if ui.button(self.actions.label(&Action::Paste)).clicked() {
                triggered = Some(Action::Paste);
                ui.close();
            }
            ui.separator();
            if ui
                .button(self.actions.label(&Action::DeleteAction))
                .clicked()
            {
                triggered = Some(Action::DeleteAction);
                ui.close();
            }
        });
        triggered
    }

    fn view_menu(
        &mut self,
        ui: &mut Ui,
        _state: &mut UiState,
        _app: &mut AppState,
    ) -> Option<Action> {
        let mut triggered = None;
        ui.menu_button("보기", |ui| {
            if ui.button(self.actions.label(&Action::ZoomIn)).clicked() {
                triggered = Some(Action::ZoomIn);
                ui.close();
            }
            if ui.button(self.actions.label(&Action::ZoomOut)).clicked() {
                triggered = Some(Action::ZoomOut);
                ui.close();
            }
            ui.separator();
            if ui.button(self.actions.label(&Action::ZoomFit)).clicked() {
                triggered = Some(Action::ZoomFit);
                ui.close();
            }
            if ui.button(self.actions.label(&Action::ZoomActual)).clicked() {
                triggered = Some(Action::ZoomActual);
                ui.close();
            }
            ui.separator();
            if ui.button(self.actions.label(&Action::ToggleGrid)).clicked() {
                triggered = Some(Action::ToggleGrid);
                ui.close();
            }
        });
        triggered
    }

    fn layer_menu(
        &mut self,
        ui: &mut Ui,
        _state: &mut UiState,
        _app: &mut AppState,
    ) -> Option<Action> {
        let mut triggered = None;
        ui.menu_button("레이어", |ui| {
            if ui.button(self.actions.label(&Action::NewLayer)).clicked() {
                triggered = Some(Action::NewLayer);
                ui.close();
            }
            if ui
                .button(self.actions.label(&Action::DuplicateLayer))
                .clicked()
            {
                triggered = Some(Action::DuplicateLayer);
                ui.close();
            }
            if ui
                .button(self.actions.label(&Action::DeleteLayer))
                .clicked()
            {
                triggered = Some(Action::DeleteLayer);
                ui.close();
            }
            ui.separator();
            if ui.button(self.actions.label(&Action::MergeDown)).clicked() {
                triggered = Some(Action::MergeDown);
                ui.close();
            }
            if ui
                .button(self.actions.label(&Action::MergeVisible))
                .clicked()
            {
                triggered = Some(Action::MergeVisible);
                ui.close();
            }
            if ui
                .button(self.actions.label(&Action::FlattenImage))
                .clicked()
            {
                triggered = Some(Action::FlattenImage);
                ui.close();
            }
        });
        triggered
    }

    fn help_menu(&mut self, ui: &mut Ui, _state: &mut UiState) -> Option<Action> {
        let mut triggered = None;
        ui.menu_button("도움말", |ui| {
            if ui.button(self.actions.label(&Action::ShowHelp)).clicked() {
                triggered = Some(Action::ShowHelp);
                ui.close();
            }
            if ui
                .button(self.actions.label(&Action::Preferences))
                .clicked()
            {
                triggered = Some(Action::Preferences);
                ui.close();
            }
        });
        triggered
    }
}

impl Default for Menubar {
    fn default() -> Self {
        Self::new()
    }
}

// Integration function for the top-level UI render.
//
// `render_ui` in `super::mod` already owns `Panel::top("menubar_panel")`; this
// used to re-open the same id, nesting a panel inside itself.
pub fn show(ui: &mut egui::Ui, ui_state: &mut UiState, app_state: &mut AppState) {
    let mut menubar = Menubar::new();
    if let Some(action) = menubar.show(ui, ui_state, app_state) {
        dispatch_action(action, app_state, ui_state);
    }
}

fn dispatch_action(action: Action, app: &mut AppState, ui_state: &mut UiState) {
    use Action::*;
    match action {
        NewDocument => {
            ui_state.dialogs.active = Some(super::dialogs::Dialog::new_document());
        }
        OpenFile => {
            if let Some(path) = pixel_platform::pick_open_file(pixel_platform::FileKind::Any) {
                crate::shell::open_any(app, &path);
            }
        }
        Save => {
            if let Ok(bytes) = crate::file_ops::save_json_bytes(app)
                && let Some(path) =
                    pixel_platform::pick_save_file(pixel_platform::FileKind::Json, "untitled")
            {
                let _ = pixel_platform::write_bytes_atomic(&path, &bytes);
            }
        }
        SaveAs => {
            // Same as Save for now
        }
        ExportPng => {
            if let Ok(bytes) = crate::file_ops::export_png_bytes(app)
                && let Some(path) =
                    pixel_platform::pick_save_file(pixel_platform::FileKind::Png, "canvas")
            {
                let _ = pixel_platform::write_bytes_atomic(&path, &bytes);
            }
        }
        Undo => {
            let _ = app.session.undo();
        }
        Redo => {
            let _ = app.session.redo();
        }
        ZoomIn => {
            app.view_store.zoom_in();
        }
        ZoomOut => {
            app.view_store.zoom_out();
        }
        ZoomFit => {
            app.view_store.fit();
        }
        ZoomActual => {
            app.view_store.actual_size();
        }
        ToggleGrid => {
            let current = app.session.settings().grid_mode;
            let next = match current {
                "off" => "unit",
                "unit" => "tile",
                "tile" => "pixel",
                _ => "off",
            };
            let _ = app
                .session
                .set_setting("gridMode", pixel_engine::model::SettingInput::Str(next));
        }
        NewLayer => {
            let _ = app.session.add_layer(None);
        }
        DuplicateLayer => {
            if let Some(id) = app.session.doc().map(|d| d.active_layer_id.clone()) {
                let _ = app.session.duplicate_layer(&id);
            }
        }
        DeleteLayer => {
            if let Some(id) = app.session.doc().map(|d| d.active_layer_id.clone()) {
                let _ = app.session.remove_layer(&id);
            }
        }
        MergeDown => {
            if let Some(id) = app.session.doc().map(|d| d.active_layer_id.clone()) {
                let _ = app.session.merge_down(&id);
            }
        }
        Preferences => {
            ui_state.dialogs.active = Some(super::dialogs::Dialog::preferences());
        }
        ShowHelp => {
            ui_state.dialogs.active = Some(super::dialogs::Dialog::new(
                super::dialogs::DialogKind::Help,
            ));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menubar_creation_works() {
        let menubar = Menubar::new();
        assert_eq!(menubar.actions.label(&Action::NewDocument), "새 문서");
    }
}
