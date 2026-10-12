//! Toast notification type definition.

#[cfg(test)]
mod tests {
    use crate::ui::{Toast, ToastKind};

    #[test]
    fn toast_kinds() {
        let info = Toast::info("info");
        assert_eq!(info.kind, ToastKind::Info);
        let success = Toast::success("success");
        assert_eq!(success.kind, ToastKind::Success);
        let warning = Toast::warning("warning");
        assert_eq!(warning.kind, ToastKind::Warning);
        let error = Toast::error("error");
        assert_eq!(error.kind, ToastKind::Error);
    }
}
