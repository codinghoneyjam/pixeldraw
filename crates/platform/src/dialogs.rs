//! Native modal dialogs (`rfd`) — the `pick_save_path`/`pick_open_path`
//! commands of `src-tauri/src/lib.rs`.
//!
//! TRAP: every function here **blocks the calling thread** on a modal loop, so
//! it must run outside the egui frame (W8 posts the action, then shows it).
//!
//! The blocking calls are not unit-testable; the pure name/filter plumbing is
//! factored into `files.rs` and covered there.

use std::path::PathBuf;

use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

use crate::files::{FileFilter, FileKind, fix_extension, sanitize_file_name, save_filter};

/// `<title>.png`/`<title>.pixeldraw.json` in the same directory the JS dialog
/// defaulted to; `None` when the user cancels.
pub fn pick_save_file(kind: FileKind, default_name: &str) -> Option<PathBuf> {
    let name = fix_extension(&sanitize_file_name(default_name), kind);
    let filter = save_filter(default_name, kind);
    let mut dialog = FileDialog::new().set_file_name(&name);
    dialog = apply_filter(dialog, &filter);
    dialog.save_file()
}

/// `None` when the user cancels.
pub fn pick_open_file(kind: FileKind) -> Option<PathBuf> {
    let filter = kind.open_filter();
    let dialog = FileDialog::new().set_title("열기");
    apply_filter(dialog, &filter).pick_file()
}

/// Replacement for `confirmDiscardChanges` (`src/ui/shared/dialogs.js`) and the
/// other "are you sure" boxes; `true` = confirmed.
///
/// rfd 0.17 dropped `show_confirm`, so the button set is explicit and only
/// `Ok` counts as accepted.
pub fn confirm(title: &str, message: &str) -> bool {
    let result = MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_buttons(MessageButtons::OkCancel)
        .set_level(MessageLevel::Info)
        .show();
    result == MessageDialogResult::Ok
}

fn apply_filter(dialog: FileDialog, filter: &FileFilter) -> FileDialog {
    if filter.is_empty() {
        return dialog;
    }
    let extensions: Vec<&str> = filter.extensions.iter().map(String::as_str).collect();
    dialog.add_filter(&filter.name, &extensions)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Dialogs themselves are manual/untestable (a modal loop needs a window);
    // only the argument plumbing is asserted here.

    #[test]
    fn save_name_is_sanitized_and_suffixed() {
        assert_eq!(
            fix_extension(&sanitize_file_name("my doc"), FileKind::Json),
            "my doc.pixeldraw.json"
        );
        assert_eq!(
            fix_extension(&sanitize_file_name("a/b"), FileKind::Png),
            "a_b.png"
        );
        assert_eq!(
            fix_extension(&sanitize_file_name(""), FileKind::Any),
            "untitled"
        );
    }

    #[test]
    fn save_name_keeps_js_document_suffix() {
        // file_actions.js doSave: `${doc.name || "untitled"}.draw.json`
        assert_eq!(
            fix_extension(&sanitize_file_name("untitled.draw.json"), FileKind::Json),
            "untitled.draw.json"
        );
        assert_eq!(
            save_filter("untitled.draw.json", FileKind::Json).name,
            "JSON"
        );
    }
}
