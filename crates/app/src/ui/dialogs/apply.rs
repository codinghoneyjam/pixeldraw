//! Applies a closed dialog's confirmed result to the app state.

use super::super::UiState;
use super::DialogKind;
use crate::session_glue::AppState;

/// Integration step: consume a dialog that the user closed.
///
/// Only an explicit confirmation mutates the document; cancel/Esc/backdrop
/// close must leave it untouched (M0-2 DialogOutcome).
pub(super) fn apply_result(ui_state: &mut UiState, app_state: &mut AppState) {
    if let Some(dialog) = ui_state.dialogs.active.as_ref()
        && !dialog.open
    {
        if dialog.confirmed {
            match &dialog.kind {
                DialogKind::NewDocument => {
                    let _ = crate::file_ops::new_document_with_bg(
                        app_state,
                        dialog.new_doc_width,
                        dialog.new_doc_height,
                        &dialog.new_doc_background,
                    );
                }
                DialogKind::ResizeCanvas => {
                    let _ = app_state
                        .session
                        .resize_canvas(dialog.resize_width, dialog.resize_height);
                }
                _ => {}
            }
        }
        ui_state.dialogs.active = None;
    }
}
