//! Action map & keyboard shortcuts — port of `src/ui/actions/**`.

use egui::{Key, Modifiers};
use std::collections::HashMap;

/// A user-facing action that can be triggered by menu, toolbar, or shortcut.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    // File
    NewDocument,
    OpenFile,
    Save,
    SaveAs,
    ExportPng,
    CloseDocument,

    // Edit
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    DeleteAction,

    // View
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ZoomActual,
    ToggleGrid,
    TogglePixelGrid,

    // Tools (toolbar shortcuts)
    ToolPen,
    ToolEraser,
    ToolLine,
    ToolRect,
    ToolEllipse,
    ToolFill,
    ToolEyedropper,
    ToolHand,
    ToolText,
    ToolGradient,
    ToolShape,

    // Layer
    NewLayer,
    DuplicateLayer,
    DeleteLayer,
    MergeDown,
    MergeVisible,
    FlattenImage,

    // Other
    Preferences,
    ShowHelp,
}

/// Keyboard shortcut definition.
#[derive(Debug, Clone)]
pub struct Shortcut {
    pub key: Key,
    pub modifiers: Modifiers,
    pub action: Action,
}

impl Shortcut {
    pub fn new(key: Key, modifiers: Modifiers, action: Action) -> Self {
        Self {
            key,
            modifiers,
            action,
        }
    }

    /// Check if this shortcut matches the current input.
    pub fn matches(&self, ctx: &egui::Context) -> bool {
        ctx.input(|i| i.modifiers == self.modifiers && i.key_pressed(self.key))
    }
}

/// Central action registry with shortcuts and menu/toolbar labels.
pub struct ActionMap {
    shortcuts: Vec<Shortcut>,
    labels: HashMap<Action, &'static str>,
}

impl ActionMap {
    pub fn new() -> Self {
        let mut map = Self {
            shortcuts: Vec::new(),
            labels: HashMap::new(),
        };
        map.register_defaults();
        map
    }

