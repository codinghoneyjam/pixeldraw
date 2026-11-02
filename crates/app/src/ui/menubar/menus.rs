//! Contents of each fixed menubar block.

use super::super::actions::Action;
use super::Menubar;
use crate::session_glue::AppState;
use egui::Ui;

impl Menubar {
    pub(super) fn file_contents(&self, ui: &mut Ui, triggered: &mut Option<Action>) {
        if ui
            .button(self.actions.label(&Action::NewDocument))
            .clicked()
        {
            *triggered = Some(Action::NewDocument);
        }
        if ui.button(self.actions.label(&Action::OpenFile)).clicked() {
            *triggered = Some(Action::OpenFile);
        }
        if ui.button(self.actions.label(&Action::ImportPng)).clicked() {
            *triggered = Some(Action::ImportPng);
        }
        if ui
            .button(self.actions.label(&Action::ImportLayer))
            .clicked()
        {
            *triggered = Some(Action::ImportLayer);
        }
        ui.separator();
        if ui.button(self.actions.label(&Action::Save)).clicked() {
            *triggered = Some(Action::Save);
        }
        if ui.button(self.actions.label(&Action::SaveAs)).clicked() {
            *triggered = Some(Action::SaveAs);
        }
        if ui.button(self.actions.label(&Action::ExportPng)).clicked() {
            *triggered = Some(Action::ExportPng);
        }
        if ui
            .button(self.actions.label(&Action::ExportLayer))
            .clicked()
        {
            *triggered = Some(Action::ExportLayer);
        }
        if ui.button(self.actions.label(&Action::Atlas)).clicked() {
            *triggered = Some(Action::Atlas);
        }
        ui.separator();
        if ui
            .button(self.actions.label(&Action::CloseDocument))
            .clicked()
        {
            *triggered = Some(Action::CloseDocument);
        }
    }

    pub(super) fn edit_contents(
        &self,
        ui: &mut Ui,
        app: &mut AppState,
        triggered: &mut Option<Action>,
    ) {
        let (can_undo, can_redo) = app.history_state();
        if ui
            .add_enabled(
                can_undo,
                egui::Button::new(self.actions.label(&Action::Undo)),
            )
            .clicked()
        {
            *triggered = Some(Action::Undo);
        }
        if ui
            .add_enabled(
                can_redo,
                egui::Button::new(self.actions.label(&Action::Redo)),
            )
            .clicked()
        {
            *triggered = Some(Action::Redo);
        }
        ui.separator();
        if ui.button(self.actions.label(&Action::Cut)).clicked() {
            *triggered = Some(Action::Cut);
        }
        if ui.button(self.actions.label(&Action::Copy)).clicked() {
            *triggered = Some(Action::Copy);
        }
        if ui.button(self.actions.label(&Action::Paste)).clicked() {
            *triggered = Some(Action::Paste);
        }
        ui.separator();
        if ui
            .button(self.actions.label(&Action::DeleteAction))
            .clicked()
        {
            *triggered = Some(Action::DeleteAction);
        }
    }

    pub(super) fn view_contents(&self, ui: &mut Ui, triggered: &mut Option<Action>) {
        if ui.button(self.actions.label(&Action::ZoomIn)).clicked() {
            *triggered = Some(Action::ZoomIn);
        }
        if ui.button(self.actions.label(&Action::ZoomOut)).clicked() {
            *triggered = Some(Action::ZoomOut);
        }
        ui.separator();
        if ui.button(self.actions.label(&Action::ZoomFit)).clicked() {
            *triggered = Some(Action::ZoomFit);
        }
        if ui.button(self.actions.label(&Action::ZoomActual)).clicked() {
            *triggered = Some(Action::ZoomActual);
        }
        ui.separator();
        if ui.button(self.actions.label(&Action::ToggleGrid)).clicked() {
            *triggered = Some(Action::ToggleGrid);
        }
    }

    pub(super) fn layer_contents(&self, ui: &mut Ui, triggered: &mut Option<Action>) {
        if ui.button(self.actions.label(&Action::NewLayer)).clicked() {
            *triggered = Some(Action::NewLayer);
        }
        if ui
            .button(self.actions.label(&Action::DuplicateLayer))
            .clicked()
        {
            *triggered = Some(Action::DuplicateLayer);
        }
        if ui
            .button(self.actions.label(&Action::DeleteLayer))
            .clicked()
        {
            *triggered = Some(Action::DeleteLayer);
        }
        ui.separator();
        if ui.button(self.actions.label(&Action::MergeDown)).clicked() {
            *triggered = Some(Action::MergeDown);
        }
        if ui
            .button(self.actions.label(&Action::MergeVisible))
            .clicked()
        {
            *triggered = Some(Action::MergeVisible);
        }
        if ui
            .button(self.actions.label(&Action::FlattenImage))
            .clicked()
        {
            *triggered = Some(Action::FlattenImage);
        }
    }

    pub(super) fn help_contents(&self, ui: &mut Ui, triggered: &mut Option<Action>) {
        if ui.button(self.actions.label(&Action::ShowHelp)).clicked() {
            *triggered = Some(Action::ShowHelp);
        }
        if ui
            .button(self.actions.label(&Action::Preferences))
            .clicked()
        {
            *triggered = Some(Action::Preferences);
        }
    }
}
