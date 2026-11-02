//! Dialogs — port of `src/ui/dialogs/**`.
//!
//! The window shell and forms live in `window`, the result application in
//! `apply`; this module keeps the public types and constructors.

mod apply;
#[cfg(test)]
mod tests;
mod window;

use super::UiState;
use crate::session_glue::AppState;

use apply::apply_result;

/// Dialog kind enumeration.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogKind {
    NewDocument,
    ResizeCanvas,
    Preferences,
    Help,
    Confirm {
        title: String,
        message: String,
    },
    ColorPicker,
    LayerProperties,
    ExportOptions,
    /// 3.10b boot restore prompt: document name + `updatedAt` string.
    RestoreAutosave {
        name: String,
        when: String,
    },
    /// 3.10b close guard (`beforeunload` parity; wording from 3.7c).
    ConfirmExit,
    /// Atlas sheet layout / merge / bake.
    Atlas,
}

/// Dialog result returned to caller.
#[derive(Debug, Clone)]
pub enum DialogResult {
    NewDocument {
        width: u32,
        height: u32,
        background: String,
    },
    ResizeCanvas {
        width: u32,
        height: u32,
    },
    Preferences,
    Confirmed(bool),
    Color([u8; 4]),
    None,
    /// 3.10b boot prompt: true = 복원, false = 버리기/Esc/닫기.
    AutosaveRestore(bool),
    /// 3.10b close guard: true = 버리고 닫기, false = 머무르기.
    ConfirmExit(bool),
}

/// Active dialog state.
#[derive(Debug, Clone)]
pub struct Dialog {
    pub kind: DialogKind,
    pub open: bool,
    /// True only when the user explicitly confirmed (OK/적용).
    pub confirmed: bool,
    // Form state for new document
    pub new_doc_width: u32,
    pub new_doc_height: u32,
    pub new_doc_background: String,
    // Form state for resize
    pub resize_width: u32,
    pub resize_height: u32,
    pub atlas: crate::ui::atlas::AtlasState,
}

impl Dialog {
    pub fn new(kind: DialogKind) -> Self {
        Self {
            kind,
            open: true,
            confirmed: false,
            new_doc_width: 512,
            new_doc_height: 512,
            new_doc_background: "transparent".to_string(),
            resize_width: 512,
            resize_height: 512,
            atlas: crate::ui::atlas::AtlasState::default(),
        }
    }

    pub fn new_document() -> Self {
        Self::new(DialogKind::NewDocument)
    }

    pub fn resize_canvas(current_w: u32, current_h: u32) -> Self {
        let mut d = Self::new(DialogKind::ResizeCanvas);
        d.resize_width = current_w;
        d.resize_height = current_h;
        d
    }

    pub fn preferences() -> Self {
        Self::new(DialogKind::Preferences)
    }

    pub fn confirm(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DialogKind::Confirm {
            title: title.into(),
            message: message.into(),
        })
    }

    /// 3.10b boot prompt: `복원` loads the record, `버리기`/Esc drops it (D8).
    pub fn restore_autosave(name: impl Into<String>, when: impl Into<String>) -> Self {
        Self::new(DialogKind::RestoreAutosave {
            name: name.into(),
            when: when.into(),
        })
    }

    /// 3.10b close guard: `버리기` closes anyway, `취소`/Esc stays.
    pub fn confirm_exit() -> Self {
        Self::new(DialogKind::ConfirmExit)
    }

    /// Build the atlas form from the current document's layers.
    pub fn atlas(app: &AppState) -> Self {
        let mut d = Self::new(DialogKind::Atlas);
        d.atlas = crate::ui::atlas::AtlasState::from_app(app);
        d
    }

    pub(super) fn title(&self) -> &str {
        match &self.kind {
            DialogKind::NewDocument => "새 문서",
            DialogKind::ResizeCanvas => "캔버스 크기 변경",
            DialogKind::Preferences => "환경설정",
            DialogKind::Help => "도움말",
            DialogKind::Confirm { title, .. } => title.as_str(),
            DialogKind::ColorPicker => "색상 선택",
            DialogKind::LayerProperties => "레이어 속성",
            DialogKind::ExportOptions => "내보내기 옵션",
            DialogKind::RestoreAutosave { .. } => "자동저장 복원",
            DialogKind::ConfirmExit => "변경 버리기",
            DialogKind::Atlas => "아틀라스 내보내기",
        }
    }
}

/// Dialogs state container.
#[derive(Default)]
pub struct DialogsState {
    pub active: Option<Dialog>,
    pub pending_result: Option<DialogResult>,
}

pub struct Dialogs;

impl Dialogs {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Dialogs {
    fn default() -> Self {
        Self::new()
    }
}

// Integration function
pub fn show(ui: &mut egui::Ui, ui_state: &mut UiState, app_state: &mut AppState) {
    let mut dialogs = Dialogs::new();
    dialogs.show(ui, ui_state, app_state);
    apply_result(ui_state, app_state);
}
