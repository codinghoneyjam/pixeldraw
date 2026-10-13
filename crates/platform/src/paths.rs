//! On-disk locations the browser used to provide for free: localStorage
//! (`src/ui/shared/storage.js`) and IndexedDB (`src/io/store_idb.js`).
//!
//! Native port keeps the two stores as plain files under one app data dir.

use std::path::PathBuf;

use crate::PlatformError;

/// `directories`' qualifier/organization are empty on purpose: the JS/Tauri app
/// id was `pixeldraw` alone (`src-tauri/tauri.conf.json`), so `%APPDATA%\<org>`
/// must not creep into the path.
const APP_NAME: &str = "pixeldraw";

pub const SETTINGS_FILE_NAME: &str = "settings.json";
pub const AUTOSAVE_FILE_NAME: &str = "autosave.json";
pub const LOG_FILE_NAME: &str = "pixeldraw.log";

/// `%APPDATA%/pixeldraw` on Windows.
///
/// TRAP: `ProjectDirs::data_dir()` appends a `data` component on Windows
/// (`%APPDATA%\pixeldraw\data`) while the retired Tauri shell and the legacy
/// `app_dir()` stub used `%APPDATA%\pixeldraw`, so the app name is joined onto
/// the roaming base instead. Falls back to `app_dir()` when no home resolves.
pub fn app_data_dir() -> Option<PathBuf> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.data_dir().join(APP_NAME))
        .or_else(|| Some(crate::app_dir()))
}

pub fn settings_file() -> Option<PathBuf> {
    Some(app_data_dir()?.join(SETTINGS_FILE_NAME))
}

/// Autosave is a single file (IndexedDB chunk records collapse into one JSON
/// document — `docs/io.md` §8's store is a loading detail, not a format).
pub fn autosave_file() -> Option<PathBuf> {
    Some(app_data_dir()?.join(AUTOSAVE_FILE_NAME))
}

pub fn log_file() -> Option<PathBuf> {
    Some(app_data_dir()?.join(LOG_FILE_NAME))
}

/// Best-effort `mkdir -p` for the app data dir; the first writer (settings
/// persist, autosave) needs a directory that does not exist yet.
pub fn ensure_app_data_dir() -> Result<PathBuf, PlatformError> {
    let dir = app_data_dir().ok_or(PlatformError::NoDir)?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn child_of(dir: &Path, leaf: &str) -> bool {
        dir.file_name()
            .map(|n| n.to_string_lossy().contains(leaf))
            .unwrap_or(false)
    }

    #[test]
    fn app_data_dir_is_absolute_and_named_pixeldraw() {
        let dir = app_data_dir().expect("APPDATA is always resolvable on Windows");
        assert!(dir.is_absolute());
        assert!(child_of(&dir, "pixeldraw"), "{dir:?}");
    }

    #[test]
    fn settings_and_autosave_live_under_app_data_dir() {
        let dir = app_data_dir().expect("app data dir");
        for file in [settings_file(), autosave_file(), log_file()] {
            let file = file.expect("file");
            assert!(file.is_absolute());
            assert!(file.starts_with(&dir), "{file:?} not under {dir:?}");
        }
        assert!(settings_file().expect("s").ends_with("settings.json"));
        assert!(autosave_file().expect("a").ends_with("autosave.json"));
    }

    #[test]
    fn ensure_app_data_dir_creates_it() {
        // Uses the real app data dir on purpose: a wrong path here is exactly
        // what Phase 3.9 would silently lose settings to.
        let dir = ensure_app_data_dir().expect("mkdir");
        assert!(dir.is_dir());
    }
}