    fn register_defaults(&mut self) {
        use Action::*;
        use Key::*;

        // File
        self.add(
            NewDocument,
            "새 문서",
            Shortcut::new(N, Modifiers::COMMAND, NewDocument),
        );
        self.add(
            OpenFile,
            "열기",
            Shortcut::new(O, Modifiers::COMMAND, OpenFile),
        );
        self.add(Save, "저장", Shortcut::new(S, Modifiers::COMMAND, Save));
        self.add(
            SaveAs,
            "다른 이름으로 저장",
            Shortcut::new(S, Modifiers::COMMAND | Modifiers::SHIFT, SaveAs),
        );
        self.add(
            ExportPng,
            "PNG 내보내기",
            Shortcut::new(E, Modifiers::COMMAND | Modifiers::SHIFT, ExportPng),
        );
        self.add(
            CloseDocument,
            "닫기",
            Shortcut::new(W, Modifiers::COMMAND, CloseDocument),
        );

        // Edit
        self.add(
            Undo,
            "실행 취소",
            Shortcut::new(Z, Modifiers::COMMAND, Undo),
        );
        self.add(
            Redo,
            "다시 실행",
            Shortcut::new(Y, Modifiers::COMMAND, Redo),
        );
        self.add(
            Action::Cut,
            "잘라내기",
            Shortcut::new(X, Modifiers::COMMAND, Action::Cut),
        );
        self.add(
            Action::Copy,
            "복사",
            Shortcut::new(C, Modifiers::COMMAND, Action::Copy),
        );
        self.add(
            Action::Paste,
            "붙여넣기",
            Shortcut::new(V, Modifiers::COMMAND, Action::Paste),
        );
        self.add(
            Action::DeleteAction,
            "삭제",
            Shortcut::new(Delete, Modifiers::NONE, Action::DeleteAction),
        );

        // View
        self.add(
            ZoomIn,
            "확대",
            Shortcut::new(Equals, Modifiers::COMMAND, ZoomIn),
        );
        self.add(
            ZoomOut,
            "축소",
            Shortcut::new(Minus, Modifiers::COMMAND, ZoomOut),
        );
        self.add(
            ZoomFit,
            "화면에 맞춤",
            Shortcut::new(Num0, Modifiers::COMMAND, ZoomFit),
        );
        self.add(
            ZoomActual,
            "실제 크기",
            Shortcut::new(Num1, Modifiers::COMMAND, ZoomActual),
        );
        self.add(
            ToggleGrid,
            "그리드 토글",
            Shortcut::new(G, Modifiers::COMMAND | Modifiers::SHIFT, ToggleGrid),
        );

        // Tools (single-key shortcuts)
        self.add(ToolPen, "펜", Shortcut::new(B, Modifiers::NONE, ToolPen));
        self.add(
            ToolEraser,
            "지우개",
            Shortcut::new(E, Modifiers::NONE, ToolEraser),
        );
        self.add(
            ToolLine,
            "직선",
            Shortcut::new(L, Modifiers::NONE, ToolLine),
        );
        self.add(
            ToolRect,
            "사각형",
            Shortcut::new(R, Modifiers::NONE, ToolRect),
        );
        self.add(
            ToolEllipse,
            "타원",
            Shortcut::new(U, Modifiers::NONE, ToolEllipse),
        );
        self.add(
            ToolFill,
            "채우기",
            Shortcut::new(F, Modifiers::NONE, ToolFill),
        );
        self.add(
            ToolEyedropper,
            "스포이드",
            Shortcut::new(I, Modifiers::NONE, ToolEyedropper),
        );
        self.add(ToolHand, "손", Shortcut::new(H, Modifiers::NONE, ToolHand));
        self.add(
            ToolText,
            "텍스트",
            Shortcut::new(T, Modifiers::NONE, ToolText),
        );
        self.add(
            ToolGradient,
            "그라디언트",
            Shortcut::new(G, Modifiers::NONE, ToolGradient),
        );
        self.add(
            ToolShape,
            "도형",
            Shortcut::new(M, Modifiers::NONE, ToolShape),
        );

        // Layer
        self.add(
            NewLayer,
            "새 레이어",
            Shortcut::new(N, Modifiers::COMMAND | Modifiers::SHIFT, NewLayer),
        );
        self.add(
            DuplicateLayer,
            "레이어 복제",
            Shortcut::new(J, Modifiers::COMMAND, DuplicateLayer),
        );
        self.add(
            DeleteLayer,
            "레이어 삭제",
            Shortcut::new(Backspace, Modifiers::NONE, DeleteLayer),
        );
        self.add(
            MergeDown,
            "아래로 병합",
            Shortcut::new(E, Modifiers::COMMAND, MergeDown),
        );

        // Other
        self.add(
            Preferences,
            "환경설정",
            Shortcut::new(Comma, Modifiers::COMMAND, Preferences),
        );
        self.add(
            ShowHelp,
            "도움말",
            Shortcut::new(F1, Modifiers::NONE, ShowHelp),
        );
    }

    fn add(&mut self, action: Action, label: &'static str, shortcut: Shortcut) {
        self.labels.insert(action.clone(), label);
        self.shortcuts.push(shortcut);
    }

    /// Get the display label for an action.
    pub fn label(&self, action: &Action) -> &str {
        self.labels.get(action).copied().unwrap_or("Unknown")
    }

    /// Get all registered shortcuts.
    pub fn shortcuts(&self) -> &[Shortcut] {
        &self.shortcuts
    }

    /// Find action by shortcut match.
    pub fn find_matching(&self, ctx: &egui::Context) -> Option<Action> {
        self.shortcuts
            .iter()
            .find(|s| s.matches(ctx))
            .map(|s| s.action.clone())
    }

    /// Check if a specific action is triggered this frame.
    pub fn is_triggered(&self, ctx: &egui::Context, action: Action) -> bool {
        self.shortcuts
            .iter()
            .filter(|s| s.action == action)
            .any(|s| s.matches(ctx))
    }
}

impl Default for ActionMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_map_has_all_basic_actions() {
        let map = ActionMap::new();
        assert!(map.label(&Action::NewDocument) != "Unknown");
        assert!(map.label(&Action::Save) != "Unknown");
        assert!(map.label(&Action::Undo) != "Unknown");
        assert!(map.label(&Action::ToolPen) != "Unknown");
    }

    #[test]
    fn shortcut_labels_are_korean() {
        let map = ActionMap::new();
        assert_eq!(map.label(&Action::NewDocument), "새 문서");
        assert_eq!(map.label(&Action::Save), "저장");
        assert_eq!(map.label(&Action::Undo), "실행 취소");
    }
}
