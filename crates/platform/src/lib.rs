//! pixel-platform — the OS surface the browser used to provide.
//!
//! Replaces `src/io/file_io.js` (native picker + direct write), `src/io/idb_*.js`
//! (IndexedDB autosave) and `src/ui/shared/storage.js` (localStorage settings).
//! Everything here is behind this crate so `engine` stays pure (docs/contract.md
//! §4's DOM-free rule, native edition).
//!
//! Phase 0.2: skeleton only. The real pieces arrive with Phase 2 (dialogs) and
//! Phase 3 (settings/autosave).

/// Where user data lives: `%APPDATA%/pixeldraw` on Windows.
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
}
