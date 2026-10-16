//! pixel-platform — the OS surface the browser used to provide.
//!
//! Replaces `src/io/file_io.js` (native picker + direct write), `src/io/idb_*.js`
//! (IndexedDB autosave) and `src/ui/shared/storage.js` (localStorage settings).
//! Everything here is behind this crate so `engine` stays pure (docs/contract.md
//! §4's DOM-free rule, native edition).
//!
//! - `paths` — `%APPDATA%/pixeldraw` + settings/autosave/log file names
//! - `files` — byte IO, atomic writes, the JS file-name rules
//! - `dialogs` — native modal pickers (blocking, outside the egui frame)
//! - `ui_prefs` — the `settings.json` UI keys (collapsed panels), merged in
//!   place so the file keeps every field it did not own
//! - `logging` — `env_logger` bootstrap

mod dialogs;
mod files;
mod logging;
mod paths;
mod ui_prefs;

pub use dialogs::{confirm, pick_open_file, pick_save_file};
pub use files::{
    FileFilter, FileKind, MAX_JSON_BYTES, extensions_from_accept, filter_for, fix_extension,
    read_bytes, read_json_text, sanitize_file_name, trailing_extension, write_bytes,
    write_bytes_atomic,
};
pub use logging::init_logging;
pub use paths::{app_data_dir, autosave_file, ensure_app_data_dir, log_file, settings_file};
pub use ui_prefs::{COLLAPSED_FIELD, load_collapsed_panels, save_collapsed_panel};

/// Failures of the platform surface.
///
/// Mirrors the `DrawToolError` codes `src/io/file_io.js` produced
/// (`SCHEMA`/`FILE_TOO_LARGE`/`INVALID_STATE`) so callers map 1:1.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// The app data directory could not be resolved (no `%APPDATA%`).
    #[error("no app data directory")]
    NoDir,
    /// A dialog could not be shown or returned an error.
    #[error("dialog: {0}")]
    Dialog(String),
    /// `readJsonFile`'s 64 MiB guard (`file_io.js` L21).
    #[error("file too large ({size} bytes, limit {limit})")]
    FileTooLarge { size: u64, limit: u64 },
}

/// Where user data lives: `%APPDATA%/pixeldraw` on Windows.
///
/// Kept as the no-dependency fallback of `paths::app_data_dir()`: if
/// `directories` cannot resolve a home directory, settings still land here.
pub fn app_dir() -> std::path::PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("pixeldraw")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_dir_is_absolute() {
        assert!(app_dir().is_absolute());
    }

    #[test]
    fn error_display_carries_the_code() {
        let err = PlatformError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"));
        assert!(err.to_string().contains("missing"));
        assert!(matches!(err, PlatformError::Io(_)));
        assert!(PlatformError::NoDir.to_string().contains("no app data"));
        assert!(
            PlatformError::Dialog("cancelled".into())
                .to_string()
                .contains("cancelled")
        );
        assert!(
            PlatformError::FileTooLarge { size: 1, limit: 2 }
                .to_string()
                .contains("too large")
        );
    }
}
