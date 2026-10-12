//! Shared UI components — toast, tooltip, collapsible sections, strings.

pub mod collapsible;
pub mod strings;
pub mod toast;
pub mod toasts;
pub mod tooltip;

/// Toast notification kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

/// A toast notification.
#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub message: String,
    pub duration: f32, // seconds
    pub created: f64,  // timestamp
}

impl Toast {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            kind: ToastKind::Info,
            message: message.into(),
            duration: 3.0,
            created: 0.0,
        }
    }
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            kind: ToastKind::Success,
            message: message.into(),
            duration: 2.5,
            created: 0.0,
        }
    }
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            kind: ToastKind::Warning,
            message: message.into(),
            duration: 4.0,
            created: 0.0,
        }
    }
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            kind: ToastKind::Error,
            message: message.into(),
            duration: 5.0,
            created: 0.0,
        }
    }
}

/// Tooltip state.
#[derive(Debug, Clone, Default)]
pub struct Tooltip {
    pub text: String,
    pub position: Option<egui::Pos2>,
    pub visible: bool,
}

impl Tooltip {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            position: None,
            visible: false,
        }
    }

    pub fn show_at(&mut self, pos: egui::Pos2, text: impl Into<String>) {
        self.text = text.into();
        self.position = Some(pos);
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }
}

/// Collapsible section state.
#[derive(Debug, Clone, Default)]
pub struct CollapsibleState {
    pub open: bool,
}

impl CollapsibleState {
    pub fn new(open: bool) -> Self {
        Self { open }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
}

/// Render a collapsible section header.
pub fn collapsible(ui: &mut egui::Ui, state: &mut CollapsibleState, title: &str) -> bool {
    let _response = ui.horizontal(|ui| {
        let arrow = if state.open { "▼" } else { "▶" };
        ui.label(egui::RichText::new(arrow).size(10.0));
        let response = ui.button(egui::RichText::new(title).size(13.0).strong());
        if response.clicked() {
            state.toggle();
        }
        response
    });
    state.open
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_creation() {
        let t = Toast::info("Test");
        assert_eq!(t.kind, ToastKind::Info);
        assert_eq!(t.message, "Test");
    }

    #[test]
    fn collapsible_state() {
        let mut s = CollapsibleState::new(false);
        assert!(!s.open);
        s.toggle();
        assert!(s.open);
    }
}
